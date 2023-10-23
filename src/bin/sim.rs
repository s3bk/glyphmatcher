use std::{fs::File, io::{BufWriter, Write}, path::Path, collections::HashMap};

use font::{Glyph, Font};
use glyphmatcher::{frechet::frechet_distance, min, UnicodeList, UnicodeEntry};
use pathfinder_content::outline::Outline;

fn read_font(path: &str) -> Box<dyn Font + Sync + Send> {
    let data = std::fs::read(path).unwrap();
    font::parse(&data).unwrap()
}
fn shapes(font: &dyn Font) -> Vec<(u32, Glyph)> {
    (1 .. font.num_glyphs()).filter_map(|n| Some((n, font.glyph(font::GlyphId(n))?))).filter(|(_, g)| g.path.len() > 0).collect()
}

fn read_uni(path: &str) -> HashMap<u32, Vec<u32>> {
    let data = std::fs::read(path).unwrap();
    serde_json::from_slice::<UnicodeList>(&data).unwrap().into_iter().map(|u| (u.gid, u.unicode)).collect()
}

fn main() {
    let font_a = read_font("fonts/axtmanalblack.ttf");
    let font_b = read_font("fonts/AXtManalBold2.ttf");
    let uni_b = read_uni("unicode/AXtManalBold2.json");
    
    let mut report = BufWriter::new(File::create("report.html").unwrap());

    report.write_all(br#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8">
<style type="text/css">
div {
    display: flex;
    padding-top: 1em;
}
div > div {
    display: flex;
    flex-direction: column;
    padding-left: 1em;
}
svg {
    border: 1px solid skyblue;
    padding: 2px;
}
</style>
</head>
<body>
"#).unwrap();

    let shapes_a: Vec<_> = shapes(&*font_a);
    let shapes_b: Vec<_> = shapes(&*font_b);

    let mut uni_a: UnicodeList = vec![];

    for &(a_gid, ref a) in shapes_a.iter() {
        println!("{a_gid}");
        write_glyph(&mut report, &a.path);

        let mut dists = vec![];
        for (b_gid, b) in shapes_b.iter() {
            dists.push((b_gid, glyph_similarity(a, b)));
        }
        dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        for &(b_gid, score) in dists.iter().take(1) {
            if let Some(uni) = uni_b.get(b_gid) {
                uni_a.push(UnicodeEntry { gid: a_gid, unicode: uni.clone() });                
            }
        }
    }

    std::fs::write(
        "unicode/axtmanalblack.json",
        &serde_json::to_string(&uni_a).unwrap()
    ).unwrap();
}

fn glyph_similarity(a: &Glyph, b: &Glyph) -> f32 {
    let mut dists = vec![];
    for (t_i, t_c) in a.path.contours().iter().enumerate() {
        let mut min_score = f32::INFINITY;
        for (r_i, r_c) in b.path.contours().iter().enumerate() {
            min_score = min(min_score, frechet_distance(t_c, r_c));
        }
        dists.push(min_score);
    }

    dists.iter().sum::<f32>() / (dists.len() as f32)
}

fn write_glyph(w: &mut impl Write, path: &Outline) {
    let b = path.bounds();
    writeln!(w, r#"<svg viewBox="{} {} {} {}" transform="scale(1, -1)" style="display: inline-block;" width="{}px"><path d="{:?}" /></svg>"#, b.min_x(), b.min_y(), b.width(), b.height(), b.width() * 0.05, path, ).unwrap();
}