use std::path::{Path, PathBuf};

use glyphmatcher::{FontDb};


fn main() {
    let db = FontDb::new("db");
    let path = PathBuf::from(std::env::args_os().nth(1).unwrap());

    let data = std::fs::read(&path).unwrap();
    
    let font = font::parse(&data).unwrap();
    let ps_name: &str = font.name().postscript_name.as_deref().unwrap_or_else(|| {
        path.file_name().unwrap().to_str().unwrap()
    }).split("+").nth(1).unwrap();
    println!("name: {:?}", ps_name);
    let report = db.font_report(ps_name, &*font);

    std::fs::write(path.with_extension("html"), report).unwrap();
}