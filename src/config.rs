use std::str::FromStr;
use ini::Ini;

pub struct Config {
    pub hotkey: u32,
    pub lower_players: u8,
    pub upper_players: u8,
}

impl Config {
    pub fn new() -> Self {
        Config {
            hotkey: 192,
            lower_players: 1,
            upper_players: 8,
        }
    }

    pub fn reload(&mut self, file_name: &str) -> &Self {
        let sections = Ini::load_from_file(file_name).unwrap();

        for (section, properties) in sections.iter() {
            match section.unwrap() {
                "TOGGLE PLAYERS" => {
                    for (k, v) in properties.iter() {
                        match k {
                            "hotkey" => self.hotkey = u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap(),
                            "lower_players" => self.lower_players = u8::from_str(v).unwrap(),
                            "upper_players" => self.upper_players = u8::from_str(v).unwrap(),
                            _ => (),
                        }
                    }
                },
                _ => (),
            }
        }

        self
    }
}