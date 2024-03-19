use base64::engine::general_purpose::STANDARD;
use std::fs::File;
use std::io;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::io::{Error, ErrorKind};
use std::net::TcpStream;
use std::vec;
use yazi::{Decoder, Format, Adler32};
use std::thread;
use std::sync::{Arc, Mutex};
mod text_formatting;

#[derive(Clone)]
struct PlayerInfo {
    uuid: Vec<u8>,
    username: String,
    ping: i32,
}

impl PlayerInfo {
    fn new() -> PlayerInfo {
        PlayerInfo {
            uuid: Vec::new(),
            username: String::new(),
            ping: 0,
        }
    }
}

#[derive(Clone)]
struct Players {
    players : Vec<PlayerInfo>,
}

impl Players {
    fn new() -> Players {
        Players {
            players: Vec::new(),
        }
    }

    fn register (&mut self, player: PlayerInfo) {
        if let Some(index) = self.players.iter_mut().find(|p| p.uuid == player.uuid) {
            index.ping = player.ping;
        }
        else {
            self.players.push(player);
        }
    }

    fn update_ping (&mut self, uuid: Vec<u8>, ping: i32) {
        if let Some(pos) = self.players.iter().position(|p| p.uuid == uuid) {
            self.players[pos].ping = ping;
        }
    }

    fn remove_player (&mut self, uuid: Vec<u8>) {
        if let Some(pos) = self.players.iter().position(|p| p.uuid == uuid) {
            self.players.remove(pos);
        }
    }

    pub fn print_all_players(&self) {
        for player in &self.players {
            println!("Username: {}  ping: {}", player.username, player.ping);
        }
    }
}

const SEGMENT_BITS: u8 = 0b0111_1111;
const CONTINUE_BIT: u8 = 0b1000_0000;

fn write_var_int(buffer: &mut Vec<u8>, mut value: i32) -> io::Result<()> {
    loop {
        if (value & !(SEGMENT_BITS as i32)) == 0 {
            buffer.push(value as u8);
            return Ok(());
        }

        buffer.push((value & SEGMENT_BITS as i32) as u8 | CONTINUE_BIT);
        value = ((value as u32) >> 7) as i32;
    }
}

fn read_var_int(buffer: &mut [u8]) -> io::Result<Vec<i32>> {
    let mut value: i32 = 0;
    let mut position: i32 = 0;
    let mut current_byte: u8;
    let mut next_byte: u8 = 0;

    for (i, item) in buffer.iter().enumerate() {
        next_byte = i as u8;
        current_byte = *item;
        value |= ((current_byte & SEGMENT_BITS) as i32) << position;

        if ((current_byte & CONTINUE_BIT) as i32) == 0 {
            break;
        }

        position += 7;

        if position >= 32 {
            return Err(Error::new(ErrorKind::Other, "VarInt is too big"));
        }
    }

    next_byte += 1;

    Ok(vec![value, next_byte as i32])
}

fn read_var_int_from_stream(mut stream: &TcpStream) -> io::Result<i32> {
    let mut value: i32 = 0;
    let mut position: i32 = 0;
    let mut current_byte: [u8; 1] = [0];
    //let mut nr_bytes: i32 = 0;

    loop {
        stream.read_exact(&mut current_byte)?;
        //nr_bytes = nr_bytes + 1;
        value |= ((current_byte[0] & SEGMENT_BITS) as i32) << position;

        if ((current_byte[0] & CONTINUE_BIT) as i32) == 0 {
            break;
        }

        position += 7;

        if position >= 32 {
            return Err(Error::new(ErrorKind::Other, "VarInt from is too big"));
        }
    }

    Ok(value)
}

/* 
fn write_var_long(buffer: &mut Vec<u8>, mut value: i64) -> io::Result<()> {
    loop {
        if (value & !(SEGMENT_BITS as i64)) == 0 {
            buffer.push(value as u8);
            return Ok(());
        }

        buffer.push((value & SEGMENT_BITS as i64) as u8 | CONTINUE_BIT);
        value = ((value as u64) >> 7) as i64;
    }
}

fn read_var_long(buffer: &mut Vec<u8>) -> io::Result<i64> {
    let mut value: i64 = 0;
    let mut position: i64 = 0;
    let mut current_byte: u8;

    for i in 0..buffer.len() {
        current_byte = buffer[i];
        value |= ((current_byte & SEGMENT_BITS) as i64) << position;

        if ((current_byte & CONTINUE_BIT) as i64) == 0 {
            break;
        }

        position += 7;

        if position >= 64 {
            return Err(Error::new(ErrorKind::Other, "VarLong is too big"));
        }
    }

    return Ok(value);
}*/

fn write_string(buffer: &mut Vec<u8>, value: &str) -> io::Result<()> {
    write_var_int(buffer, value.len() as i32)?;
    buffer.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_long (buffer: &mut Vec<u8>, value: i64) -> io::Result<()> {
    let mut payload_vec: Vec<u8> = value.to_be_bytes().to_vec();
    buffer.append(&mut payload_vec);
    Ok(())
}

fn handshake_packet(id: u8, version: i32, ip: &str, port: u16, number: i32) -> io::Result<Vec<u8>> {
    let mut packet: Vec<u8> = Vec::<u8>::new();
    packet.push(id);
    write_var_int(&mut packet, version)?;
    write_string(&mut packet, ip)?;
    packet.push(port.to_be_bytes()[0]);
    packet.push(port.to_be_bytes()[1]);
    write_var_int(&mut packet, number)?;
    Ok(packet)
}

fn packet_lenght(packet: Vec<u8>) -> io::Result<Vec<u8>> {
    let lenght: i32 = packet.len() as i32;
    let mut packet_lenght: Vec<u8> = Vec::<u8>::new();
    write_var_int(&mut packet_lenght, lenght)?;
    Ok(packet_lenght)
}

fn status_request_packet(id: u8) -> io::Result<Vec<u8>> {
    let mut packet: Vec<u8> = Vec::<u8>::new();
    write_var_int(&mut packet, 1)?;
    packet.push(id);
    Ok(packet)
}

fn read_packets(mut stream: &TcpStream) -> io::Result<()> {
    let mut buffer: Vec<u8> = Vec::<u8>::new();
    stream.read_to_end(&mut buffer)?;
    //println!("Bytes read: {}", bytes);
    let packet_length1: Vec<i32> = read_var_int(&mut buffer)?;
    let packet_size1: i32 = packet_length1[0];
    let pack1: Vec<u8> = buffer[0..(packet_size1 + packet_length1[1]) as usize].to_vec();
    let pack2: Vec<u8> = buffer[(packet_size1 + packet_length1[1]) as usize..].to_vec();
    read_status_response(&mut pack1.clone())?;
    read_ping_response(&mut pack2.clone())?;
    Ok(())
}

fn read_status_response(response: &mut [u8]) -> io::Result<()> {
    let vec1: Vec<i32> = read_var_int(response)?;
    let packet_size: i32 = vec1[0];
    let mut read_next: i32 = vec1[1];
    let packet_id: i32 = response[read_next as usize] as i32;
    read_next += 1;
    let mut slice = Vec::<u8>::new();
    slice.extend_from_slice(&response[read_next as usize..]);
    let vec2: Vec<i32> = read_var_int(&mut slice)?;
    let json_size: i32 = vec2[0];
    read_next = vec2[1];
    let mut slice2 = Vec::<u8>::new();
    slice2.extend_from_slice(&slice[read_next as usize..]);
    let json_string = String::from_utf8(slice2).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json_string).unwrap();

    println!("====Status_response====");
    println!("Packet id: {}", packet_id);
    println!("Packet size: {}", packet_size);
    println!("Json size: {}", json_size);
    println!("Server version: {}", json["version"]["name"]);
    println!("Server protocol: {}", json["version"]["protocol"]);
    println!("Online players: {}", json["players"]["online"]);
    println!("Max players: {}", json["players"]["max"]);
    
    let image: &str = json["favicon"].as_str().unwrap();
    let image: String = image.replace("data:image/png;base64,", "");
    let image_str: &str = &image;
    save_image(image_str)?;
    Ok(())
}

fn save_image(image: &str) -> io::Result<()> {
    let mut file = File::options().write(true).open("src/image.png")?;
    let mut reader = Cursor::new(image);
    let mut decoder = base64::read::DecoderReader::new(&mut reader, &STANDARD);
    io::copy(&mut decoder, &mut file)?;

    Ok(())
}

fn ping_request_packet() -> io::Result<Vec<u8>> {
    let mut packet: Vec<u8> = Vec::<u8>::new();
    write_var_int(&mut packet, 0x01)?;
    let payload: i64 = 77;
    write_long(&mut packet, payload)?;
    let mut ping_packet: Vec<u8> = packet_lenght(packet.clone())?;
    ping_packet.append(&mut packet);
    Ok(ping_packet)
}

fn read_ping_response(response: &mut [u8]) -> io::Result<()> {
    let vec1: Vec<i32> = read_var_int(response)?;
    let packet_size: i32 = vec1[0];
    let mut read_next: i32 = vec1[1];
    let packet_id: i32 = response[read_next as usize] as i32;
    read_next += 1;
    let mut slice = Vec::<u8>::new();
    slice.extend_from_slice(&response[read_next as usize..]);
    let bytes_array: [u8; 8] = slice.try_into().expect("Too long for i64");
    let payload: i64 = i64::from_be_bytes(bytes_array);
    println!("====Pong_response====");
    println!("Packet id: {}", packet_id);
    println!("Packet size: {}", packet_size);
    println!("Payload: {}", payload);
    Ok(())
}

fn login_request() -> io::Result<Vec<u8>> {
    let mut packet: Vec<u8> = Vec::<u8>::new();
    write_var_int(&mut packet, 0x00)?;
    let mut username = String::new();
    loop {
        println!("Write your username:");
        io::stdin().read_line(&mut username)?;
        username = username.trim().replace('\0', "");
        let name = username.trim();
        if name.len() > 16 {
            println!("Username too long");
            username.clear();
            continue;
        }
        break;
    }
    write_string(&mut packet, &username)?;
    let mut login_packet: Vec<u8> = packet_lenght(packet.clone())?;
    login_packet.append(&mut packet);
    Ok(login_packet)
}

fn flush_bytes(mut stream: &TcpStream, nr_bytes: usize) -> io::Result<()> {
    let mut buffer: Vec<u8> = vec![0; nr_bytes];
    stream.read_exact(&mut buffer)?;
    Ok(())
}

fn packet_decoder(mut stream: &TcpStream, nr_bytes: usize) -> Result<Vec<u8>, yazi::Error> {
    let mut buffer: Vec<u8> = vec![0; nr_bytes];
    stream.read_exact(&mut buffer)?;

    let mut decoder = Decoder::new();
    decoder.set_format(Format::Zlib);

    let mut decompressed_vec = Vec::<u8>::new();
    let mut decomp_stream = decoder.stream_into_vec(&mut decompressed_vec);
    decomp_stream.write_all(&buffer)?;

    let (_, chekcsum) = decomp_stream.finish()?;
    if Adler32::from_buf(&decompressed_vec).finish() != chekcsum.unwrap() {
        return Err(yazi::Error::InvalidBitstream);
    }
    Ok(decompressed_vec)
}

fn packet_monitoring(mut stream: &TcpStream, all_players: Arc<Mutex<Players>>) -> io::Result<()> {
    let mut login: bool = false;
    loop {
        let packet_length: i32 = read_var_int_from_stream(stream)?;
        let data_length: i32 = read_var_int_from_stream(stream)?;
        let mut data_length_in_bytes: Vec<u8> = Vec::<u8>::new();
        write_var_int(&mut data_length_in_bytes, data_length)?;
        let bytes: usize = data_length_in_bytes.len();
        if data_length == 0 {
            let packet_id: i32 = read_var_int_from_stream(stream)?;
            if login && packet_id == 0x02 {
                flush_bytes(stream, (packet_length - 2) as usize)?;
                continue;
            }
            match packet_id {
                0x02 => {
                    login=true;
                    println!("====Login_success====");
                    println!("Packet id: {}", packet_id);
                    println!("Packet size: {}", packet_length);
                    login_success(stream)?;
                }
                
                0x0F => {
                    let mut buffer: Vec<u8> = vec![0; (packet_length - 2) as usize];
                    stream.read_exact(&mut buffer)?;
                    chat_from_server(buffer)?;
                }

                0x21 => {
                    keep_alive_from_server(stream)?;
                }

                0x36 => {
                    let mut buffer: Vec<u8> = vec![0; (packet_length - 2) as usize];
                    stream.read_exact(&mut buffer)?;
                    let all_players_clone = Arc::clone(&all_players);
                    player_info(buffer, all_players_clone)?;
                }

                _ => {
                    flush_bytes(stream, (packet_length - 2) as usize)?;
                }
            }
        }

        if data_length != 0 {
            let packet: Vec<u8> = packet_decoder(stream, packet_length as usize - bytes).unwrap();
            let packet_id: i32 = packet[0] as i32;
            let mut data: Vec<u8> = packet[1..].to_vec();

            match packet_id {
                0x0F => {
                    chat_from_server(data)?; 
                }

                0x36 => {
                    println!("here");
                    let all_players_clone = Arc::clone(&all_players);
                    player_info(data, all_players_clone)?;
                }

                _ => {
                    data.clear();
                }
            }
        }
    }
}

fn login_success(mut stream: &TcpStream) -> io::Result<()> {
    let mut uuid: Vec<u8> = vec![0; 16];
    stream.read_exact(&mut uuid)?;
    let username_size = read_var_int_from_stream(stream)?;
    let mut buffer: Vec<u8> = vec![0; username_size as usize];
    stream.read_exact(&mut buffer)?;
    let username = String::from_utf8(buffer).unwrap();
    println!("Uuid: {:02x?}", uuid);
    println!("Your username is: {}", username);
    Ok(())
}

fn set_compression(stream: &TcpStream) -> io::Result<()> {
    let packet_size: i32 = read_var_int_from_stream(stream)?;
    let packet_id: i32 = read_var_int_from_stream(stream)?;
    let compression: i32 = read_var_int_from_stream(stream)?;
    println!("====Set_compression====");
    println!("Packet id: {}", packet_id);
    println!("Packet size: {}", packet_size);
    println!("Compression: {}", compression);
    Ok(())
}

fn keep_alive_from_server(mut stream: &TcpStream) -> io::Result<()> {
    let mut keep_alive: Vec<u8> = vec![0; 8];
    stream.read_exact(&mut keep_alive)?;
    keep_alive_from_client(stream, &mut keep_alive)?;
    Ok(())
}

fn keep_alive_from_client(mut stream: &TcpStream, keep_alive: &mut Vec<u8>) -> io::Result<()> {
    let mut buffer: Vec<u8> = Vec::<u8>::new();
    buffer.push(0x00_u8);
    buffer.push(0x0F_u8);
    buffer.append(keep_alive);
    let mut packet: Vec<u8> = packet_lenght(buffer.clone())?;
    packet.append(&mut buffer);
    stream.write_all(&packet)?;
    Ok(())
}

fn read_json_from_file(file_text: &str) -> io::Result<serde_json::Value> {
    let file = File::open(file_text)?;
    let json = serde_json::from_reader(file)?;
    Ok(json)
}

fn chat_from_server(buffer: Vec<u8>) -> io::Result<()> {
    let vec = read_var_int(&mut buffer.clone())?;
    let nbt_size = vec[0];
    let read_next = vec[1];
    let nbt_vec = buffer[read_next as usize..nbt_size as usize + read_next as usize].to_vec();
    let nbt_text: String = String::from_utf8(nbt_vec).unwrap();
    let json: serde_json::Value = serde_json::from_str(&nbt_text).unwrap();
    let style = text_formatting::Styles::new();
    text_formatting::parse_json_obj(json, style)?;
    println!();
    Ok(())
}

fn f1(stream: &TcpStream, all_players: Arc<Mutex<Players>>) -> io::Result<()> {
    packet_monitoring(stream, all_players)?;
    Ok(())
}

fn f2(stream: &TcpStream, all_players: Arc<Mutex<Players>>) -> io::Result<()> {
    loop {
        let mut message = String::new();
        io::stdin().read_line(&mut message)?;
        message = message.trim().replace('\0', "");
        let msg = message.trim();
        let mut buffer: Vec<u8> = Vec::<u8>::new();
        buffer.push(0x00);
        buffer.push(0x03_u8);
        let msg_len = msg.len();
        if msg_len > 256 {
            println!("Message too long");
            message.clear();
            buffer.clear();
            continue;
        }

        match msg {
            "/help" => {
                println!("===Custom_commands===");
                println!("</all players> : prints online players");
                println!("</quit> : exits the application");
            }

            "/quit" => {
                stream.shutdown(std::net::Shutdown::Both)?;
                std::process::exit(0);
            }

            "/all players" => {
                all_players.lock().unwrap().print_all_players();
            }

            _ => {
                write_var_int(&mut buffer, msg_len as i32)?;
                buffer.append(&mut msg.as_bytes().to_vec());
                let mut packet: Vec<u8> = packet_lenght(buffer.clone())?;
                packet.append(&mut buffer);
                let stream = Arc::new(Mutex::new(stream));
                let mut stream = stream.lock().unwrap();
                stream.write_all(&packet)?;
            }
        }
    }
}

fn player_info (mut buffer: Vec<u8>, all_players: Arc<Mutex<Players>>) -> io::Result<()> {
    let vec = read_var_int(&mut buffer)?;
    let action = vec[0];
    let mut read_next = vec[1];
    buffer = buffer[read_next as usize..].to_vec();
    let vec = read_var_int(&mut buffer)?;
    let nr_players = vec[0];
    read_next = vec[1];
    let mut player_vec = buffer[read_next as usize..].to_vec();
    for _ in 0..nr_players {
        let mut player: PlayerInfo = PlayerInfo::new();
        match action {
            0 => {
                    player.uuid = player_vec[0..16].to_vec();
                    player_vec = player_vec[16..].to_vec();
                    let vec = read_var_int(&mut player_vec)?;
                    let player_name_size = vec[0];
                    read_next = vec[1];
                    let player_username = player_vec[read_next as usize .. player_name_size as usize + read_next as usize].to_vec();
                    player.username = String::from_utf8(player_username).unwrap();
                    player_vec = player_vec[player_name_size as usize + read_next as usize..].to_vec();
                    let vec = read_var_int(&mut player_vec)?;
                    let nop = vec[0];
                    read_next = vec[1];
                    player_vec = player_vec[read_next as usize..].to_vec();
                    for _ in 0..nop {
                        let vec = read_var_int(&mut player_vec)?;
                        let name_size = vec[0];
                        read_next = vec[1];
                        player_vec = player_vec[name_size as usize + read_next as usize..].to_vec();
                        let vec = read_var_int(&mut player_vec)?;
                        let value_size = vec[0];
                        read_next = vec[1];
                        player_vec = player_vec[value_size as usize + read_next as usize..].to_vec();
                        let signed: u8 = player_vec[0];
                        player_vec = player_vec[1..].to_vec();
                        if signed == 0x01 {
                            let vec = read_var_int(&mut player_vec)?;
                            let signature_size = vec[0];
                            read_next = vec[1];
                            player_vec = player_vec[signature_size as usize + read_next as usize..].to_vec();
                        }
                    }
                    let vec = read_var_int(&mut player_vec)?;
                    read_next = vec[1];
                    player_vec = player_vec[read_next as usize..].to_vec();
                    let vec = read_var_int(&mut player_vec)?;
                    let ping = vec[0];
                    player.ping = ping;
                    read_next = vec[1];
                    player_vec = player_vec[read_next as usize..].to_vec();
                    let display_name: u8 = player_vec[0];
                    player_vec = player_vec[1..].to_vec();
                    if display_name == 0x01 {
                        let vec = read_var_int(&mut player_vec)?;
                        let display_name_size = vec[0];
                        read_next = vec[1];
                        player_vec = player_vec[display_name_size as usize + read_next as usize..].to_vec();
                    }
                    all_players.lock().unwrap().register(player);
            }

            1 => {
                player.uuid = player_vec[0..16].to_vec();
                player_vec = player_vec[16..].to_vec();
                let vec = read_var_int(&mut player_vec)?;
                read_next = vec[1];
                player_vec = player_vec[read_next as usize..].to_vec();
            }

            2 => {
                player.uuid = player_vec[0..16].to_vec();
                player_vec = player_vec[16..].to_vec();
                let vec = read_var_int(&mut player_vec)?;
                let ping = vec[0];
                player.ping = ping;
                read_next = vec[1];
                player_vec = player_vec[read_next as usize..].to_vec();
                all_players.lock().unwrap().update_ping(player.uuid.clone(), ping);
            }

            3 => {
                player.uuid = player_vec[0..16].to_vec();
                player_vec = player_vec[16..].to_vec();
                let display_name: u8 = player_vec[0];
                player_vec = player_vec[1..].to_vec();
                if display_name == 0x01 {
                    let vec = read_var_int(&mut player_vec)?;
                    let display_name_size = vec[0];
                    read_next = vec[1];
                    player_vec = player_vec[display_name_size as usize + read_next as usize..].to_vec();
                }
            }

            4 => {
                player.uuid = player_vec[0..16].to_vec();
                all_players.lock().unwrap().remove_player(player.uuid.clone());
                continue;
            }

            _ => {
                unreachable!();
            }
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let mut stream =
        TcpStream::connect("VladMovi2.aternos.me:37266").expect("Could not connect to server");
    println!("Connected to server");
    let port: u16 = 37266;
    //handshake
    let handshake: Vec<u8> = handshake_packet(0x00, 757, "VladMovi2.aternos.me", port, 1)?;
    let handshake_lenght: Vec<u8> = packet_lenght(handshake.clone())?;
    stream.write_all(&handshake_lenght)?;
    stream.write_all(&handshake)?;
    //status request
    let status_request: Vec<u8> = status_request_packet(0x00)?;
    stream.write_all(&status_request)?;
    //ping request
    let ping_request: Vec<u8> = ping_request_packet()?;
    stream.write_all(&ping_request)?;
    //status and pong response
    read_packets(&stream)?;
    //conection 2
    stream.shutdown(std::net::Shutdown::Both)?;
    stream = TcpStream::connect("VladMovi2.aternos.me:37266").expect("Could not connect to server");
    println!("Connected to server for login");
    //handshake next state = 2
    let handshake2: Vec<u8> = handshake_packet(0x00, 757, "VladMovi2.aternos.me", port, 2)?;
    let handshake_lenght2: Vec<u8> = packet_lenght(handshake2.clone())?;
    stream.write_all(&handshake_lenght2)?;
    stream.write_all(&handshake2)?;
    //login request
    let login_request: Vec<u8> = login_request()?;
    stream.write_all(&login_request)?;
    //set compression
    set_compression(&stream)?;
    //login success
    let write_stream = stream.try_clone()?;
    let all_players: Arc<Mutex<Players>> = Arc::new(Mutex::new(Players::new()));
    let players1 = Arc::clone(&all_players);
    let thread1 = thread::spawn(move || {
        f1(&stream, players1).unwrap();
    });

    let players2 = Arc::clone(&all_players);
    let thread2 = thread::spawn(move || {

        f2(&write_stream, players2).unwrap();
    });

    thread1.join().unwrap();
    thread2.join().unwrap();
    println!("Logged in");
    Ok(())
}
