//! Standard arrangements: what a company puts around each kind of equipment (a pump's isolation
//! valves, strainer, check valve and gauge; a vessel's relief valve, vent, drain and level loop),
//! kept as data per class and placed around a symbol in one step. Parts are placed from the
//! equipment's nozzles outward: in-line valves and fittings in order along the nozzle's line,
//! branches (instruments, relief valves, vents, drains) tapped onto it, a controller beside its
//! transmitter, and a bypass round a control valve. Sizes and pipe classes then follow the lines
//! (`propagate`), so the parts match the line they sit in. The result is a draft to tidy.

use crate::graph::port_pos;
use crate::model::{End, Item, Kind, Line, Project, Val};
use crate::symbols::{self, dir_vec, Cat, Place};
use serde::{Deserialize, Serialize};

/// One part of an arrangement.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Part {
    /// The nozzle it goes on, by name; alternatives separated by "|" (the first the symbol has).
    pub nozzle: String,
    /// "inline" (in the nozzle's line, in order from the nozzle outward), "branch" (tapped onto
    /// it), "controller" (beside the branch instrument before it, by a signal line) or "bypass"
    /// (round the equipment, from its inlet line to its outlet line).
    pub kind: String,
    /// The symbol.
    pub sym: String,
    /// The tag letters ("PI", "PSV"); the number is the equipment's where it is free.
    pub tag: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Arrangement {
    pub name: String,
    /// The class it is for (and its subclasses).
    pub class: String,
    pub parts: Vec<Part>,
}

pub const KINDS: [&str; 4] = ["inline", "branch", "controller", "bypass"];

fn part(nozzle: &str, kind: &str, sym: &str, tag: &str) -> Part {
    Part { nozzle: nozzle.into(), kind: kind.into(), sym: sym.into(), tag: tag.into() }
}

/// The starter set: a centrifugal pump, a vessel, a shell-and-tube exchanger, a control valve
/// station and a compressor.
pub fn starter() -> Vec<Arrangement> {
    let top = "top|roof|top right|top left|gas out";
    let bottom = "bottom|boot|bottom left|bottom right|outlet";
    let side = "right upper|right|right end|right middle|left upper|bottom right";
    vec![
        Arrangement {
            name: "Centrifugal pump".into(),
            class: "Pump".into(),
            parts: vec![part("suction", "inline", "strainer_y", ""), part("suction", "inline", "gate", ""), part("suction", "branch", "drain", ""), part("discharge", "inline", "check", ""), part("discharge", "inline", "gate", ""), part("discharge", "branch", "inst_field", "PI")],
        },
        Arrangement {
            name: "Vessel".into(),
            class: "Vessel".into(),
            parts: vec![part(top, "branch", "psv", "PSV"), part(top, "branch", "vent", ""), part(top, "branch", "inst_field", "PI"), part(side, "branch", "inst_field", "LT"), part(side, "controller", "inst_dcs", "LIC"), part(bottom, "branch", "drain", "")],
        },
        Arrangement {
            name: "Shell-and-tube exchanger".into(),
            class: "HeatExchanger".into(),
            parts: vec![part("tube in|cold in|inlet", "inline", "gate", ""), part("tube out|cold out|outlet", "inline", "gate", ""), part("tube out|cold out|outlet", "branch", "inst_field", "TT"), part("shell in|hot in", "inline", "gate", ""), part("shell out|hot out", "inline", "gate", ""), part("shell out|hot out", "branch", "drain", "")],
        },
        Arrangement {
            name: "Control valve station".into(),
            class: "ControlValve".into(),
            parts: vec![part("in", "inline", "gate", ""), part("in", "branch", "drain", ""), part("out", "inline", "gate", ""), part("", "bypass", "globe", "")],
        },
        Arrangement {
            name: "Compressor".into(),
            class: "Compressor".into(),
            parts: vec![part("suction", "branch", "inst_field", "PI"), part("discharge", "inline", "check", ""), part("discharge", "branch", "psv", "PSV"), part("discharge", "branch", "inst_field", "PT"), part("discharge", "branch", "inst_field", "TT")],
        },
    ]
}

pub fn normalize(v: &mut Vec<Arrangement>) {
    v.truncate(100);
    let mut seen = std::collections::HashSet::new();
    v.retain(|a| !a.name.trim().is_empty() && seen.insert(a.name.trim().to_lowercase()));
    for a in v.iter_mut() {
        a.name = crate::clip(a.name.trim(), 60);
        a.class = crate::clip(a.class.trim(), 80);
        a.parts.truncate(40);
        a.parts.retain(|p| KINDS.contains(&p.kind.as_str()) && symbols::get(&p.sym).is_some());
        for p in &mut a.parts {
            p.nozzle = crate::clip(&p.nozzle, 120);
            p.tag = crate::clip(p.tag.trim(), 12);
        }
    }
}

/// The arrangement for an object's class: the most specific one (its own class, then its parents).
pub fn for_class<'a>(p: &'a Project, class: &str) -> Option<&'a Arrangement> {
    let chain = crate::classes::chain(class);
    let names: Vec<&str> = if chain.is_empty() { vec![class] } else { chain.iter().map(|c| c.name.as_str()).collect() };
    names.iter().find_map(|n| p.settings.arrangements.iter().find(|a| a.class == *n))
}

/// Whether a nozzle takes flow in (so the parts in its line face it with their outlets).
fn inlet(name: &str) -> bool {
    let n = name.to_lowercase();
    n == "in" || n.starts_with("inlet") || n.contains("suction") || n.ends_with(" in") || n == "feed"
}

const GAP: f64 = 8.0;
const TAP: f64 = 12.0;

/// The item centre that puts local port `k` at `at`, turned `rot` quarters.
fn centre_for(sym: &symbols::Sym, k: usize, rot: u8, at: [f64; 2]) -> [f64; 2] {
    let pl = Place { x: 0.0, y: 0.0, rot, flip: false, scale: 1.0 };
    let off = pl.pt([sym.ports[k].x, sym.ports[k].y]);
    [at[0] - off[0], at[1] - off[1]]
}

/// The turn that makes local direction `local` face `want` on the sheet.
fn rot_for(local: u8, want: u8) -> u8 {
    (want + 4 - local % 4) % 4
}

fn dir_of(v: [f64; 2]) -> u8 {
    if v[0].abs() >= v[1].abs() {
        if v[0] >= 0.0 { 0 } else { 2 }
    } else if v[1] >= 0.0 {
        1
    } else {
        3
    }
}

fn add(a: [f64; 2], b: [f64; 2], s: f64) -> [f64; 2] {
    [a[0] + b[0] * s, a[1] + b[1] * s]
}

/// A point `dist` along a polyline from its start.
fn along(pts: &[[f64; 2]], dist: f64) -> [f64; 2] {
    let mut left = dist.max(0.0);
    for w in pts.windows(2) {
        let len = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        if len >= left && len > 0.0 {
            let t = left / len;
            return [w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t];
        }
        left -= len;
    }
    *pts.last().unwrap_or(&[0.0, 0.0])
}

/// The tag a part gets: its letters with the equipment's number where that tag is free, else
/// the letters alone (the engine then gives the next free loop or tag).
fn tag_for(p: &Project, letters: &str, equip_tag: &str) -> String {
    if letters.is_empty() {
        return String::new();
    }
    let (_, n, suffix) = crate::tags::split(equip_tag);
    if let Some(n) = n {
        let t = format!("{letters}{}{n}{suffix}", p.settings.tag_format.sep);
        if !p.objects.iter().any(|o| o.tag.eq_ignore_ascii_case(&t)) {
            return t;
        }
    }
    letters.to_string()
}

struct Ctx<'a> {
    created: &'a mut Vec<u32>,
}

impl Ctx<'_> {
    fn item(&mut self, p: &mut Project, sheet: u32, sym: &str, at: [f64; 2], rot: u8, tag: String) -> Result<u32, String> {
        let mut out = vec![];
        crate::doc::exec_op(p, crate::doc::Op::AddItem { item: Item { sheet, sym: sym.into(), x: at[0], y: at[1], rot, ..Item::default() }, on: None, tag, obj: None }, &mut out)?;
        let id = out[0];
        self.created.extend(out);
        Ok(id)
    }
    fn line(&mut self, p: &mut Project, sheet: u32, a: End, b: End, kind: Kind, obj: u32) -> Result<u32, String> {
        let mut out = vec![];
        crate::doc::exec_op(p, crate::doc::Op::AddLine { line: Line { sheet, kind, a, b, obj, ..Line::default() } }, &mut out)?;
        let id = out[0];
        self.created.extend(out);
        Ok(id)
    }
}

/// The process line on a port, and whether the port is its `a` end.
fn line_at(p: &Project, item: u32, port: u8) -> Option<(u32, bool)> {
    p.lines.iter().find_map(|l| {
        if !l.kind.process() {
            return None;
        }
        if matches!(l.a, End::Port { item: i, port: k } if i == item && k == port) {
            Some((l.id, true))
        } else if matches!(l.b, End::Port { item: i, port: k } if i == item && k == port) {
            Some((l.id, false))
        } else {
            None
        }
    })
}

/// A line's drawn path, starting from the end at (`item`, `port`).
fn path_from(p: &Project, line: u32, at_a: bool) -> Vec<[f64; 2]> {
    let sheet = p.line(line).map_or(0, |l| l.sheet);
    let mut pts = crate::view::routes(p, sheet, None).remove(&line).unwrap_or_default();
    if !at_a {
        pts.reverse();
    }
    pts
}

/// Places the arrangement `name` (empty: each object's own class's) around the equipment
/// symbols `ids`. Equipment that already has that arrangement is left alone. Returns how many
/// were arranged and what could not be placed.
pub fn apply(p: &mut Project, ids: &[u32], name: &str, created: &mut Vec<u32>) -> Result<(usize, Vec<String>), String> {
    let mut done = 0;
    let mut notes = vec![];
    let mut seen = std::collections::HashSet::new();
    for &id in ids {
        let Some(it) = p.item(id).cloned() else { continue };
        if it.obj == 0 || !seen.insert(it.obj) {
            continue;
        }
        let Some(sym) = symbols::get(&it.sym) else { continue };
        if !(sym.cat.equipment() || sym.cat == Cat::Control) {
            continue;
        }
        let class = p.obj(it.obj).map(|o| o.class.clone()).unwrap_or_default();
        let arr = if name.trim().is_empty() { for_class(p, &class).cloned() } else { p.settings.arrangements.iter().find(|a| a.name.eq_ignore_ascii_case(name.trim())).cloned() };
        let Some(arr) = arr else { continue };
        let tag = p.obj(it.obj).map(|o| o.tag.clone()).unwrap_or_default();
        let who = if tag.trim().is_empty() { sym.name.to_string() } else { tag.clone() };
        if p.data_of(it.obj).get("~arr").is_some_and(|v| v.text().eq_ignore_ascii_case(&arr.name)) {
            notes.push(format!("{who} already has the {} arrangement.", arr.name));
            continue;
        }
        place(p, &it, sym, &arr, &tag, created, &mut notes)?;
        if let Some(o) = p.obj_mut(it.obj) {
            o.data.insert("~arr".into(), Val::Text(arr.name.clone()));
        }
        done += 1;
    }
    if done == 0 && notes.is_empty() {
        return Err("Select a piece of equipment or a control valve that has an arrangement (Options › Arrangements).".into());
    }
    Ok((done, notes))
}

/// A nozzle, the line on the outer end of its chain (and whether that is the line's a end), and where the chain ends.
type Outer = (usize, Option<(u32, bool)>, [f64; 2]);

fn place(p: &mut Project, it: &Item, sym: &symbols::Sym, arr: &Arrangement, tag: &str, created: &mut Vec<u32>, notes: &mut Vec<String>) -> Result<(), String> {
    let mut cx = Ctx { created };
    let sheet = it.sheet;
    // Nozzle groups, in the order they first appear.
    let mut groups: Vec<(usize, Vec<&Part>)> = vec![];
    let mut bypass: Vec<&Part> = vec![];
    for part in &arr.parts {
        if part.kind == "bypass" {
            bypass.push(part);
            continue;
        }
        let k = part.nozzle.split('|').map(|n| n.trim().to_lowercase()).find_map(|n| sym.ports.iter().position(|q| !q.signal && q.name.to_lowercase() == n));
        let Some(k) = k else {
            notes.push(format!("{}: no nozzle “{}” on a {}; that part was left out.", arr.name, part.nozzle, sym.name.to_lowercase()));
            continue;
        };
        match groups.iter_mut().find(|g| g.0 == k) {
            Some(g) => g.1.push(part),
            None => groups.push((k, vec![part])),
        }
    }
    // The outermost line at each nozzle after the in-line parts (for a bypass).
    let mut outer: Vec<Outer> = vec![];
    for (k, parts) in groups {
        let Some((pos, d)) = port_pos(it, k as u8) else { continue };
        let dv = dir_vec(d);
        let into = inlet(sym.ports[k].name);
        let existing = line_at(p, it.id, k as u8);
        let kind = existing.and_then(|(l, _)| p.line(l)).map_or(Kind::Major, |l| l.kind);
        let lobj = existing.and_then(|(l, _)| p.line(l)).map_or(0, |l| l.obj);
        // In-line parts need a line to sit in: on a free nozzle they would leave an open end.
        let inline: Vec<&&Part> = if existing.is_some() { parts.iter().filter(|x| x.kind == "inline").collect() } else { vec![] };
        let branches: Vec<&&Part> = parts.iter().filter(|x| x.kind == "branch").collect();
        // The first straight run from the nozzle is long enough for the branch taps on it.
        let first = GAP + TAP * branches.len() as f64;
        let mut cur = pos;
        let mut from = End::Port { item: it.id, port: k as u8 };
        let mut first_seg: Option<u32> = None;
        let mut obj = lobj;
        for (n, part) in inline.iter().enumerate() {
            let s = symbols::get(&part.sym).ok_or("There is no such symbol.")?;
            let (near, far) = if into { (1usize, 0usize) } else { (0usize, 1usize) };
            if s.ports.len() < 2 {
                continue;
            }
            // The near port faces the nozzle (back along the line).
            let rot = rot_for(s.ports[near].dir, (d + 2) % 4);
            let at = add(cur, dv, if n == 0 { first } else { GAP });
            let c = centre_for(s, near, rot, at);
            let id = cx.item(p, sheet, &part.sym, c, rot, tag_for(p, &part.tag, tag))?;
            let (a, b) = if into { (End::Port { item: id, port: near as u8 }, from.clone()) } else { (from.clone(), End::Port { item: id, port: near as u8 }) };
            let l = cx.line(p, sheet, a, b, kind, obj)?;
            if obj == 0 {
                obj = p.line(l).map_or(0, |x| x.obj);
            }
            first_seg.get_or_insert(l);
            let it2 = p.item(id).cloned().ok_or("lost a part")?;
            cur = port_pos(&it2, far as u8).map_or(cur, |x| x.0);
            from = End::Port { item: id, port: far as u8 };
        }
        // The nozzle's own line now starts at the last in-line part.
        if !inline.is_empty() {
            if let Some((l, at_a)) = existing {
                if let Some(line) = p.lines.iter_mut().find(|x| x.id == l) {
                    if at_a {
                        line.a = from.clone();
                    } else {
                        line.b = from.clone();
                    }
                    line.pts.clear();
                }
            }
        }
        outer.push((k, line_at_end(p, &from, existing), cur));
        // Branches: taps along the first straight run (or along the nozzle's line, or straight onto
        // a free nozzle for the first one).
        let mut prev_inst: Option<u32> = None;
        let mut tap_line = first_seg;
        let mut tap_path: Vec<[f64; 2]> = match first_seg {
            Some(_) => vec![pos, add(pos, dv, first)],
            None => existing.map(|(l, a)| path_from(p, l, a)).unwrap_or_default(),
        };
        if tap_line.is_none() {
            tap_line = existing.map(|x| x.0);
        }
        let mut j = 0usize;
        let mut side = 0usize;
        for part in parts.iter().filter(|x| x.kind == "branch" || x.kind == "controller") {
            let s = symbols::get(&part.sym).ok_or("There is no such symbol.")?;
            let ptag = tag_for(p, &part.tag, tag);
            if part.kind == "controller" {
                // Beside the instrument before it, joined by a signal line.
                let Some(prev) = prev_inst else { continue };
                let pv = p.item(prev).cloned().ok_or("lost a part")?;
                let off = [pv.x - pos[0], pv.y - pos[1]];
                let away = dir_vec(dir_of(off));
                let at = add([pv.x, pv.y], away, 16.0);
                // The controller takes its measurement's loop number.
                let ptag = {
                    let (_, n, sfx) = crate::tags::split(p.tag_of(&pv));
                    let t = n.map(|n| format!("{}{}{n}{sfx}", part.tag, p.settings.tag_format.sep)).filter(|t| !p.objects.iter().any(|o| o.tag.eq_ignore_ascii_case(t)));
                    t.unwrap_or(ptag)
                };
                let id = cx.item(p, sheet, &part.sym, at, 0, ptag)?;
                let s1 = symbols::get(&pv.sym).ok_or("lost a part")?;
                let pa = s1.ports.iter().position(|q| q.dir == dir_of(away)).unwrap_or(0) as u8;
                let pb = s.ports.iter().position(|q| q.dir == (dir_of(away) + 2) % 4).unwrap_or(0) as u8;
                cx.line(p, sheet, End::Port { item: prev, port: pa }, End::Port { item: id, port: pb }, Kind::Electric, 0)?;
                continue;
            }
            let instrument = s.cat.instrument();
            let down = part.sym == "drain";
            // Which side of the line: drains below, the rest above; on an upright line, alternate sides.
            let horizontal = d.is_multiple_of(2);
            let nd = if horizontal {
                if down { 1 } else { 3 }
            } else {
                side += 1;
                if side % 2 == 1 { 0 } else { 2 }
            };
            let nv = dir_vec(nd);
            let reach = if instrument { 14.0 } else { 10.0 };
            // The port that connects to the process: the first process port, or for an instrument
            // bubble the side facing the line.
            let ck = if instrument { s.ports.iter().position(|q| q.dir == (nd + 2) % 4).unwrap_or(0) } else { s.ports.iter().position(|q| !q.signal).unwrap_or(0) };
            let kind_b = if instrument { Kind::Minor } else { kind };
            if tap_line.is_none() {
                // A free nozzle: the first branch goes straight onto it.
                let rot = if instrument { 0 } else { rot_for(s.ports[ck].dir, (d + 2) % 4) };
                let ck = if instrument { s.ports.iter().position(|q| q.dir == (d + 2) % 4).unwrap_or(0) } else { ck };
                let run = 14.0 + TAP * (branches.len().saturating_sub(1)) as f64;
                let at = add(pos, dv, run);
                let c = centre_for(s, ck, rot, at);
                let id = cx.item(p, sheet, &part.sym, c, rot, ptag)?;
                let l = cx.line(p, sheet, End::Port { item: it.id, port: k as u8 }, End::Port { item: id, port: ck as u8 }, kind_b, 0)?;
                // A new line number: its fluid, class and schedule from the equipment's other lines.
                if !instrument {
                    let sib = p.lines.iter().filter(|x| x.id != l && x.kind.process() && x.obj != 0 && [&x.a, &x.b].iter().any(|e| e.item().is_some_and(|(i, _)| i == it.id))).map(|x| x.obj).next();
                    let lobj = p.line(l).map_or(0, |x| x.obj);
                    if let (Some(sib), true) = (sib, lobj != 0) {
                        let from = p.data_of(sib).clone();
                        if let Some(o) = p.obj_mut(lobj) {
                            for key in ["fluid", "class", "sched"] {
                                if let Some(v) = from.get(key) {
                                    o.data.entry(key.to_string()).or_insert_with(|| v.clone());
                                }
                            }
                            let what = if part.sym == "psv" { "relief" } else if part.sym == "drain" { "drain" } else { "vent" };
                            o.data.entry("service".to_string()).or_insert_with(|| Val::Text(format!("{} {what}", if tag.is_empty() { "Equipment" } else { tag })));
                        }
                    }
                }
                tap_line = Some(l);
                tap_path = vec![pos, at];
                if instrument {
                    prev_inst = Some(id);
                }
                continue;
            }
            let Some(tl) = tap_line else { continue };
            if tap_path.len() < 2 {
                notes.push(format!("{}: a {} could not be tapped on.", arr.name, s.name.to_lowercase()));
                continue;
            }
            let at_line = along(&tap_path, 6.0 + TAP * j as f64);
            j += 1;
            let rot = if instrument { 0 } else { rot_for(s.ports[ck].dir, (nd + 2) % 4) };
            let c = centre_for(s, ck, rot, add(at_line, nv, reach));
            let id = cx.item(p, sheet, &part.sym, c, rot, ptag)?;
            // A branch continues the line number it is tapped on.
            let tobj = if instrument { 0 } else { p.line(tl).map_or(0, |l| l.obj) };
            cx.line(p, sheet, End::Port { item: id, port: ck as u8 }, End::Line { line: tl, x: at_line[0], y: at_line[1] }, kind_b, tobj)?;
            if instrument {
                prev_inst = Some(id);
            }
        }
    }
    // A bypass round the equipment: from its inlet's outer line to its outlet's.
    for part in bypass {
        let ins: Vec<_> = outer.iter().filter(|o| inlet(sym.ports[o.0].name)).collect();
        let outs: Vec<_> = outer.iter().filter(|o| !inlet(sym.ports[o.0].name)).collect();
        let (Some(i), Some(o)) = (ins.first(), outs.first()) else {
            notes.push(format!("{}: the bypass needs parts on an inlet and an outlet.", arr.name));
            continue;
        };
        let (Some((li, ai)), Some((lo, ao))) = (i.1, o.1) else {
            notes.push(format!("{}: draw the lines in and out first; the bypass was left out.", arr.name));
            continue;
        };
        let pi = along(&path_from(p, li, ai), 6.0);
        let po = along(&path_from(p, lo, ao), 6.0);
        let s = symbols::get(&part.sym).ok_or("There is no such symbol.")?;
        if s.ports.len() < 2 {
            continue;
        }
        let flow = [po[0] - pi[0], po[1] - pi[1]];
        let fd = dir_of(flow);
        let nd = if fd.is_multiple_of(2) { 1 } else { 0 };
        let mid = add([(pi[0] + po[0]) / 2.0, (pi[1] + po[1]) / 2.0], dir_vec(nd), 28.0);
        let rot = rot_for(s.ports[1].dir, fd);
        let id = cx.item(p, sheet, &part.sym, mid, rot, tag_for(p, &part.tag, tag))?;
        let kind = p.line(li).map_or(Kind::Major, |l| l.kind);
        let obj = p.line(li).map_or(0, |l| l.obj);
        cx.line(p, sheet, End::Line { line: li, x: pi[0], y: pi[1] }, End::Port { item: id, port: 0 }, kind, obj)?;
        cx.line(p, sheet, End::Port { item: id, port: 1 }, End::Line { line: lo, x: po[0], y: po[1] }, kind, obj)?;
    }
    Ok(())
}

/// The line on the outer end of a nozzle's chain (the nozzle's own line, moved there).
fn line_at_end(p: &Project, from: &End, existing: Option<(u32, bool)>) -> Option<(u32, bool)> {
    if let End::Port { item, port } = from {
        if let Some(x) = line_at(p, *item, *port) {
            return Some(x);
        }
    }
    existing
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    fn tag_item(d: &Doc, t: &str) -> u32 {
        d.p.items.iter().find(|i| d.p.tag_of(i) == t).unwrap_or_else(|| panic!("{t}")).id
    }

    #[test]
    fn every_starter_arrangement_places_cleanly_on_the_examples() {
        for ex in ["pump", "compressor", "exchanger"] {
            let mut d = Doc::new(crate::examples::get(ex).unwrap());
            let errs = |p: &Project| crate::api::derived(p, 0).issues.into_iter().filter(|i| i.level == "error").map(|i| i.msg).collect::<Vec<_>>();
            let errors0 = errs(&d.p).len();
            let ids: Vec<u32> = d.p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| s.cat.equipment() || s.cat == Cat::Control)).map(|i| i.id).collect();
            let n0 = d.p.items.len();
            d.apply(Op::Arrange { ids: ids.clone(), name: String::new() }).unwrap_or_else(|e| panic!("{ex}: {e}"));
            assert!(d.p.items.len() > n0, "{ex}: nothing placed");
            let after = errs(&d.p);
            assert!(after.len() <= errors0, "{ex}: {after:#?}");
            // Applying again adds nothing.
            let n1 = d.p.items.len();
            d.apply(Op::Arrange { ids, name: String::new() }).ok();
            assert_eq!(d.p.items.len(), n1, "{ex}: applied twice");
            // Undo takes it all back.
            d.undo();
            assert_eq!(d.p.items.len(), n0, "{ex}: undo");
        }
    }

    #[test]
    fn a_pump_gets_its_valves_in_the_line_and_its_gauge_and_tags() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        // P-101B has its discharge line and check valve already; use a new pump on a fresh sheet.
        let s = d.p.sheets[0].id;
        let made = d.apply(Op::AddItem { item: Item { sheet: s, sym: "pump_cent".into(), x: 300.0, y: 250.0, ..Item::default() }, on: None, tag: "P-301".into(), obj: None }).unwrap();
        let pump = made[0];
        let tank = d.apply(Op::AddItem { item: Item { sheet: s, sym: "vvessel".into(), x: 250.0, y: 250.0, ..Item::default() }, on: None, tag: "V-301".into(), obj: None }).unwrap()[0];
        d.apply(Op::AddLine { line: Line { sheet: s, kind: Kind::Major, a: End::Port { item: tank, port: 1 }, b: End::Port { item: pump, port: 0 }, ..Line::default() } }).unwrap();
        let l = *d.p.lines.last().map(|l| &l.id).unwrap();
        let drum = d.apply(Op::AddItem { item: Item { sheet: s, sym: "vvessel".into(), x: 380.0, y: 250.0, ..Item::default() }, on: None, tag: "V-302".into(), obj: None }).unwrap()[0];
        d.apply(Op::AddLine { line: Line { sheet: s, kind: Kind::Major, a: End::Port { item: pump, port: 1 }, b: End::Port { item: drum, port: 2 }, ..Line::default() } }).unwrap();
        d.apply(Op::SetData { ids: vec![l], key: "size".into(), val: Some(Val::Text("4".into())) }).unwrap();
        d.apply(Op::Arrange { ids: vec![pump], name: String::new() }).unwrap();
        // The suction line now runs tank → gate → strainer → pump, all one line number, size 4".
        let sym_of = |id: u32| d.p.item(id).map(|i| i.sym.clone()).unwrap_or_default();
        let at_pump = d.p.lines.iter().find(|l| l.b == End::Port { item: pump, port: 0 } || l.a == End::Port { item: pump, port: 0 }).unwrap();
        let other = if at_pump.a == (End::Port { item: pump, port: 0 }) { &at_pump.b } else { &at_pump.a };
        assert_eq!(sym_of(other.item().unwrap().0), "strainer_y");
        let strainer = other.item().unwrap().0;
        assert_eq!(d.p.data_of(d.p.item(strainer).unwrap().obj).get("size").map(|v| v.text()), Some("4".into()));
        // The pressure gauge takes the pump's number; the valves the next free HV tags.
        assert!(d.p.objects.iter().any(|o| o.tag == "PI-301"));
        let _ = tag_item(&d, "PI-301");
        assert!(d.p.objects.iter().filter(|o| o.tag.starts_with("HV-")).count() >= 2);
        // A check valve on the discharge, facing away from the pump.
        let chk = d.p.items.iter().find(|i| i.sym == "check" && d.p.lines.iter().any(|l| l.a == End::Port { item: pump, port: 1 } && l.b == End::Port { item: i.id, port: 0 })).is_some();
        assert!(chk, "check valve on the discharge, inlet towards the pump");
    }

    #[test]
    fn a_control_valve_station_gets_block_valves_and_a_bypass() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let lv = tag_item(&d, "LV-101");
        let n0 = d.p.items.len();
        d.apply(Op::Arrange { ids: vec![lv], name: "Control valve station".into() }).unwrap();
        let new: Vec<&Item> = d.p.items.iter().skip(n0).collect();
        assert_eq!(new.iter().filter(|i| i.sym == "gate").count(), 2);
        assert_eq!(new.iter().filter(|i| i.sym == "globe").count(), 1);
        let globe = new.iter().find(|i| i.sym == "globe").unwrap().id;
        assert_eq!(d.p.lines.iter().filter(|l| [&l.a, &l.b].iter().any(|e| e.item().is_some_and(|x| x.0 == globe))).count(), 2);
        // Hostile arrangements are cleaned, and nothing selected says so.
        let mut v = vec![Arrangement { name: " ".into(), ..Default::default() }, Arrangement { name: "X".into(), class: "Pump".into(), parts: vec![part("nowhere", "inline", "gate", ""), part("suction", "??", "gate", ""), part("suction", "branch", "no_such", "")] }];
        normalize(&mut v);
        assert_eq!((v.len(), v[0].parts.len()), (1, 1));
        assert!(d.apply(Op::Arrange { ids: vec![], name: String::new() }).is_err());
    }

    #[test]
    fn any_arrangement_on_any_symbol_never_panics() {
        for ex in ["pump", "compressor", "exchanger", "faults", "pfd"] {
            let p = crate::examples::get(ex).unwrap();
            let names: Vec<String> = std::iter::once(String::new()).chain(starter().into_iter().map(|a| a.name)).collect();
            for it in p.items.iter().take(60) {
                for n in &names {
                    let mut d = Doc::new(p.clone());
                    let _ = d.apply(Op::Arrange { ids: vec![it.id, it.id, 999_999], name: n.clone() });
                }
            }
        }
    }
}
