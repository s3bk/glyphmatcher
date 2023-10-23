use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}, fmt::Display, sync::{RwLock, Arc}};

use font::{TrueTypeFont, CffFont, OpenTypeFont, opentype::cmap::CMap, GlyphId, Glyph, Font};
use istring::SmallString;
use itertools::Itertools;
use pathfinder_content::outline::{Contour, Outline};
use pathfinder_geometry::vector::Vector2F;
use pdf_encoding::glyphname_to_unicode;
use serde::{Deserialize, Serialize};

use crate::frechet::frechet_distance;

pub mod frechet;

#[derive(Serialize, Deserialize)]
struct Entry<I> {
    contour_sets: Vec<HashSet<(u16, u16)>>,
   // outline: Outline,
    data: I,
}
 
#[derive(Serialize, Deserialize)]
pub struct ShapeDb<I> {
    entries: Vec<Entry<I>>,
    points: HashMap<(u16, u16), Vec<usize>>,
}
impl<I> ShapeDb<I> {
    pub fn new() -> Self {
        ShapeDb {
            entries: vec![],
            points: HashMap::new(),
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
            font_file.file_stem().unwrap().to_str().unwrap()
        }
    };
    let use_name = font_file.extension().map(|s| s == "name").unwrap_or(false);

    let mut db = ShapeDb::new();

    let label_file = Path::new("unicode").join(font_file.file_stem().unwrap()).with_extension("json");
    let list = if label_file.exists() {
        println!("loading patch");
        let list: UnicodeList = serde_json::from_slice(&std::fs::read(&label_file).unwrap()).unwrap();
        Some(list.into_iter().map(|e| (GlyphId(e.gid), e.unicode.iter().flat_map(|&n| std::char::from_u32(n)).collect())).collect())
    } else {
        font_uni_list(&*font, use_name)
    };
    if let Some(list) = list {

        for (gid, s) in list {
            let g = font.glyph(gid).unwrap();
            
            db.add_outline(&g.path, s);
        }

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

pub fn font_uni_list(font: &(dyn Font + Sync + Send), use_name: bool) -> Option<Vec<(GlyphId, SmallString)>> {
    if let Some(ttf) = font.downcast_ref::<TrueTypeFont>() {
        println!("TTF");
        if let Some(ref cmap) = ttf.cmap {
            Some(use_cmap(cmap))
        } else {
            None
        }
    } else if let Some(cff) = font.downcast_ref::<CffFont>() {
        println!("CFF");
        None
    } else if let Some(otf) = font.downcast_ref::<OpenTypeFont>() {
        println!("OTF");
        if use_name && otf.name_map.len() > 0 {
            Some(use_name_map(&otf.name_map))
        }
        else if let Some(ref cmap) = otf.cmap {
            Some(use_cmap(cmap))
        } else {
            None
        }
    } else {
        None
    }
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

impl<I: Display + PartialEq> ShapeDb<I> {
    pub fn add_outline(&mut self, outline: &Outline, value: I) {
        let val_idx = self.entries.len();
        let mut points_seen = HashSet::new();
        for c in outline.contours().iter() {
            for &p in c.points().iter() {
                let key = (p.x() as u16, p.y() as u16);
                if points_seen.insert(key) {
                    self.points.entry(key).or_default().push(val_idx);
                }
            }
        }
        let contours = outline.contours().iter().map(points_set).collect();
        self.entries.push(Entry { data: value, contour_sets: contours });
    }
    pub fn get(&self, outline: &pathfinder_content::outline::Outline, mut report: Option<&mut String>) -> Option<&I> {
        use std::fmt::Write;

        let mut candiates: HashMap<usize, usize> = HashMap::new();
        let mut points_seen = HashSet::new();

        for c in outline.contours().iter() {
            for &p in c.points().iter() {
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
            let e = &self.entries[idx];
            if let Some(report) = report.as_deref_mut() {
                writeln!(report, "<div>candiate <span>{}</span>", e.data);
            };
            if e.contour_sets.len() != outline.contours().len() {
                if let Some(report) = report.as_deref_mut() {
                    writeln!(report, " incorrect number of contours {} != {}</div>", e.contour_sets.len(), outline.contours().len());
                }
                continue;
            }

            let mut used = vec![false; outline.contours().len()];
            for t_c in outline.contours().iter() {
                let t_s = points_set(t_c);
                for (r_c_i, r_s) in e.contour_sets.iter().enumerate() {
                    if used[r_c_i] {
                        continue;
                    }

                    if t_s == *r_s {
                        used[r_c_i] = true;
                    }
                }
            }

            if used.iter().all(|&b| b) {
                if let Some(report) = report.as_deref_mut() {
                    writeln!(report, "<p>Unicode: <span>{}</span>, {used:?}</p>", e.data).unwrap();
                    writeln!(report, "</div>");
                }
                return Some(&e.data);
            }
        }
        /*
        let mut best_entry = None;
        for e in self.entries.iter() {
            let n = outline.0.len();
            let mut score = vec![0.0; n * n];

            if e.outline.0.len() != outline.0.len() {
                continue;
            }
            for (t_i, t_c) in outline.0.iter().enumerate() {
                for (r_i, r_c) in e.outline.0.iter().enumerate() {
                    score[t_i * n + r_i] = frechet_distance(t_c, r_c);
                }
            }

            let sum: f32 = score.windows(n).map(|w| w.iter().cloned().reduce(|a, b| min(a, b)).unwrap()).sum();

            if let Some(report) = report.as_deref_mut() {
                if debug.as_ref().map(|d| *d == e.data).unwrap_or(false) {
                    for w in score.windows(n) {
                        for s in w {
                            print!("  {s:5.0}");
                        }
                        println!();
                    }
                }

                if sum < 100. * n as f32 {
                    writeln!(report, "<div><span>{}</span>: {sum}</div>", e.data);
                }
            }
            
            match best_entry {
                Some((_, s2)) if sum >= s2 => {}
                _ => {
                    best_entry = Some((e, sum));
                    //println!("{}", score.iter().format(", "));
                }
            }
        }
        if let Some((e, sum)) = best_entry {
            if let Some(report) = report.as_deref_mut() {
                writeln!(report, "<p>Best match: <span>{}</span>, {sum}</p>", e.data).unwrap();
            }
            return Some(&e.data);
        }
        */

        None
    }
}

fn points_set(contour: &Contour) -> HashSet<(u16, u16)> {
    contour.points().iter().map(|&p| (p.x() as u16, p.y() as u16)).collect()
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
                        writeln!(report, r#"<div class="test">Glyph {i}"#).unwrap();
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

fn write_glyph(w: &mut String, path: &pathfinder_content::outline::Outline) {
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

pub fn max(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}
pub fn min(a: f32, b: f32) -> f32 {
    if a > b { b } else { a }
}

#[derive(Serialize, Deserialize)]
pub struct UnicodeEntry {
    pub gid: u32,
    pub unicode: Vec<u32>,
}
pub type UnicodeList = Vec<UnicodeEntry>;
