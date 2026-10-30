//! Reading DXF drawings (ASCII DXF, any release from R12 on): the blocks and the entities of the
//! model space, flattened into Datum Schema's drawing primitives in millimetres with y down. Used
//! by the legend import (a client's symbols are the blocks of their legend sheet) and by the
//! background import (an old drawing to trace over).
//!
//! Read: LINE, LWPOLYLINE (with arcs by bulge), POLYLINE with VERTEX, CIRCLE, ARC, ELLIPSE, SPLINE
//! (through its fit or control points), SOLID and TRACE, HATCH with solid fill (polyline and line
//! boundaries), TEXT, MTEXT (formatting removed), INSERT (nested blocks, scale, rotation, the
//! attributes it carries). Everything else (dimensions, images, 3D entities) is skipped and
//! counted. DWG files are not read: save them as DXF from any CAD program.

use crate::symbols::Prim;
use std::collections::HashMap;

/// One entity, in drawing units with y up, as the file has it.
#[derive(Clone, Debug)]
pub enum Ent {
    Poly { pts: Vec<[f64; 2]>, closed: bool, fill: bool },
    Circle { c: [f64; 2], r: f64 },
    Text { p: [f64; 2], h: f64, s: String, rot: f64, anchor: u8 },
    /// A block placed: its attributes are (tag, value, where, height, visible), placed in the
    /// insert's surroundings (not the block's own coordinates).
    Insert { name: String, p: [f64; 2], sx: f64, sy: f64, rot: f64, attribs: Vec<Attrib> },
}

#[derive(Clone, Debug)]
pub struct Attrib {
    pub tag: String,
    pub value: String,
    pub p: [f64; 2],
    pub h: f64,
    pub visible: bool,
}

#[derive(Debug, Default)]
pub struct Dxf {
    /// Millimetres per drawing unit ($INSUNITS; unitless drawings are taken as millimetres).
    pub mm: f64,
    pub blocks: HashMap<String, Vec<Ent>>,
    /// The model space.
    pub ents: Vec<Ent>,
    /// Entity types skipped, with how many.
    pub skipped: std::collections::BTreeMap<String, usize>,
}

const MAX_ENTS: usize = 500_000;

/// The pairs of a DXF file: (group code, value).
fn pairs(text: &str) -> Result<Vec<(i32, &str)>, String> {
    if text.starts_with("AutoCAD Binary DXF") {
        return Err("This is a binary DXF; save it as an ASCII DXF and import that.".into());
    }
    let mut v = vec![];
    let mut lines = text.lines();
    while let Some(c) = lines.next() {
        let c = c.trim();
        if c.is_empty() {
            continue;
        }
        let code: i32 = c.parse().map_err(|_| "This is not a DXF file (it does not start with DXF group codes).".to_string())?;
        let val = lines.next().unwrap_or("").trim_end_matches('\r');
        v.push((code, val));
    }
    if !v.iter().any(|p| p == &(0, "SECTION")) {
        return Err("This is not a DXF file (it has no sections).".into());
    }
    Ok(v)
}

fn f(v: &str) -> f64 {
    v.trim().parse::<f64>().ok().filter(|x| x.is_finite()).unwrap_or(0.0)
}

/// DXF text: \U+XXXX escapes decoded, %%d °, %%p ±, %%c ⌀, %%% %; MTEXT codes removed.
fn text(s: &str, mtext: bool) -> String {
    let mut s = s.replace("%%d", "°").replace("%%D", "°").replace("%%p", "±").replace("%%P", "±").replace("%%c", "⌀").replace("%%C", "⌀").replace("%%%", "%");
    if mtext {
        // Paragraphs, then formatting groups {\f…;text} and codes \A1; \H2.5; \C1; \L \O …
        s = s.replace("\\P", " ").replace("\\~", " ");
        let mut o = String::new();
        let mut cs = s.chars().peekable();
        while let Some(c) = cs.next() {
            match c {
                '{' | '}' => {}
                '\\' => match cs.next() {
                    Some('\\') => o.push('\\'),
                    Some('{') => o.push('{'),
                    Some('}') => o.push('}'),
                    Some('U') => o.push_str("\\U"),
                    Some('S') => {
                        // Stacked text a^b: written a/b.
                        for d in cs.by_ref() {
                            if d == ';' {
                                break;
                            }
                            o.push(if d == '^' || d == '#' { '/' } else { d });
                        }
                    }
                    Some(c2) if "LlOoKk".contains(c2) => {}
                    Some(_) => {
                        for d in cs.by_ref() {
                            if d == ';' {
                                break;
                            }
                        }
                    }
                    None => {}
                },
                c => o.push(c),
            }
        }
        s = o;
    }
    // \U+XXXX
    let mut o = String::new();
    let mut i = 0;
    let b: Vec<char> = s.chars().collect();
    while i < b.len() {
        if b[i] == '\\' && i + 6 < b.len() + 1 && b.get(i + 1) == Some(&'U') && b.get(i + 2) == Some(&'+') {
            let hex: String = b[i + 3..(i + 7).min(b.len())].iter().collect();
            if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                o.push(ch);
                i += 7;
                continue;
            }
        }
        o.push(b[i]);
        i += 1;
    }
    crate::clip(o.split_whitespace().collect::<Vec<_>>().join(" ").as_str(), 300)
}

/// Points of an arc from a0 to a1 degrees (counter-clockwise, y up).
fn arc_pts(c: [f64; 2], r: f64, a0: f64, a1: f64) -> Vec<[f64; 2]> {
    let mut sweep = (a1 - a0).rem_euclid(360.0);
    if sweep < 1e-9 {
        sweep = 360.0;
    }
    let n = ((sweep / 10.0).ceil() as usize).clamp(2, 64);
    (0..=n).map(|i| { let a = (a0 + sweep * i as f64 / n as f64).to_radians(); [c[0] + r * a.cos(), c[1] + r * a.sin()] }).collect()
}

/// A polyline vertex list with bulges (arc segments) to plain points.
fn bulged(v: &[([f64; 2], f64)], closed: bool) -> Vec<[f64; 2]> {
    let mut out = vec![];
    let n = v.len();
    for i in 0..n {
        let (p, b) = v[i];
        out.push(p);
        if i + 1 == n && !closed {
            break;
        }
        let q = v[(i + 1) % n].0;
        if b.abs() > 1e-9 {
            // The arc from p to q with bulge b = tan(θ/4).
            let th = 4.0 * b.atan();
            let d = (q[0] - p[0]).hypot(q[1] - p[1]);
            if d < 1e-12 {
                continue;
            }
            let r = d / (2.0 * (th / 2.0).sin());
            let m = [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
            let h = r * (th / 2.0).cos();
            let (ux, uy) = ((q[0] - p[0]) / d, (q[1] - p[1]) / d);
            let c = [m[0] - uy * h, m[1] + ux * h];
            let (a0, a1) = ((p[1] - c[1]).atan2(p[0] - c[0]).to_degrees(), (q[1] - c[1]).atan2(q[0] - c[0]).to_degrees());
            let pts = if th > 0.0 { arc_pts(c, r.abs(), a0, a1) } else { let mut v = arc_pts(c, r.abs(), a1, a0); v.reverse(); v };
            out.extend(pts.into_iter().skip(1).take_while(|_| true));
            out.pop();
        }
    }
    out
}

/// Reads an entity list from `pairs[i..]` up to ENDSEC or ENDBLK; returns it and where it stopped.
fn entities(ps: &[(i32, &str)], mut i: usize, end: &str, skipped: &mut std::collections::BTreeMap<String, usize>, count: &mut usize) -> Result<(Vec<Ent>, usize), String> {
    let mut out = vec![];
    while i < ps.len() {
        let (c, v) = ps[i];
        if c != 0 {
            i += 1;
            continue;
        }
        if v == end || v == "ENDSEC" || v == "EOF" {
            return Ok((out, i));
        }
        *count += 1;
        if *count > MAX_ENTS {
            return Err("That drawing is too large to read.".into());
        }
        // The entity's own pairs.
        let ty = v;
        let mut j = i + 1;
        while j < ps.len() && ps[j].0 != 0 {
            j += 1;
        }
        let g = &ps[i + 1..j];
        let get = |code: i32| g.iter().find(|p| p.0 == code).map(|p| p.1);
        let num = |code: i32, d: f64| get(code).map_or(d, f);
        let pt = |cx: i32| [num(cx, 0.0), num(cx + 10, 0.0)];
        match ty {
            "LINE" => out.push(Ent::Poly { pts: vec![pt(10), pt(11)], closed: false, fill: false }),
            "LWPOLYLINE" => {
                let mut v: Vec<([f64; 2], f64)> = vec![];
                for (k, &(code, val)) in g.iter().enumerate() {
                    if code == 10 {
                        let y = g[k + 1..].iter().find(|p| p.0 == 20).map_or(0.0, |p| f(p.1));
                        v.push(([f(val), y], 0.0));
                    } else if code == 42 {
                        if let Some(last) = v.last_mut() {
                            last.1 = f(val);
                        }
                    }
                }
                let closed = (get(70).map_or(0, |x| x.trim().parse::<i32>().unwrap_or(0)) & 1) == 1;
                if v.len() >= 2 {
                    out.push(Ent::Poly { pts: bulged(&v, closed), closed, fill: false });
                }
            }
            "POLYLINE" => {
                let closed = (get(70).map_or(0, |x| x.trim().parse::<i32>().unwrap_or(0)) & 1) == 1;
                let mut v: Vec<([f64; 2], f64)> = vec![];
                let mut k = j;
                while k < ps.len() && ps[k] != (0, "SEQEND") {
                    if ps[k] == (0, "VERTEX") {
                        let mut m = k + 1;
                        let (mut x, mut y, mut b) = (0.0, 0.0, 0.0);
                        while m < ps.len() && ps[m].0 != 0 {
                            match ps[m].0 {
                                10 => x = f(ps[m].1),
                                20 => y = f(ps[m].1),
                                42 => b = f(ps[m].1),
                                _ => {}
                            }
                            m += 1;
                        }
                        v.push(([x, y], b));
                        k = m;
                    } else {
                        k += 1;
                    }
                    if ps.get(k).is_some_and(|p| p.0 == 0 && p.1 != "VERTEX" && p.1 != "SEQEND") {
                        break;
                    }
                }
                if v.len() >= 2 {
                    out.push(Ent::Poly { pts: bulged(&v, closed), closed, fill: false });
                }
                i = k;
                continue;
            }
            "CIRCLE" => out.push(Ent::Circle { c: pt(10), r: num(40, 0.0).abs() }),
            "ARC" => out.push(Ent::Poly { pts: arc_pts(pt(10), num(40, 0.0).abs(), num(50, 0.0), num(51, 360.0)), closed: false, fill: false }),
            "ELLIPSE" => {
                let (c, m, ratio) = (pt(10), pt(11), num(40, 1.0));
                let (t0, t1) = (num(41, 0.0), num(42, std::f64::consts::TAU));
                let mut sweep = (t1 - t0).rem_euclid(std::f64::consts::TAU);
                if sweep < 1e-9 {
                    sweep = std::f64::consts::TAU;
                }
                let minor = [-m[1] * ratio, m[0] * ratio];
                let n = 48;
                let pts = (0..=n).map(|k| { let t = t0 + sweep * k as f64 / n as f64; [c[0] + m[0] * t.cos() + minor[0] * t.sin(), c[1] + m[1] * t.cos() + minor[1] * t.sin()] }).collect();
                out.push(Ent::Poly { pts, closed: false, fill: false });
            }
            "SPLINE" => {
                let pick = |cx: i32| -> Vec<[f64; 2]> { g.iter().enumerate().filter(|(_, p)| p.0 == cx).map(|(k, p)| [f(p.1), g[k + 1..].iter().find(|q| q.0 == cx + 10).map_or(0.0, |q| f(q.1))]).collect() };
                let fit = pick(11);
                let pts = if fit.len() >= 2 { fit } else { pick(10) };
                if pts.len() >= 2 {
                    out.push(Ent::Poly { pts, closed: false, fill: false });
                }
            }
            "SOLID" | "TRACE" => {
                let (a, b, c, d) = (pt(10), pt(11), pt(12), pt(13));
                out.push(Ent::Poly { pts: if c == d { vec![a, b, c] } else { vec![a, b, d, c] }, closed: true, fill: true });
            }
            "HATCH" => {
                // Solid fills with polyline or line boundaries.
                let solid = get(70).is_some_and(|x| x.trim() == "1");
                if solid {
                    let mut pts: Vec<[f64; 2]> = vec![];
                    for (k, &(code, val)) in g.iter().enumerate() {
                        if code == 10 {
                            let y = g.get(k + 1).filter(|p| p.0 == 20).map_or(0.0, |p| f(p.1));
                            pts.push([f(val), y]);
                        }
                    }
                    // The first 10/20 of a hatch is its elevation point (0, 0): skip it.
                    if pts.len() > 3 {
                        pts.remove(0);
                        pts.dedup();
                        out.push(Ent::Poly { pts, closed: true, fill: true });
                    }
                } else {
                    *skipped.entry("HATCH (pattern)".into()).or_insert(0) += 1;
                }
            }
            "TEXT" | "ATTDEF" if ty == "TEXT" => {
                let (h72, v73) = (get(72).map_or(0, |x| x.trim().parse::<i32>().unwrap_or(0)), get(73).map_or(0, |x| x.trim().parse::<i32>().unwrap_or(0)));
                let p = if h72 != 0 || v73 != 0 { pt(11) } else { pt(10) };
                let anchor = match h72 { 1 | 4 => 1, 2 => 2, _ => 0 };
                let s = text(get(1).unwrap_or(""), false);
                if !s.is_empty() {
                    out.push(Ent::Text { p, h: num(40, 2.5), s, rot: num(50, 0.0), anchor });
                }
            }
            "MTEXT" => {
                let raw: String = g.iter().filter(|p| p.0 == 3).map(|p| p.1).chain(get(1)).collect();
                let s = text(&raw, true);
                let att = get(71).map_or(1, |x| x.trim().parse::<i32>().unwrap_or(1));
                let anchor = match (att - 1) % 3 { 1 => 1, 2 => 2, _ => 0 };
                if !s.is_empty() {
                    out.push(Ent::Text { p: pt(10), h: num(40, 2.5), s, rot: num(50, 0.0), anchor });
                }
            }
            "INSERT" => {
                let name = get(2).unwrap_or("").to_string();
                let mut attribs = vec![];
                let mut k = j;
                if get(66).is_some_and(|x| x.trim() == "1") {
                    while k < ps.len() && ps[k] != (0, "SEQEND") {
                        if ps[k] == (0, "ATTRIB") {
                            let mut m = k + 1;
                            let mut a = Attrib { tag: String::new(), value: String::new(), p: [0.0, 0.0], h: 2.5, visible: true };
                            let mut al: Option<[f64; 2]> = None;
                            while m < ps.len() && ps[m].0 != 0 {
                                match ps[m].0 {
                                    2 => a.tag = ps[m].1.to_string(),
                                    1 => a.value = text(ps[m].1, false),
                                    10 => a.p[0] = f(ps[m].1),
                                    20 => a.p[1] = f(ps[m].1),
                                    11 => al = Some([f(ps[m].1), al.map_or(0.0, |q| q[1])]),
                                    21 => al = Some([al.map_or(0.0, |q| q[0]), f(ps[m].1)]),
                                    40 => a.h = f(ps[m].1),
                                    70 => a.visible = ps[m].1.trim().parse::<i32>().unwrap_or(0) & 1 == 0,
                                    _ => {}
                                }
                                m += 1;
                            }
                            if let Some(q) = al.filter(|q| *q != [0.0, 0.0]) {
                                a.p = q;
                            }
                            attribs.push(a);
                            k = m;
                        } else {
                            k += 1;
                        }
                    }
                }
                out.push(Ent::Insert { name, p: pt(10), sx: num(41, 1.0), sy: num(42, 1.0), rot: num(50, 0.0), attribs });
                if k > j {
                    i = k;
                    continue;
                }
            }
            "SEQEND" | "VERTEX" | "ATTRIB" | "ATTDEF" | "ENDBLK" => {}
            other => *skipped.entry(other.to_string()).or_insert(0) += 1,
        }
        i = j;
    }
    Ok((out, i))
}

/// Reads a DXF file.
pub fn read(text_in: &str) -> Result<Dxf, String> {
    if text_in.len() > crate::doc::MAX_FILE as usize {
        return Err("That drawing is too large to read.".into());
    }
    let ps = pairs(text_in)?;
    let mut d = Dxf { mm: 1.0, ..Dxf::default() };
    let mut count = 0usize;
    let mut i = 0;
    while i < ps.len() {
        if ps[i] == (0, "SECTION") {
            let name = ps.get(i + 1).map_or("", |p| p.1);
            i += 2;
            match name {
                "HEADER" => {
                    while i < ps.len() && ps[i] != (0, "ENDSEC") {
                        if ps[i] == (9, "$INSUNITS") {
                            if let Some(&(70, v)) = ps.get(i + 1) {
                                d.mm = match v.trim() { "1" => 25.4, "2" => 304.8, "5" => 10.0, "6" => 1000.0, "14" => 100.0, "8" => 0.0254, _ => 1.0 };
                            }
                        }
                        i += 1;
                    }
                }
                "BLOCKS" => {
                    while i < ps.len() && ps[i] != (0, "ENDSEC") {
                        if ps[i] == (0, "BLOCK") {
                            let mut j = i + 1;
                            let mut name = String::new();
                            let mut base = [0.0, 0.0];
                            while j < ps.len() && ps[j].0 != 0 {
                                match ps[j].0 {
                                    2 => name = ps[j].1.to_string(),
                                    10 => base[0] = f(ps[j].1),
                                    20 => base[1] = f(ps[j].1),
                                    _ => {}
                                }
                                j += 1;
                            }
                            let (mut ents, stop) = entities(&ps, j, "ENDBLK", &mut d.skipped, &mut count)?;
                            // Block geometry relative to its base point.
                            if base != [0.0, 0.0] {
                                for e in &mut ents {
                                    shift(e, [-base[0], -base[1]]);
                                }
                            }
                            d.blocks.insert(name, ents);
                            i = stop + 1;
                            continue;
                        }
                        i += 1;
                    }
                }
                "ENTITIES" => {
                    let (ents, stop) = entities(&ps, i, "ENDSEC", &mut d.skipped, &mut count)?;
                    d.ents = ents;
                    i = stop;
                }
                _ => {
                    while i < ps.len() && ps[i] != (0, "ENDSEC") {
                        i += 1;
                    }
                }
            }
        }
        i += 1;
    }
    Ok(d)
}

fn shift(e: &mut Ent, by: [f64; 2]) {
    let mv = |p: &mut [f64; 2]| {
        p[0] += by[0];
        p[1] += by[1];
    };
    match e {
        Ent::Poly { pts, .. } => pts.iter_mut().for_each(mv),
        Ent::Circle { c, .. } => mv(c),
        Ent::Text { p, .. } => mv(p),
        Ent::Insert { p, attribs, .. } => {
            mv(p);
            attribs.iter_mut().for_each(|a| mv(&mut a.p));
        }
    }
}

/// A 2D transform: x' = a·x + b·y + e, y' = c·x + d·y + f.
#[derive(Clone, Copy, Debug)]
pub struct Tf {
    pub m: [f64; 6],
}

impl Tf {
    pub const ID: Tf = Tf { m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] };
    fn ap(&self, p: [f64; 2]) -> [f64; 2] {
        let [a, b, c, d, e, f] = self.m;
        [a * p[0] + b * p[1] + e, c * p[0] + d * p[1] + f]
    }
    fn then(&self, o: &Tf) -> Tf {
        // o ∘ self
        let [a, b, c, d, e, f] = self.m;
        let [a2, b2, c2, d2, e2, f2] = o.m;
        Tf { m: [a2 * a + b2 * c, a2 * b + b2 * d, c2 * a + d2 * c, c2 * b + d2 * d, a2 * e + b2 * f + e2, c2 * e + d2 * f + f2] }
    }
    fn scale(&self) -> f64 {
        let [a, b, c, d, ..] = self.m;
        ((a * d - b * c).abs()).sqrt()
    }
    fn uniform(&self) -> bool {
        let [a, b, c, d, ..] = self.m;
        ((a * a + c * c) - (b * b + d * d)).abs() < 1e-9 * (a * a + c * c).max(1e-12) && (a * b + c * d).abs() < 1e-9 * (a * a + c * c).max(1e-12)
    }
    /// An INSERT's placement: scale, then rotate, then move.
    pub fn insert(p: [f64; 2], sx: f64, sy: f64, rot: f64) -> Tf {
        let (s, c) = rot.to_radians().sin_cos();
        Tf { m: [c * sx, -s * sy, s * sx, c * sy, p[0], p[1]] }
    }
}

/// Entities to primitives in millimetres with y down (y' = −y; the caller moves them on the sheet),
/// following inserts into their blocks (eight levels deep at most).
pub fn flatten(d: &Dxf, ents: &[Ent], tf: Tf, out: &mut Vec<Prim>, depth: usize) {
    if depth > 8 || out.len() > MAX_ENTS {
        return;
    }
    let to = |p: [f64; 2]| { let q = tf.ap(p); [q[0] * d.mm, -q[1] * d.mm] };
    let w = crate::symbols::STROKE;
    for e in ents {
        match e {
            Ent::Poly { pts, closed, fill } => out.push(Prim::Poly { pts: pts.iter().map(|p| to(*p)).collect(), closed: *closed, fill: *fill, dash: false, w }),
            Ent::Circle { c, r } => {
                if tf.uniform() {
                    out.push(Prim::Circle { c: to(*c), r: r * tf.scale() * d.mm, fill: false, w });
                } else {
                    out.push(Prim::Poly { pts: arc_pts(*c, *r, 0.0, 360.0).into_iter().map(to).collect(), closed: true, fill: false, dash: false, w });
                }
            }
            Ent::Text { p, h, s, anchor, .. } => out.push(Prim::Text { p: to(*p), h: (h * tf.scale() * d.mm).clamp(0.5, 50.0), s: s.clone(), anchor: *anchor, bold: false }),
            Ent::Insert { name, p, sx, sy, rot, attribs } => {
                if let Some(b) = d.blocks.get(name) {
                    let t = Tf::insert(*p, *sx, *sy, *rot).then(&tf);
                    flatten(d, b, t, out, depth + 1);
                }
                for a in attribs.iter().filter(|a| a.visible && !a.value.is_empty()) {
                    out.push(Prim::Text { p: to(a.p), h: (a.h * tf.scale() * d.mm).clamp(0.5, 50.0), s: a.value.clone(), anchor: 1, bold: false });
                }
            }
        }
    }
}

/// The bounding box of primitives: (x0, y0, x1, y1).
pub fn bbox(ps: &[Prim]) -> Option<[f64; 4]> {
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    let mut add = |p: [f64; 2]| {
        b[0] = b[0].min(p[0]);
        b[1] = b[1].min(p[1]);
        b[2] = b[2].max(p[0]);
        b[3] = b[3].max(p[1]);
    };
    for p in ps {
        match p {
            Prim::Poly { pts, .. } => pts.iter().for_each(|q| add(*q)),
            Prim::Circle { c, r, .. } | Prim::Arc { c, r, .. } => {
                add([c[0] - r, c[1] - r]);
                add([c[0] + r, c[1] + r]);
            }
            Prim::Text { p, h, s, .. } => {
                add(*p);
                add([p[0] + h * 0.6 * s.chars().count() as f64, p[1] + h / 2.0]);
            }
        }
    }
    (b[0].is_finite() && b[2] >= b[0]).then_some(b)
}

/// Moves and scales primitives: p' = (p + by) · k.
pub fn transform(ps: &mut [Prim], by: [f64; 2], k: f64) {
    let t = |p: &mut [f64; 2]| {
        p[0] = (p[0] + by[0]) * k;
        p[1] = (p[1] + by[1]) * k;
    };
    for p in ps {
        match p {
            Prim::Poly { pts, .. } => pts.iter_mut().for_each(t),
            Prim::Circle { c, r, .. } | Prim::Arc { c, r, .. } => {
                t(c);
                *r *= k;
            }
            Prim::Text { p, h, .. } => {
                t(p);
                *h *= k;
            }
        }
    }
}

/// An old drawing read for a sheet's background: its shapes moved to start at (0, 0), then
/// scaled to fit the sheet's drawing area (never enlarged) and centred in it.
pub fn underlay(text: &str, name: &str, s: &crate::model::Sheet) -> Result<(crate::model::Underlay, Vec<String>), String> {
    let d = read(text)?;
    let mut ps = vec![];
    flatten(&d, &d.ents, Tf::ID, &mut ps, 0);
    let b = bbox(&ps).ok_or("That drawing has nothing Datum Schema can show (lines, circles, arcs or text).")?;
    transform(&mut ps, [-b[0], -b[1]], 1.0);
    let draw = crate::symlib::to_draw(&ps);
    if draw.len() > crate::model::MAX_UNDERLAY {
        return Err("That drawing is too large to show behind a sheet.".into());
    }
    let (w, h) = ((b[2] - b[0]).max(1e-3), (b[3] - b[1]).max(1e-3));
    let (x0, y0, x1, y1) = crate::frame::inner(s);
    let k = ((x1 - x0) / w).min((y1 - y0) / h).min(1.0);
    let mut warnings = vec![];
    if !d.skipped.is_empty() {
        warnings.push(format!("Not shown: {}.", d.skipped.iter().map(|(k, v)| format!("{v} {k}")).collect::<Vec<_>>().join(", ")));
    }
    if k < 1.0 {
        warnings.push(format!("The drawing is larger than the sheet; it is shown at {}%.", crate::fmt_num((k * 1000.0).round() / 10.0)));
    }
    let rev = crate::model::Underlay::fingerprint(&draw);
    let u = crate::model::Underlay { name: crate::clip(name, 120), draw: std::sync::Arc::new(draw), w, h, x: (x0 + x1 - w * k) / 2.0, y: (y0 + y1 - h * k) / 2.0, k, show: true, print: false, rev };
    Ok((u, warnings))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small DXF by hand: inches, a block with a bulged polyline (a half circle), MTEXT, a nested
    /// insert, a solid hatch, and an entity we skip.
    pub(crate) const SAMPLE: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n1\n0\nENDSEC\n0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nDOT\n10\n0\n20\n0\n0\nCIRCLE\n10\n0\n20\n0\n40\n0.05\n0\nENDBLK\n0\nBLOCK\n2\nVLV-GATE\n10\n0\n20\n0\n0\nLWPOLYLINE\n90\n4\n70\n1\n10\n-0.2\n20\n-0.1\n10\n0.2\n20\n0.1\n10\n0.2\n20\n-0.1\n10\n-0.2\n20\n0.1\n0\nLWPOLYLINE\n90\n2\n70\n0\n10\n-0.1\n20\n0.2\n42\n1\n10\n0.1\n20\n0.2\n0\nINSERT\n2\nDOT\n10\n0\n20\n0\n0\nHATCH\n70\n1\n10\n0\n20\n0\n10\n0\n20\n0\n10\n0.1\n20\n0\n10\n0\n20\n0.1\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nINSERT\n2\nVLV-GATE\n10\n1\n20\n5\n0\nMTEXT\n10\n1.5\n20\n5.05\n40\n0.1\n1\n{\\fArial|b1;GATE VALVE}\\PFLANGED\n0\nDIMENSION\n0\nENDSEC\n0\nEOF\n";

    #[test]
    fn a_hand_written_dxf_reads() {
        let d = read(SAMPLE).unwrap();
        assert_eq!(d.mm, 25.4);
        assert_eq!(d.blocks.len(), 2);
        assert_eq!(d.skipped.get("DIMENSION"), Some(&1));
        let mut out = vec![];
        flatten(&d, &d.ents, Tf::ID, &mut out, 0);
        // The bowtie, the half circle, the nested dot, the filled hatch, and the text.
        assert!(out.iter().any(|p| matches!(p, Prim::Circle { r, .. } if (r - 0.05 * 25.4).abs() < 1e-9)));
        assert!(out.iter().any(|p| matches!(p, Prim::Poly { fill: true, .. })));
        let half = out.iter().find_map(|p| match p { Prim::Poly { pts, closed: false, .. } if pts.len() > 3 => Some(pts.clone()), _ => None }).unwrap();
        // A half circle of radius 0.1 in, round (1, 5.2) in: every point 2.54 mm from the centre.
        for q in &half {
            assert!(((q[0] - 25.4).hypot(q[1] + 5.2 * 25.4) - 2.54).abs() < 1e-6, "{q:?}");
        }
        assert!(out.iter().any(|p| matches!(p, Prim::Text { s, .. } if s == "GATE VALVE FLANGED")));
    }

    #[test]
    fn our_own_dxf_reads_back() {
        let p = crate::examples::get("pump").unwrap();
        let d = read(&crate::dxf::write(&p, &p.sheets[0])).unwrap();
        assert!(d.blocks.contains_key("SYM_PUMP_CENT"));
        let mut out = vec![];
        flatten(&d, &d.ents, Tf::ID, &mut out, 0);
        assert!(out.len() > 200);
        assert!(out.iter().any(|p| matches!(p, Prim::Text { s, .. } if s == "P-101A")));
    }

    #[test]
    fn an_old_drawing_goes_behind_a_sheet() {
        // An A1 drawing behind an A3 sheet: scaled to fit the drawing area, centred.
        let mut p = crate::examples::get("pump").unwrap();
        p.sheets[0].size = "A1".into();
        let text = crate::dxf::write(&p, &p.sheets[0]);
        let mut d = crate::doc::Doc::new(crate::examples::get("compressor").unwrap());
        let mut sh = d.p.sheets[0].clone();
        let (u, w) = underlay(&text, "old.dxf", &sh).unwrap();
        assert!(u.k < 0.6 && u.k > 0.4, "{}", u.k);
        assert!(w.iter().any(|x| x.contains("larger than the sheet")));
        let (x0, y0, x1, y1) = crate::frame::inner(&sh);
        let b = bbox(&u.prims()).unwrap();
        assert!(b[0] >= x0 - 0.01 && b[1] >= y0 - 0.01 && b[2] <= x1 + 0.01 && b[3] <= y1 + 0.01, "{b:?}");
        assert!(u.prims().iter().any(|p| matches!(p, Prim::Text { s, .. } if s == "P-101A")));
        // Set on the sheet; the window's copy (without the drawing) keeps it; undo shares it.
        let draw = u.draw.clone();
        sh.underlay = Some(u);
        d.apply(crate::doc::Op::SetSheet { sheet: sh.clone() }).unwrap();
        let mut from_window: crate::model::Sheet = serde_json::from_str(&serde_json::to_string(&d.p.sheets[0]).unwrap()).unwrap();
        assert!(from_window.underlay.as_ref().unwrap().draw.is_empty());
        from_window.underlay.as_mut().unwrap().print = true;
        d.apply(crate::doc::Op::SetSheet { sheet: from_window.clone() }).unwrap();
        assert_eq!(d.p.sheets[0].underlay.as_ref().unwrap().draw, draw);
        // A forged fingerprint takes nothing.
        from_window.underlay.as_mut().unwrap().rev = "x".into();
        d.apply(crate::doc::Op::SetSheet { sheet: from_window }).unwrap();
        assert!(d.p.sheets[0].underlay.is_none());
        d.undo();
        assert_eq!(d.p.sheets[0].underlay.as_ref().unwrap().draw, draw);
        // Printed: on its own layer in the sheet's DXF.
        assert!(crate::dxf::write(&d.p, &d.p.sheets[0]).contains("  8\r\nBACKGROUND\r\n"));
        // Saved and opened again.
        let path = std::env::temp_dir().join(format!("dschema-under-{}.dschema", std::process::id()));
        crate::file::write(&path, &d.p, &[]).unwrap();
        let (q, _, warn) = crate::file::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(warn.is_empty(), "{warn:?}");
        assert_eq!(q.sheets[0].underlay, d.p.sheets[0].underlay);
        // A small drawing is not enlarged.
        let (u, _) = underlay(SAMPLE, "s.dxf", &sh).unwrap();
        assert_eq!(u.k, 1.0);
        assert!(underlay("0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n", "e", &sh).is_err());
    }

    #[test]
    fn anything_else_is_refused_politely() {
        assert!(read("hello").is_err());
        assert!(read("AutoCAD Binary DXF\r\n\x1a\0").unwrap_err().contains("binary"));
        let mut r = 5u64;
        for n in [10usize, 2000, 50_000] {
            let s: String = (0..n).map(|_| { r ^= r << 13; r ^= r >> 7; r ^= r << 17; ["0\n", "SECTION\n", "10\n", "1.5\n", "LWPOLYLINE\n", "42\n", "INSERT\n", "2\n", "X\n", "ENDSEC\n", "BLOCK\n", "SEQEND\n", "66\n", "1\n"][(r % 14) as usize] }).collect();
            let _ = read(&s).map(|d| { let mut o = vec![]; flatten(&d, &d.ents, Tf::ID, &mut o, 0); });
        }
        // A block that inserts itself stops at eight levels.
        let looped = "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nA\n0\nINSERT\n2\nA\n10\n1\n20\n0\n0\nLINE\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nINSERT\n2\nA\n0\nENDSEC\n0\nEOF\n";
        let d = read(looped).unwrap();
        let mut o = vec![];
        flatten(&d, &d.ents, Tf::ID, &mut o, 0);
        assert!(o.len() <= 10);
    }
}
