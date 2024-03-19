use std::io;
use colored::{ColoredString, Colorize};
use crate::read_json_from_file;
use std::collections::HashMap;


fn minecraft_to_ansi(s: String) -> String {

    let mut color_codes = HashMap::new();
    color_codes.insert('0', "\x1b[30m");
    color_codes.insert('1', "\x1b[34m");
    color_codes.insert('2', "\x1b[32m");
    color_codes.insert('3', "\x1b[36m");
    color_codes.insert('4', "\x1b[31m");
    color_codes.insert('5', "\x1b[35m");
    color_codes.insert('6', "\x1b[33m");
    color_codes.insert('7', "\x1b[37m");
    color_codes.insert('8', "\x1b[90m");
    color_codes.insert('9', "\x1b[94m");
    color_codes.insert('a', "\x1b[92m");
    color_codes.insert('b', "\x1b[96m");
    color_codes.insert('c', "\x1b[91m");
    color_codes.insert('d', "\x1b[95m");
    color_codes.insert('e', "\x1b[93m");
    color_codes.insert('f', "\x1b[97m");
    color_codes.insert('r', "\x1b[0m");
    color_codes.insert('l', "\x1b[1m");
    color_codes.insert('o', "\x1b[3m");
    color_codes.insert('n', "\x1b[4m");
    color_codes.insert('m', "\x1b[9m");

    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '§' {
            if let Some(code) = chars.peek().and_then(|c| color_codes.get(c)) {
                result.push_str(code);
                let _ = chars.next();
            }
        } else {
            result.push(ch);
        }
    }

    result
}
//


fn hex_to_rgb(hex: &str) -> Result<[u8; 3], &str> {
    if hex.len() != 6 {
        return Err("Invalid hexadecimal color code");
    }

    let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap();

    Ok([r, g, b])
}

pub fn mc_colors(color: &str) -> ([u8;3], [u8;3]) {
    match color {
        "black" => ([0, 0, 0], [0, 0, 0]),
        "dark_blue" => ([0, 0, 170], [0, 0, 42]),
        "dark_green" => ([0, 170, 0], [0, 42, 0]),
        "dark_aqua" => ([0, 170, 170], [0, 42, 42]),
        "dark_red" => ([170, 0, 0], [42, 0, 0]),
        "dark_purple" => ([170, 0, 170], [42, 0, 42]),
        "gold" => ([255, 170, 0], [63, 42, 0]),
        "gray" => ([170, 170, 170], [42, 42, 42]),
        "dark_gray" => ([85, 85, 85], [21, 21, 21]),
        "blue" => ([85, 85, 255], [21, 21, 63]),
        "green" => ([85, 255, 85], [21, 63, 21]),
        "aqua" => ([85, 255, 255], [21, 63, 63]),
        "red" => ([255, 85, 85], [63, 21, 21]),
        "light_purple" => ([255, 85, 255], [63, 21, 63]),
        "yellow" => ([255, 255, 85], [63, 63, 21]),
        "white" => ([255, 255, 255], [63, 63, 63]),
        _ => {
            if color.starts_with('#') {
                let mut rgb = [0; 3];
                if let Some(stripped) = color.strip_prefix('#') {
                    let hex = stripped;
                    rgb = hex_to_rgb(hex).unwrap();
                }
                (rgb, [rgb[0]/4, rgb[1]/4, rgb[2]/4])
            }
            else {
                ([255, 255, 255], [63, 63, 63])
            
            }
        }
    }
}

#[derive(Clone)]
pub struct Styles {
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
    foreground: [u8; 3],
    background: [u8; 3],
}

impl Styles {
    pub fn new() -> Styles {
        Styles {
            bold: false,
            italic: false,
            underlined: false,
            strikethrough: false,
            foreground: [255, 255, 255],
            background: [63, 63, 63],
        }
    }

    pub fn copy_from(&mut self, other: &Styles) {
        self.bold = other.bold;
        self.italic = other.italic;
        self.underlined = other.underlined;
        self.strikethrough = other.strikethrough;
        self.foreground = other.foreground;
        self.background = other.background;
    }

    pub fn from_styles(inherited_format: &Styles) -> Styles {
        let mut text_format = Styles::new();
        text_format.copy_from(inherited_format);
        text_format
    }

    pub fn from_obj(json_obj: &serde_json::Value, inherited_format: &Styles)  -> Styles {
        let mut text_format = Styles::from_styles(inherited_format);
        let content = json_obj.as_object().unwrap();

        if content.contains_key("color") {
            let color = content["color"].as_str().unwrap();
            let (fg, bg) = mc_colors(color);
            text_format.foreground = fg;
            text_format.background = bg;
        }

        if content.contains_key("bold") && content["bold"].as_bool().unwrap(){
            text_format.bold = true;
        }

        if content.contains_key("italic") && content["italic"].as_bool().unwrap() {
            text_format.italic = true;
        }

        if content.contains_key("underlined") && content["underlined"].as_bool().unwrap(){
            text_format.underlined = true;
        }

        if content.contains_key("strikethrough") && content["strikethrough"].as_bool().unwrap() {
            text_format.strikethrough = true;
        }

        text_format
    }
}

pub fn print_string(text: String, text_format: &Styles) {
    let string = minecraft_to_ansi(text);
    let mut colorised: ColoredString = string.normal();

    if text_format.bold {
        colorised = colorised.bold();
    }

    if text_format.italic {
        colorised = colorised.italic();
    }

    if text_format.underlined {
        colorised = colorised.underline();
    }

    if text_format.strikethrough {
        colorised = colorised.strikethrough();
    }

    colorised = colorised.truecolor(text_format.foreground[0], text_format.foreground[1], text_format.foreground[2]).on_truecolor(text_format.background[0], text_format.background[1], text_format.background[2]);

    print!("{}", colorised);
}

pub fn parse_json_obj(json_obj: serde_json::Value, inherited_format: Styles) -> io::Result<()> {
    let content = json_obj.as_object().unwrap();
    let text_format = Styles::from_obj(&json_obj, &inherited_format);
    if content.contains_key("text") && content["text"].is_string() {
        print_string(content["text"].to_string(), &text_format.clone());
    }

    if content.contains_key("translate") && content["translate"].is_string() {
        let translate_msg = translate(content["translate"].as_str().unwrap(), content["with"].clone(), text_format.clone())?;
        print_string(translate_msg, &text_format.clone());
    }

    if content.contains_key("extra") {
        if content["extra"].is_object(){
             parse_json_obj(content["extra"].clone(), text_format.clone())?;
        }
        if content["extra"].is_array() {
            parse_json_array(content["extra"].clone(), text_format.clone())?;
        }
    }
    Ok(())
}

pub fn parse_json_array(json_array: serde_json::Value, inherited_format: Styles) -> io::Result<()> {

    let content = json_array.as_array().unwrap();
    let text_format: Styles = Styles::from_styles(&inherited_format);
    for item in content {
        if item.is_object() {
            parse_json_obj(item.clone(), text_format.clone())?;
        }
        if item.is_array() {
            parse_json_array(item.clone(), text_format.clone())?;
        }
        if item.is_string() {
            let msg = item.as_str().unwrap().to_string();
            print_string(msg, &text_format.clone());
        }
    }

    Ok(())
}

fn replace_placeholders(message: &str, replacements: &mut [String]) -> String {
    let mut replaced_message = message.to_string();
    let mut index = 0;

    while let Some(pos) = replaced_message.find("%s") {
        if let Some(replacement) = replacements.get(index) {
            replaced_message.replace_range(pos..pos+2, replacement);
            index += 1;
        } else {
            break;
        }
    }

    for (i, replacement) in replacements.iter().enumerate() {
        let placeholder = format!("%{}$s", i + 1);
        replaced_message = replaced_message.replace(&placeholder, replacement);
    }

    replaced_message = replaced_message.replace("%%", "%");

    replaced_message
}

pub fn translate (translation: &str, with: serde_json::Value, inherited_format: Styles) -> io::Result<String> {
    let translate_file = read_json_from_file("src/translations")?;
    let content = translate_file.as_object().unwrap();
    if !content.contains_key(translation) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Translation not found."));
    }

    let mut translated_message = String::new();

    if content[translation].is_string() {
       translated_message = content[translation].as_str().unwrap().to_string();
    }

    let mut with_vec = Vec::<String>::new();

    if with.is_object() {
        with_vec = parse_with_obj(with, inherited_format)?;
    }
    else if with.is_array() {
        with_vec = parse_with_array(with, inherited_format.clone())?;
    }
    else if with.is_string() {
        let string = with.as_str().unwrap().to_string();
        with_vec.push(string);
    }

    let message = replace_placeholders(&translated_message, &mut with_vec);

    Ok(message)
}

pub fn parse_with_obj(with: serde_json::Value, inherited_format: Styles) -> io::Result<Vec<String>> {
    let content = with.as_object().unwrap();
    let mut text_vec = Vec::<String>::new();
    if content.contains_key("text") && content["text"].is_string() {
        let string = content["text"].as_str().unwrap().to_string();
        text_vec.push(string);
    }

    if content.contains_key("extra") {
        if content["extra"].is_object(){
            let mut vec = parse_with_obj(content["extra"].clone(), inherited_format.clone())?;
            text_vec.append(&mut vec);
        }
        if content["extra"].is_array() {
            let mut vec = parse_with_array(content["extra"].clone(), inherited_format.clone())?;
            text_vec.append(&mut vec);
        }
    }
 
    Ok(text_vec)
}

pub fn parse_with_array(with: serde_json::Value, inherited_format: Styles) -> io::Result<Vec<String>> {
    let content = with.as_array().unwrap();
    let mut text_vec = Vec::<String>::new();
    let text_format: Styles = Styles::from_styles(&inherited_format);

    for item in content {
        if item.is_object() {
            let msg = parse_with_obj(item.clone(), text_format.clone());
            text_vec.append(&mut msg?);
        }
        if item.is_array() {
            let msg = parse_with_array(item.clone(), text_format.clone());
            text_vec.append(&mut msg?);
        }
        if item.is_string() {
            let msg = item.as_str().unwrap().to_string();
            text_vec.push(msg);
        }
    }

    Ok(text_vec)
}