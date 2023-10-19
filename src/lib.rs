use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}, fmt::Display, sync::{RwLock, Arc}};

use font::{TrueTypeFont, CffFont, OpenTypeFont, opentype::cmap::CMap, GlyphId, Glyph, Font};
use istring::SmallString;
use pathfinder_content::outline::{Outline, Contour};
use pdf_encoding::glyphname_to_unicode;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ShapeDb<I> {
    entries: Vec<(I, Vec<HashSet<(u16, u16)>>)>,
    points: HashMap<(u16, u16), Vec<usize>>,
}
impl<I> ShapeDb<I> {
    pub fn new() -> Self {
        ShapeDb {
            entries: vec![],
            points: HashMap::new()
        }
    }
}

fn add_font(db_dir: &Path, font_file: &Path) {
    let data = std::fs::read(&font_file).unwrap();
    let font = font::parse(&data).unwrap();
    let ps_name = match dbg!(&font.name().postscript_name) {
        Some(ref n) => n,
        None => {
            println!("no postscript name");
            return;
        }
    };
    if let Some(db) = read_font(&*font) {
        let db_data = postcard::to_allocvec(&db).unwrap();
        std::fs::write(db_dir.join(ps_name), &db_data).unwrap();
    }
}

pub fn init(db_dir: &Path) {
    for e in std::fs::read_dir("fonts").unwrap().filter_map(|r| r.ok()) {
        let path = e.path();
        println!("{path:?}");
        add_font(db_dir, &path);
    }
}
pub fn read_font(font: &(dyn Font + Sync + Send)) -> Option<ShapeDb<SmallString>> {
    let mut db = ShapeDb::new();

    let list = if let Some(ttf) = font.downcast_ref::<TrueTypeFont>() {
        println!("TTF");
        if let Some(ref cmap) = ttf.cmap {
            use_cmap(cmap)
        } else {
            return None;
        }
    } else if let Some(cff) = font.downcast_ref::<CffFont>() {
        println!("CFF");
        return None;
    } else if let Some(otf) = font.downcast_ref::<OpenTypeFont>() {
        println!("OTF");
        if otf.name_map.len() > 0 {
            use_name_map(&otf.name_map)
        }
        else if let Some(ref cmap) = otf.cmap {
            use_cmap(cmap)
        } else {
            return None;
        }
    } else {
        return None;
    };

    for (gid, s) in list {
        let g = font.glyph(gid).unwrap();
        db.add_outline(g.path, s);
    }

    Some(db)
}

fn use_cmap(cmap: &CMap) -> Vec<(GlyphId, SmallString)> {
    let mut v = Vec::new();
    for (uni, gid) in cmap.items() {
        if let Some(c) = char::from_u32(uni) {
            v.push((gid, c.into()));
        };
    }
    v
}
fn use_name_map(map: &HashMap<String, u16>) -> Vec<(GlyphId, SmallString)> {
    let mut v = vec![];
    for (name, &id) in map.iter() {
        if let Some(s) = glyphname_to_unicode(&name) {
            v.push((GlyphId(id as u32), s.into()));
        } else if let Some(uni) = name.strip_prefix("uni").and_then(|hex| u32::from_str_radix(hex, 16).ok()).and_then(std::char::from_u32) {
            v.push((GlyphId(id as u32), uni.into()));
        } else {
            println!("not found: {name}");
        }
    }
    v
}

impl<I: Display> ShapeDb<I> {
    pub fn add_outline(&mut self, outline: Outline, value: I) {
        let val_idx = self.entries.len();
        let mut points_seen = HashSet::new();
        for c in outline.contours() {
            for p in c.points() {
                let key = (p.x() as u16, p.y() as u16);
                if points_seen.insert(key) {
                    self.points.entry(key).or_default().push(val_idx);
                }
            }
        }
        let contours = outline.contours().iter().map(points_set).collect();
        self.entries.push((value, contours));
    }
    pub fn get(&self, outline: &Outline, mut report: Option<&mut String>) -> Option<&I> {
        use std::fmt::Write;

        let mut candiates: HashMap<usize, usize> = HashMap::new();
        let mut points_seen = HashSet::new();

        for c in outline.contours() {
            for p in c.points() {
                let key = (p.x() as u16, p.y() as u16);

                if points_seen.insert(key) {
                    if let Some(list) = self.points.get(&key) {
                        for &idx in list {
                            *candiates.entry(idx).or_default() += 1;
                        }
                    }
                }
            }
        }
        let mut candiates: Vec<_> = candiates.into_iter().collect();
        candiates.sort_by_key(|t| t.1);
        
        for &(idx, n) in candiates.iter().rev() {
            let (ref s, ref contours) = self.entries[idx];
            if let Some(report) = report.as_deref_mut() {
                writeln!(report, "<div>candiate <span>{s}</span>");
            };
            if contours.len() != outline.contours().len() {
                if let Some(report) = report.as_deref_mut() {
                    writeln!(report, " incorrect number of contours {} != {}</div>", contours.len(), outline.len());
                }
                continue;
            }

            let mut used = vec![false; contours.len()];
            for t_c in outline.contours().iter() {
                let t_s = points_set(t_c);
                for (r_c_i, r_s) in contours.iter().enumerate() {
                    if used[r_c_i] {
                        continue;
                    }

                    if t_s == *r_s {
                        used[r_c_i] = true;
                    } else {
                        if let Some(report) = report.as_deref_mut() {
                            let i = t_s.difference(r_s).count();
                            writeln!(report, " {} of {} points do not match", i, t_s.len());
                        }
                    }
                }
            }

            if let Some(report) = report.as_deref_mut() {
                writeln!(report, "<p>Unicode: <span>{s}</span>, {used:?}</p></div>").unwrap();
            }
            if used.iter().all(|&b| b) {
                return Some(s);
            }
        }
        None
    }
}

fn points_set(contour: &Contour) -> HashSet<(u16, u16)> {
    contour.points().iter().map(|p| (p.x() as u16, p.y() as u16)).collect()
}

pub fn check_font(db: &ShapeDb<SmallString>, ps_name: &str, font: &(dyn Font + Sync + Send), mut report: Option<&mut String>) -> Option<HashMap<GlyphId, SmallString>> {
    use std::fmt::Write;

    if let Some(report) = report.as_deref_mut() {
        report.push_str(r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8">
<style type="text/css">
.test {
    margin-bottom: 1em;
}
.candidate {
    display: flex;
    margin-left: 2em;
}
svg {
    border: 1px solid blue;
}
p > span {
    font-size: 40pt;
}
</style>
</head>
<body>
"#);
    }

    let mut map = HashMap::new();

    for i in 0 .. font.num_glyphs() {
        if let Some(g) = font.glyph(GlyphId(i)) {
            if g.path.len() > 0 {
                if g.path.len() > 0 {
                    if let Some(report) = report.as_deref_mut() {
                        writeln!(report, r#"<div class="test">"#).unwrap();
                        write_glyph(report, &g.path);
                    }
                    if let Some(s) = db.get(&g.path, report.as_deref_mut()) {
                        map.insert(GlyphId(i), s.clone());
                    }
                    if let Some(report) = report.as_deref_mut() {
                        writeln!(report, "</div>").unwrap();
                    }
                }
            }
        }
    }

    if let Some(report) = report.as_deref_mut() {
        report.push_str("</body></html>");
    }

    Some(map)
}

fn write_glyph(w: &mut String, path: &Outline) {
    use std::fmt::Write;

    let b = path.bounds();
    writeln!(w, r#"<svg viewBox="{} {} {} {}" transform="scale(1, -1)" style="display: inline-block;" width="{}px"><path d="{:?}" /></svg>"#, b.min_x(), b.min_y(), b.width(), b.height(), b.width() * 0.05, path, ).unwrap();
}

pub struct FontDb {
    path: PathBuf,
    cache: RwLock<HashMap<String, Option<Arc<ShapeDb<SmallString>>>>>,
}
impl FontDb {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        FontDb { path: path.into(), cache: Default::default() }
    }
    pub fn scan(&self) {
        init(&self.path)
    }
    fn get_db(&self, ps_name: &str) -> Option<Arc<ShapeDb<SmallString>>> {
        if let Some(cached) = self.cache.read().unwrap().get(ps_name) {
            return cached.clone();
        }

        let file_path = self.path.join(ps_name);
        let db = if file_path.is_file() {
            Some(Arc::new(postcard::from_bytes(&std::fs::read(&file_path).unwrap()).unwrap()))
        } else {
            None
        };
        self.cache.write().unwrap().insert(ps_name.into(), db.clone());
        db
    }
    pub fn font_report(&self, ps_name: &str, font: &(dyn Font + Sync + Send)) -> String {
        let mut report = String::new();
        let db = self.get_db(ps_name).unwrap();
        check_font(&db, ps_name, font, Some(&mut report));
        report
    }
    pub fn check_font(&self, ps_name: &str, font: &(dyn Font + Sync + Send)) -> Option<Arc<HashMap<GlyphId, SmallString>>> {
        let db = self.get_db(ps_name)?;
        let out = check_font(&db, ps_name, font, None).map(Arc::new);
        out
    }
    pub fn add_font(&self, font_path: &Path) {
        add_font(&self.path, font_path)
    }
}
