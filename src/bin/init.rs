use std::path::Path;

use glyphmatcher::{FontDb};

fn main() {
    let db = FontDb::new(Path::new("db"));
    db.scan();
}