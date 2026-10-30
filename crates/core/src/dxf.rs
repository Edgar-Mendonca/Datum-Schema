//! The DXF writer: ASCII DXF, R12-compatible (AC1009), so the drawing opens in any CAD program.
//! One layer per line type and per symbol category; symbols as blocks with their tag as an
//! attribute; text as text; the border and title block included. Sheet millimetres, y up (the
//! sheet's bottom left corner is the origin). Everything is drawn from the same primitives the
//! window shows (`view.rs`, `frame.rs`, `symbols.rs`).

use crate::graph::Net;
use crate::hydraulics;
use crate::model::{Kind, Project, Sheet};
use crate::symbols::{self, Cat, Prim};
use crate::view;
use std::collections::BTreeSet;
use std::fmt::Write;

struct W {
    s: String,
    h: f64,
}

impl W {
    fn g(&mut self, code: i32, v: impl std::fmt::Display) {
        let _ = write!(self.s, "{code:>3}\r\n{v}\r\n");
    }
    fn f(&mut self, code: i32, v: f64) {
        self.g(code, crate::fmt_num((v * 1e4).round() / 1e4));
    }
    fn xy(&mut self, c: i32, p: [f64; 2], flip: bool) {
        self.f(10 + c, p[0]);
        self.f(20 + c, if flip { self.h - p[1] } else { -p[1] });
    }
    /// One primitive. `sheet` true: y = sheet height − y (entities); false: y = −y (inside blocks).
    fn prim(&mut self, p: &Prim, layer: &str, sheet: bool, dash: bool) {
        match p {
            Prim::Poly { pts, closed, fill, dash: d, .. } => {
                if *fill && (3..=4).contains(&pts.len()) {
                    self.g(0, "SOLID");
                    self.g(8, layer);
                    let q = [pts[0], pts[1], pts[pts.len() - 1], pts[2]];
                    for (i, pt) in q.iter().enumerate() {
                        self.xy(i as i32, *pt, sheet);
                    }
                    return;
                }
                if pts.len() < 2 {
                    return;
                }
                self.g(0, "POLYLINE");
                self.g(8, layer);
                if *d || dash {
                    self.g(6, "DASHED");
                }
                self.g(66, 1);
                self.g(70, if *closed { 1 } else { 0 });
                self.xy(0, [0.0, if sheet { self.h } else { 0.0 }], sheet);
                for q in pts {
                    self.g(0, "VERTEX");
                    self.g(8, layer);
                    self.xy(0, *q, sheet);
                }
                self.g(0, "SEQEND");
                self.g(8, layer);
            }
            Prim::Circle { c, r, .. } => {
                self.g(0, "CIRCLE");
                self.g(8, layer);
                self.xy(0, *c, sheet);
                self.f(40, *r);
            }
            Prim::Arc { c, r, a0, a1, .. } => {
                self.g(0, "ARC");
                self.g(8, layer);
                self.xy(0, *c, sheet);
                self.f(40, *r);
                // Clockwise from a0 to a1 with y down is counter-clockwise from −a1 to −a0 with y up.
                self.f(50, (-a1).rem_euclid(360.0));
                self.f(51, (-a0).rem_euclid(360.0));
            }
            Prim::Text { p, h, s, anchor, .. } => self.text(*p, *h, s, *anchor, layer, sheet, None),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, p: [f64; 2], h: f64, s: &str, anchor: u8, layer: &str, sheet: bool, attrib: Option<(&str, bool)>) {
        self.g(0, if attrib.is_some() { "ATTRIB" } else { "TEXT" });
        self.g(8, layer);
        self.xy(0, p, sheet);
        self.f(40, h);
        self.g(1, clean(s));
        if let Some((tag, hidden)) = attrib {
            self.g(2, tag);
            self.g(70, if hidden { 1 } else { 0 });
        }
        self.g(72, anchor.min(2));
        self.xy(1, p, sheet);
        if attrib.is_some() {
            self.g(74, 2);
        } else {
            self.g(73, 2);
        }
    }
}

/// Text a DXF file can hold: no control characters; characters beyond ASCII as \U+XXXX.
fn clean(s: &str) -> String {
    // The hydraulic badges as plain letters: every reader shows them, whatever its fonts.
    match s {
        "✓" => return "OK".into(),
        "✗" => return "X".into(),
        _ => {}
    }
    let mut o = String::new();
    for c in s.chars() {
        match c {
            c if (c as u32) < 0x20 => o.push(' '),
            c if c.is_ascii() => o.push(c),
            c if (c as u32) <= 0xFFFF => {
                let _ = write!(o, "\\U+{:04X}", c as u32);
            }
            _ => o.push('?'),
        }
    }
    o
}

pub fn line_layer(k: Kind) -> String {
    format!("LINE-{}", k.key().to_uppercase())
}
pub fn sym_layer(c: Cat) -> String {
    format!("SYM-{:?}", c).to_uppercase()
}
fn block(key: &str) -> String {
    format!("SYM_{}", key.to_uppercase())
}

/// The DXF of one sheet.
pub fn write(p: &Project, s: &Sheet) -> String {
    let net = Net::new(p);
    let hyd = hydraulics::all(p, &net);
    let v = view::sheet(p, s, &net, &hyd);
    let mut w = W { s: String::with_capacity(1 << 16), h: v.h };
    let syms: BTreeSet<&str> = v.items.iter().map(|i| i.sym.as_str()).chain(v.legend_rows.iter().filter_map(|r| r.sym.as_deref())).collect();
    let mut layers: Vec<String> = vec!["FRAME".into(), "TEXT".into(), "NOTES".into(), "MARKS".into()];
    layers.extend(Kind::ALL.iter().map(|k| line_layer(*k)));
    layers.extend(Cat::ALL.iter().map(|c| sym_layer(*c)));
    head(&mut w, &layers, v.w, v.h);
    // Blocks: one per symbol used, drawn round its centre, with a TAG attribute definition.
    w.g(0, "SECTION");
    w.g(2, "BLOCKS");
    for key in &syms {
        let Some(sym) = symbols::get(key) else { continue };
        w.g(0, "BLOCK");
        w.g(8, "0");
        w.g(2, block(key));
        w.g(70, 2);
        w.f(10, 0.0);
        w.f(20, 0.0);
        w.f(30, 0.0);
        w.g(3, block(key));
        // Shapes only: the symbol's letters are written upright as text, as the window shows them.
        for pr in symbols::parse(sym.draw).iter().filter(|x| !matches!(x, Prim::Text { .. })) {
            w.prim(pr, "0", false, false);
        }
        w.g(0, "ATTDEF");
        w.g(8, "0");
        w.f(10, 0.0);
        w.f(20, -sym.h / 2.0 - 3.0);
        w.f(40, 2.5);
        w.g(1, "");
        w.g(3, "Tag");
        w.g(2, "TAG");
        w.g(70, 0);
        w.g(0, "ENDBLK");
        w.g(8, "0");
    }
    w.g(0, "ENDSEC");
    // Entities.
    w.g(0, "SECTION");
    w.g(2, "ENTITIES");
    for pr in &v.frame {
        w.prim(pr, if matches!(pr, Prim::Text { .. }) { "TEXT" } else { "FRAME" }, true, false);
    }
    for l in &v.lines {
        let layer = line_layer(l.kind);
        for seg in &l.segs {
            w.prim(&Prim::Poly { pts: seg.clone(), closed: false, fill: false, dash: false, w: 0.0 }, &layer, true, l.kind == Kind::Electric);
        }
        for m in &l.marks {
            w.prim(m, if matches!(m, Prim::Text { .. }) { "TEXT" } else { &layer }, true, false);
        }
    }
    for it in &v.items {
        let layer = sym_layer(it.cat);
        w.g(0, "INSERT");
        w.g(8, &layer);
        w.g(66, 1);
        w.g(2, block(&it.sym));
        w.xy(0, [it.x, it.y], true);
        w.f(41, if it.flip { -it.scale } else { it.scale });
        w.f(42, it.scale);
        w.f(50, (-(it.rot as f64) * 90.0).rem_euclid(360.0));
        // The tag as the attribute: at its label, or hidden inside an instrument bubble (whose
        // letters and loop number are written as text).
        let label = it.texts.iter().find_map(|t| match t { Prim::Text { p, h, s, anchor, .. } if *s == it.tag && !it.tag.is_empty() => Some((*p, *h, *anchor)), _ => None });
        let (pt, h, anchor, hidden) = label.map_or(([it.x, it.bbox[3] + 3.0], 2.5, 1, true), |(p, h, a)| (p, h, a, false));
        w.text(pt, h, &it.tag, anchor, &layer, true, Some(("TAG", hidden || it.tag.is_empty())));
        w.g(0, "SEQEND");
        w.g(8, &layer);
        for t in &it.texts {
            if !matches!(t, Prim::Text { s, .. } if *s == it.tag && label.is_some()) {
                w.prim(t, &layer, true, false);
            }
        }
    }
    for n in &v.notes {
        for pr in &n.prims {
            w.prim(pr, "NOTES", true, false);
        }
    }
    for r in &v.legend_rows {
        for pr in view::legend_prims(r) {
            w.prim(&pr, "TEXT", true, false);
        }
    }
    w.g(0, "ENDSEC");
    w.g(0, "EOF");
    w.s
}

/// The header (units, extents) and the tables (line types, layers, a text style).
fn head(w: &mut W, layers: &[String], vw: f64, vh: f64) {
    // Header.
    w.g(0, "SECTION");
    w.g(2, "HEADER");
    w.g(9, "$ACADVER");
    w.g(1, "AC1009");
    w.g(9, "$INSUNITS");
    w.g(70, 4);
    w.g(9, "$EXTMIN");
    w.f(10, 0.0);
    w.f(20, 0.0);
    w.g(9, "$EXTMAX");
    w.f(10, vw);
    w.f(20, vh);
    w.g(0, "ENDSEC");
    // Tables: line types, layers, a text style.
    w.g(0, "SECTION");
    w.g(2, "TABLES");
    w.g(0, "TABLE");
    w.g(2, "LTYPE");
    w.g(70, 2);
    for (name, desc, pat) in [("CONTINUOUS", "Solid line", &[][..]), ("DASHED", "Dashed __ __ __", &[1.5, -1.0][..])] {
        w.g(0, "LTYPE");
        w.g(2, name);
        w.g(70, 0);
        w.g(3, desc);
        w.g(72, 65);
        w.g(73, pat.len());
        w.f(40, pat.iter().map(|x: &f64| x.abs()).sum());
        for x in pat {
            w.f(49, *x);
        }
    }
    w.g(0, "ENDTAB");
    w.g(0, "TABLE");
    w.g(2, "LAYER");
    w.g(70, layers.len());
    for l in layers {
        w.g(0, "LAYER");
        w.g(2, l);
        w.g(70, 0);
        w.g(62, if l == "LINE-ELECTRIC" || l.starts_with("SYM-INSTRUMENT") { 5 } else if l == "NOTES" { 1 } else { 7 });
        w.g(6, if l == "LINE-ELECTRIC" { "DASHED" } else { "CONTINUOUS" });
    }
    w.g(0, "ENDTAB");
    w.g(0, "TABLE");
    w.g(2, "STYLE");
    w.g(70, 1);
    w.g(0, "STYLE");
    w.g(2, "STANDARD");
    w.g(70, 0);
    w.f(40, 0.0);
    w.f(41, 1.0);
    w.f(50, 0.0);
    w.g(71, 0);
    w.f(42, 2.5);
    w.g(3, "txt");
    w.g(4, "");
    w.g(0, "ENDTAB");
    w.g(0, "ENDSEC");
}

/// Drawing primitives on a sheet of their own (a loop diagram) as a DXF R12 file: the frame and
/// texts on FRAME and TEXT, the drawing on DRAWING (dashed where the drawing is dashed).
pub fn write_prims(prims: &[Prim], vw: f64, vh: f64) -> String {
    let mut w = W { s: String::with_capacity(1 << 15), h: vh };
    let layers: Vec<String> = vec!["FRAME".into(), "TEXT".into(), "DRAWING".into()];
    head(&mut w, &layers, vw, vh);
    w.g(0, "SECTION");
    w.g(2, "BLOCKS");
    w.g(0, "ENDSEC");
    w.g(0, "SECTION");
    w.g(2, "ENTITIES");
    for pr in prims {
        let layer = match pr {
            Prim::Text { .. } => "TEXT",
            Prim::Poly { w: lw, .. } if *lw >= 0.35 => "FRAME",
            _ => "DRAWING",
        };
        w.prim(pr, layer, true, false);
    }
    w.g(0, "ENDSEC");
    w.g(0, "EOF");
    w.s
}

/// Reads a DXF back into (layers, block names, entity type counts in ENTITIES) — for the tests,
/// and a check that the file is well formed: codes and values in pairs, sections closed.
pub type ReadBack = (Vec<String>, Vec<String>, std::collections::BTreeMap<String, usize>);

pub fn read_back(text: &str) -> Result<ReadBack, String> {
    let lines: Vec<&str> = text.split("\r\n").collect();
    let mut pairs = vec![];
    let mut i = 0;
    while i + 1 < lines.len() {
        let code: i32 = lines[i].trim().parse().map_err(|_| format!("line {}: not a group code: {:?}", i + 1, lines[i]))?;
        pairs.push((code, lines[i + 1]));
        i += 2;
    }
    if pairs.last() != Some(&(0, "EOF")) {
        return Err("no EOF".into());
    }
    let (mut layers, mut blocks, mut ents) = (vec![], vec![], std::collections::BTreeMap::new());
    let mut section = "";
    let mut k = 0;
    while k < pairs.len() {
        let (c, v) = pairs[k];
        if c == 0 && v == "SECTION" {
            section = pairs.get(k + 1).map_or("", |x| x.1);
        } else if c == 0 && v == "ENDSEC" {
            section = "";
        } else if c == 0 && section == "TABLES" && v == "LAYER" {
            if let Some(n) = pairs[k + 1..].iter().find(|x| x.0 == 2) {
                layers.push(n.1.to_string());
            }
        } else if c == 0 && section == "BLOCKS" && v == "BLOCK" {
            if let Some(n) = pairs[k + 1..].iter().find(|x| x.0 == 2) {
                blocks.push(n.1.to_string());
            }
        } else if c == 0 && section == "ENTITIES" {
            *ents.entry(v.to_string()).or_insert(0) += 1;
        }
        k += 1;
    }
    Ok((layers, blocks, ents))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::tests::B;

    #[test]
    fn dxf_parses_back_to_the_same_drawing() {
        let mut b = B::new();
        let v1 = b.item("vvessel", 80.0, 100.0);
        let pm = b.item("pump_cent", 200.0, 140.0);
        let l = b.line((v1, 3), (pm, 0));
        b.spec(&[l], "3", "A1A");
        let i = b.item("inst_field", 140.0, 60.0);
        b.tag(i, "LT-101");
        b.tag(v1, "V-101 «€»");
        let p = &b.d.p;
        let s = &p.sheets[0];
        let text = write(p, s);
        let (layers, blocks, ents) = read_back(&text).unwrap();
        for l in ["FRAME", "LINE-MAJOR", "LINE-ELECTRIC", "SYM-VESSEL", "SYM-ROTATING", "SYM-INSTRUMENT"] {
            assert!(layers.iter().any(|x| x == l), "layer {l}");
        }
        assert_eq!(blocks.len(), 3, "one block per symbol used: {blocks:?}");
        assert!(blocks.contains(&"SYM_PUMP_CENT".to_string()));
        assert_eq!(ents.get("INSERT"), Some(&3));
        assert_eq!(ents.get("ATTRIB"), Some(&3));
        // The line: one polyline per drawn piece, plus the frame's.
        let net = Net::new(p);
        let v = view::sheet(p, s, &net, &[]);
        let frame_polys = v.frame.iter().filter(|x| matches!(x, Prim::Poly { .. })).count();
        let line_polys: usize = v.lines.iter().map(|l| l.segs.len()).sum();
        let mark_polys: usize = v.lines.iter().flat_map(|l| &l.marks).filter(|m| matches!(m, Prim::Poly { fill: false, .. })).count();
        assert_eq!(ents.get("POLYLINE").copied().unwrap_or(0), frame_polys + line_polys + mark_polys);
        assert!(ents.get("SOLID").copied().unwrap_or(0) >= 1, "flow arrows");
        assert!(text.contains("\\U+20AC"), "non-ASCII written as escapes");
        assert!(text.contains("PID-001"));
    }
}
