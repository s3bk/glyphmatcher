use std::{path::Path, fs::read_dir};

use glyphmatcher::{font_uni_list, UnicodeEntry, UnicodeList};
use serde::{Serialize, Deserialize};

fn main() {
    let fonts_path = Path::new("fonts");
    let unicode_path = Path::new("unicode");

    for e in read_dir(fonts_path).unwrap().filter_map(Result::ok) {
        let path = e.path();
        let uni_path = unicode_path.join(path.file_stem().unwrap()).with_extension("json");
        if uni_path.exists() {
            continue;
        }
        
        let use_name = path.extension().map(|s| s == "name").unwrap_or(false);

        let data = std::fs::read(&path).unwrap();
        if let Ok(font) = font::parse(&data) {
            if let Some(name_list) = font_uni_list(&*font, use_name) {
                let name_list: UnicodeList = name_list.into_iter()
                    .map(|(gid, uni)| UnicodeEntry { gid: gid.0, unicode: uni.chars().map(|c| c as u32).collect() })
                    .collect();
                std::fs::write(uni_path, serde_json::to_string_pretty(&name_list).unwrap()).unwrap();
            }
        }
    }
}
