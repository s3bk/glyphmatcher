use std::path::{Path, PathBuf};

use glyphmatcher::{FontDb};


fn main() {
    let db = FontDb::new("db");
    let path = PathBuf::from(std::env::args_os().nth(1).unwrap());
    let mut report = String::new();

    let data = std::fs::read(&path).unwrap();
    
    let font = font::parse(&data).unwrap();
    let ps_name: &str = font.name().postscript_name.as_deref().unwrap_or_else(|| {
        path.file_name().unwrap().to_str().unwrap().split("+").nth(1).unwrap()
    });
    let glyphs = db.check_font(ps_name, &*font).unwrap();
    for (k, v) in glyphs.iter() {
        println!("{} -> {:?}", k.0, v);
    }

    std::fs::write(path.with_extension("html"), report).unwrap();
}