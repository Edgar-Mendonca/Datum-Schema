//! Old P&IDs turned into live sheets. A DXF drawing's blocks become Datum Schema's symbols (through
//! a client library that names the block, else by the block's name, else they are left out, the
//! user's review deciding), placed where the blocks were with their turn and mirror; tags come from
//! the blocks' attributes or the nearest tag-like text. The drawing's lines become lines: their
//! kind from their layer, their corners kept as bend points, pieces cut by crossing gaps joined
//! again, ends snapped to the nearest nozzle of a symbol or onto another line, valves drawn over a
//! line split into it, and line numbers read from the text beside them. The old drawing stays behind
//! the new sheet to compare with, and a report says what was converted and what was not.

use crate::doc::{exec_op, Op};
use crate::dxfin::{self, Ent, Tf};
use crate::model::{End, Item, Kind, Line, Project, Sheet, Val, LINE_CLASS};
use crate::symbols;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// One block of the drawing and the symbol it becomes.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BlockUse {
    pub block: String,
    pub count: usize,
    /// The suggested symbol key ("" to leave it out).
    pub sym: String,
    /// How it was chosen: library, name or none.
    pub how: &'static str,
    /// A tag one of them carries, to recognise the block by.
    pub sample: String,
}

/// What a drawing holds, for the review before converting.
#[derive(Clone, Debug, Serialize, PartialEq, Default)]
pub struct Scan {
    pub blocks: Vec<BlockUse>,
    pub lines: usize,
    pub texts: usize,
    pub warnings: Vec<String>,
}

/// What a conversion did.
#[derive(Clone, Debug, Serialize, PartialEq, Default)]
pub struct Report {
    pub sheet: u32,
    pub symbols: usize,
    pub tagged: usize,
    pub left_out: Vec<(String, usize)>,
    pub lines: usize,
    pub joined_ends: usize,
    pub open_ends: usize,
    pub split: usize,
    pub line_numbers: usize,
    pub notes: Vec<String>,
}

/// The symbol a block stands for: a library symbol of that block name, else a suggestion from the
/// block's name (and a description attribute), else none.
fn choose(block: &str, desc: &str) -> (String, &'static str) {
    if let Some(d) = crate::symlib::registered().iter().filter_map(|s| crate::symlib::definition(s.key)).find(|d| d.block.eq_ignore_ascii_case(block) && !d.like.is_empty()) {
        return (d.like, "library");
    }
    // Our own drawings name blocks SYM_<KEY>.
    if let Some(k) = block.strip_prefix("SYM_").map(|k| k.to_lowercase()).filter(|k| symbols::builtin(k).is_some()) {
        return (k, "name");
    }
    match crate::symlib::suggest(desc, &block.replace(['_', '-', '$'], " ")) {
        Some((k, score)) if score >= 0.5 => (k.to_string(), "name"),
        _ => (String::new(), "none"),
    }
}

fn desc_of(attribs: &[dxfin::Attrib]) -> String {
    attribs.iter().filter(|a| { let t = a.tag.to_uppercase(); t.contains("DESC") || t.contains("TYPE") || t.contains("NAME") }).map(|a| a.value.clone()).collect::<Vec<_>>().join(" ")
}

/// Does a text look like a tag (letters, then a number: P-101A, FIC 101)?
fn tag_like(s: &str) -> bool {
    let t = s.trim();
    let (pre, num, _) = crate::tags::split(t);
    num.is_some() && !pre.is_empty() && pre.len() <= 6 && pre.chars().all(|c| c.is_ascii_alphabetic()) && t.len() <= 16 && !t.contains('"')
}

/// Does a text look like a line number (3"-P-1003-A1A, 80-P-1003-A1A)?
fn number_like(s: &str) -> bool {
    let t = s.trim();
    t.matches('-').count() >= 2 && t.chars().any(|c| c.is_ascii_digit()) && t.chars().any(|c| c.is_ascii_alphabetic()) && t.len() <= 40 && !t.contains(' ')
}

/// A block's tag from its attributes: a TAG attribute, else letters and number joined, else any
/// value that looks like a tag.
fn tag_from(attribs: &[dxfin::Attrib]) -> Option<String> {
    let vals: Vec<(String, String)> = attribs.iter().map(|a| (a.tag.to_uppercase(), a.value.trim().to_string())).filter(|a| !a.1.is_empty()).collect();
    if let Some((_, v)) = vals.iter().find(|(t, v)| ["TAG", "TAGNO", "TAG_NO", "TAGNUMBER", "TAG_NUMBER", "ITEM", "ITEMNO", "EQUIPNO", "EQUIP_NO"].contains(&t.as_str()) && tag_like(v)) {
        return Some(v.clone());
    }
    let letters = vals.iter().find(|(_, v)| (1..=5).contains(&v.len()) && v.chars().all(|c| c.is_ascii_alphabetic()));
    let number = vals.iter().find(|(_, v)| v.chars().next().is_some_and(|c| c.is_ascii_digit()) && v.len() <= 8 && v.chars().all(|c| c.is_ascii_alphanumeric()));
    if let (Some(l), Some(n)) = (letters, number) {
        return Some(format!("{}-{}", l.1.to_uppercase(), n.1));
    }
    vals.into_iter().map(|x| x.1).find(|v| tag_like(v))
}

fn kind_of(layer: &str) -> Kind {
    let u = layer.to_uppercase();
    let has = |w: &[&str]| w.iter().any(|x| u.contains(x));
    if has(&["PNEU"]) {
        Kind::Pneumatic
    } else if has(&["CAPIL"]) {
        Kind::Capillary
    } else if has(&["DATA", "SOFT", "FIELDBUS"]) {
        Kind::Data
    } else if has(&["ELEC", "SIG", "INSTR", "CTRL", "CONTROL"]) {
        Kind::Electric
    } else if has(&["UTIL"]) {
        Kind::Utility
    } else if has(&["MINOR", "SECOND"]) {
        Kind::Minor
    } else {
        Kind::Major
    }
}

/// Layers that are not process or signal lines.
fn skip_layer(layer: &str) -> bool {
    let u = layer.to_uppercase();
    ["FRAME", "BORDER", "TITLE", "TEXT", "DIM", "NOTE", "MARK", "BACKGROUND", "REVISION", "HAZOP", "HATCH", "DEFPOINTS", "VIEWPORT", "SYM-"].iter().any(|w| u.contains(w))
}

/// Reads a drawing for the review: its blocks with the symbols suggested, its lines and texts.
pub fn scan(text: &str) -> Result<Scan, String> {
    let d = dxfin::read(text)?;
    let mut by: BTreeMap<String, (usize, String, String)> = BTreeMap::new();
    let (mut lines, mut texts) = (0, 0);
    for (k, e) in d.ents.iter().enumerate() {
        match e {
            Ent::Insert { name, attribs, .. } => {
                let x = by.entry(name.clone()).or_insert((0, desc_of(attribs), String::new()));
                x.0 += 1;
                if x.2.is_empty() {
                    x.2 = tag_from(attribs).unwrap_or_default();
                }
            }
            Ent::Poly { closed: false, .. } if !skip_layer(d.layers.get(k).map_or("", |s| s.as_str())) => lines += 1,
            Ent::Text { .. } => texts += 1,
            _ => {}
        }
    }
    let mut blocks: Vec<BlockUse> = by.into_iter().map(|(block, (count, desc, sample))| { let (sym, how) = choose(&block, &desc); BlockUse { block, count, sym, how, sample } }).collect();
    blocks.sort_by(|a, b| b.count.cmp(&a.count).then(a.block.cmp(&b.block)));
    let mut warnings = vec![];
    if blocks.is_empty() {
        warnings.push("The drawing has no blocks: its symbols are loose lines that cannot be recognised. Put it behind a sheet (Sheets › Old Drawing) and trace over it.".into());
    }
    if !d.skipped.is_empty() {
        warnings.push(format!("Not read: {}.", d.skipped.iter().map(|(k, v)| format!("{v} {k}")).collect::<Vec<_>>().join(", ")));
    }
    Ok(Scan { blocks, lines, texts, warnings })
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// The distance from a point to a segment, and the nearest point on it.
fn to_seg(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, [f64; 2]) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
    let q = [a[0] + t * dx, a[1] + t * dy];
    (dist(p, q), q)
}

/// Joins pieces of one line cut by a gap (where a crossing line hops over): two ends close
/// together, each pointing at the other.
fn join_gaps(chains: &mut Vec<(Vec<[f64; 2]>, Kind)>, gap: f64) -> usize {
    let mut joined = 0;
    'again: loop {
        for i in 0..chains.len() {
            for j in 0..chains.len() {
                if i == j || chains[i].1 != chains[j].1 {
                    continue;
                }
                let (a, b) = (&chains[i].0, &chains[j].0);
                // The end of a, the start of b (each chain can be reversed).
                for (ra, rb) in [(false, false), (false, true), (true, false), (true, true)] {
                    let (ae, ap) = if ra { (a[0], a[1]) } else { (a[a.len() - 1], a[a.len() - 2]) };
                    let (bs, bn) = if rb { (b[b.len() - 1], b[b.len() - 2]) } else { (b[0], b[1]) };
                    let d = dist(ae, bs);
                    if d > gap {
                        continue;
                    }
                    let out = [ae[0] - ap[0], ae[1] - ap[1]];
                    let into = [bn[0] - bs[0], bn[1] - bs[1]];
                    let unit = |v: [f64; 2]| { let n = v[0].hypot(v[1]).max(1e-9); [v[0] / n, v[1] / n] };
                    let (o, n) = (unit(out), unit(into));
                    let across = if d > 1e-6 { unit([bs[0] - ae[0], bs[1] - ae[1]]) } else { o };
                    if o[0] * n[0] + o[1] * n[1] > 0.98 && o[0] * across[0] + o[1] * across[1] > 0.98 {
                        let mut a2 = chains[i].0.clone();
                        if ra {
                            a2.reverse();
                        }
                        let mut b2 = chains[j].0.clone();
                        if rb {
                            b2.reverse();
                        }
                        if d < 1e-6 {
                            b2.remove(0);
                        }
                        a2.extend(b2);
                        let k = chains[i].1;
                        let (hi, lo) = (i.max(j), i.min(j));
                        chains.remove(hi);
                        chains.remove(lo);
                        chains.push((a2, k));
                        joined += 1;
                        continue 'again;
                    }
                }
            }
        }
        break;
    }
    joined
}

/// Drops points that do not turn a corner.
fn simplify(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = vec![];
    for &p in pts {
        if out.last().is_some_and(|q| dist(*q, p) < 1e-3) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let cross = (b[0] - a[0]) * (p[1] - b[1]) - (b[1] - a[1]) * (p[0] - b[0]);
            if cross.abs() < 1e-6 * dist(a, b).max(1e-9) * dist(b, p).max(1e-9) {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// Converts a DXF drawing into a new sheet of the project, the blocks becoming the symbols `map`
/// names (block → symbol key; "" or missing: left out).
pub fn convert(p: &mut Project, text: &str, name: &str, map: &BTreeMap<String, String>, created: &mut Vec<u32>) -> Result<Report, String> {
    let d = dxfin::read(text)?;
    let mut prims = vec![];
    dxfin::flatten(&d, &d.ents, Tf::ID, &mut prims, 0);
    let b = dxfin::bbox(&prims).ok_or("That drawing has nothing to convert.")?;
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    // The smallest sheet it fits on, else A1 (scaled down).
    let mut sheet = Sheet { number: next_number(p), title: format!("Converted from {}", crate::clip(name, 80)), rev: "A".into(), ..Sheet::default() };
    for size in ["A3", "A1"] {
        sheet.size = size.into();
        let (x0, y0, x1, y1) = crate::frame::inner(&sheet);
        if w <= x1 - x0 && h <= y1 - y0 {
            break;
        }
    }
    // The old drawing behind it, and the same placement for what is converted.
    let (u, mut notes) = dxfin::underlay(text, name, &sheet)?;
    let (k, ux, uy) = (u.k, u.x, u.y);
    sheet.underlay = Some(u);
    let to = |q: [f64; 2]| [(q[0] * d.mm - b[0]) * k + ux, (-q[1] * d.mm - b[1]) * k + uy];
    let mut out = vec![];
    exec_op(p, Op::AddSheet { sheet }, &mut out)?;
    let sid = out[0];
    created.extend(out);
    let mut rep = Report { sheet: sid, ..Report::default() };

    // Texts (for tags and line numbers), in sheet millimetres.
    let mut texts: Vec<([f64; 2], String, bool)> = d.ents.iter().filter_map(|e| match e { Ent::Text { p, s, .. } => Some((to(*p), s.trim().to_string(), false)), _ => None }).collect();
    let mut taken: HashSet<String> = p.objects.iter().map(|o| o.tag.to_uppercase()).filter(|t| !t.is_empty()).collect();

    // Symbols.
    let mut left: BTreeMap<String, usize> = BTreeMap::new();
    let mut placed: Vec<u32> = vec![];
    let mut clashed: Vec<String> = vec![];
    for e in &d.ents {
        let Ent::Insert { name: block, p: at, sx, sy, rot, attribs } = e else { continue };
        let sym = map.get(block).cloned().unwrap_or_default();
        if sym.is_empty() || symbols::get(&sym).is_none() {
            *left.entry(block.clone()).or_default() += 1;
            continue;
        }
        let pos = to(*at);
        let q = ((rot / 90.0).round() as i64).rem_euclid(4) as u8;
        let item = Item { sheet: sid, sym: sym.clone(), x: pos[0], y: pos[1], rot: (4 - q) % 4, flip: sx * sy < 0.0, ..Item::default() };
        // The tag: its attributes, else the nearest free text that looks like a tag.
        let mut tag = tag_from(attribs).map(|t| t.to_uppercase());
        if tag.is_none() {
            let near = texts.iter_mut().filter(|t| !t.2 && tag_like(&t.1) && dist(t.0, pos) < 14.0).min_by(|a, b| dist(a.0, pos).total_cmp(&dist(b.0, pos)));
            if let Some(t) = near {
                t.2 = true;
                tag = Some(t.1.replace(' ', "-").to_uppercase());
            }
        }
        if tag.as_ref().is_some_and(|t| taken.contains(t)) {
            clashed.push(tag.clone().unwrap_or_default());
        }
        let tag = tag.filter(|t| !taken.contains(t));
        if let Some(t) = &tag {
            taken.insert(t.clone());
            rep.tagged += 1;
        }
        let mut o = vec![];
        if exec_op(p, Op::AddItem { item, on: None, tag: tag.unwrap_or_default(), obj: None }, &mut o).is_ok() {
            rep.symbols += 1;
            placed.extend(o.iter().copied());
            created.extend(o);
        }
    }
    rep.left_out = left.into_iter().collect();
    rep.left_out.sort_by_key(|x| std::cmp::Reverse(x.1));

    // Lines: open polylines on line layers, in sheet millimetres.
    let mut chains: Vec<(Vec<[f64; 2]>, Kind)> = vec![];
    for (i, e) in d.ents.iter().enumerate() {
        let Ent::Poly { pts, closed: false, .. } = e else { continue };
        let layer = d.layers.get(i).map_or("", |s| s.as_str());
        if skip_layer(layer) || pts.len() < 2 {
            continue;
        }
        let pts: Vec<[f64; 2]> = simplify(&pts.iter().map(|q| to(*q)).collect::<Vec<_>>());
        if pts.len() >= 2 {
            chains.push((pts, kind_of(layer)));
        }
    }
    rep.joined_ends = join_gaps(&mut chains, 4.0);
    // Short marks (ticks, arrow tails) are not lines.
    chains.retain(|c| c.0.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>() >= 4.0);

    // Ends on nozzles: the nearest free port of a symbol whose outline the end is in or next to.
    let mut used: HashSet<(u32, u8)> = HashSet::new();
    let mut ends: Vec<[End; 2]> = vec![];
    for (pts, _) in &chains {
        let mut pair = [End::Free { x: pts[0][0], y: pts[0][1] }, End::Free { x: pts[pts.len() - 1][0], y: pts[pts.len() - 1][1] }];
        for (n, e) in [pts[0], pts[pts.len() - 1]].into_iter().enumerate() {
            let mut best: Option<(f64, u32, u8)> = None;
            for &id in &placed {
                let Some(it) = p.item(id) else { continue };
                let bb = crate::view::bbox(it);
                let m = 3.0;
                if e[0] < bb[0] - m || e[0] > bb[2] + m || e[1] < bb[1] - m || e[1] > bb[3] + m {
                    continue;
                }
                let s = symbols::get(&it.sym).map_or(0, |s| s.ports.len());
                for k in 0..s as u8 {
                    if used.contains(&(id, k)) {
                        continue;
                    }
                    if let Some((q, _)) = crate::graph::port_pos(it, k) {
                        let dd = dist(q, e);
                        if dd < 12.0 && best.is_none_or(|b| dd < b.0) {
                            best = Some((dd, id, k));
                        }
                    }
                }
            }
            if let Some((_, id, k)) = best {
                used.insert((id, k));
                pair[n] = End::Port { item: id, port: k };
            }
        }
        ends.push(pair);
    }
    // The lines, their corners kept as bend points.
    let mut ids: Vec<u32> = vec![];
    for ((pts, kind), [a, b2]) in chains.iter().zip(ends) {
        let bends = if pts.len() > 2 { pts[1..pts.len() - 1].to_vec() } else { vec![] };
        let mut o = vec![];
        if exec_op(p, Op::AddLine { line: Line { sheet: sid, kind: *kind, a, b: b2, pts: bends, ..Line::default() } }, &mut o).is_ok() {
            ids.push(o.first().copied().unwrap_or(0));
            created.extend(o);
        } else {
            ids.push(0);
        }
    }
    rep.lines = ids.iter().filter(|x| **x != 0).count();
    // Free ends on another line: a branch.
    for (ci, &id) in ids.iter().enumerate() {
        let Some(l) = p.line(id).cloned() else { continue };
        let mut nl = l.clone();
        let mut changed = false;
        for e in [&mut nl.a, &mut nl.b] {
            let End::Free { x, y } = *e else { continue };
            let hit = chains.iter().enumerate().filter(|(cj, _)| *cj != ci && ids[*cj] != 0).find_map(|(cj, c)| c.0.windows(2).find_map(|s| { let (dd, q) = to_seg([x, y], s[0], s[1]); (dd < 1.5).then_some((ids[cj], q)) }));
            if let Some((other, q)) = hit {
                *e = End::Line { line: other, x: q[0], y: q[1] };
                changed = true;
            }
        }
        if changed {
            let _ = exec_op(p, Op::SetLine { line: nl }, &mut vec![]);
        }
    }
    // Valves and fittings drawn over a line: split into it.
    for &id in &placed {
        let Some(it) = p.item(id).cloned() else { continue };
        if !symbols::get(&it.sym).is_some_and(|s| s.inline) || p.lines.iter().any(|l| [&l.a, &l.b].iter().any(|e| e.item().is_some_and(|x| x.0 == id))) {
            continue;
        }
        let routes = crate::view::routes(p, sid, Some(id));
        let on = p.lines.iter().filter(|l| l.sheet == sid && l.kind.process()).find(|l| routes.get(&l.id).is_some_and(|r| r.windows(2).any(|s| to_seg([it.x, it.y], s[0], s[1]).0 < 1.5))).map(|l| l.id);
        if let Some(l) = on {
            if exec_op(p, Op::Split { line: l, item: id }, &mut vec![]).is_ok() {
                rep.split += 1;
            }
        }
    }
    // Line numbers from the text beside each line, shared by every piece of the same number.
    let routes = crate::view::routes(p, sid, None);
    let mut by_number: BTreeMap<String, u32> = BTreeMap::new();
    for t in texts.iter().filter(|t| !t.2 && number_like(&t.1)) {
        let near = p.lines.iter().filter(|l| l.sheet == sid && l.kind.process()).filter_map(|l| routes.get(&l.id).map(|r| (l.id, r.windows(2).map(|s| to_seg(t.0, s[0], s[1]).0).fold(f64::MAX, f64::min)))).filter(|x| x.1 < 10.0).min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((lid, _)) = near else { continue };
        let Some(obj) = p.line(lid).map(|l| l.obj) else { continue };
        let num = t.1.clone();
        match by_number.get(&num) {
            Some(&first) if first != obj => {
                for l in p.lines.iter_mut().filter(|l| l.obj == obj) {
                    l.obj = first;
                }
            }
            _ => {
                by_number.insert(num.clone(), obj);
                if let Some(o) = p.obj_mut(obj) {
                    o.data.insert("number".into(), Val::Text(num));
                }
            }
        }
    }
    // A piece with no number beside it takes the number of the piece across a valve or fitting.
    for _ in 0..20 {
        let mut changed = false;
        let numbered: HashSet<u32> = by_number.values().copied().collect();
        let pieces: Vec<Line> = p.lines.iter().filter(|l| l.sheet == sid && l.kind.process()).cloned().collect();
        for l in &pieces {
            if numbered.contains(&l.obj) {
                continue;
            }
            let across = [&l.a, &l.b].iter().filter_map(|e| e.item()).find_map(|(it, port)| {
                let s = symbols::get(&p.item(it)?.sym)?;
                if !s.inline {
                    return None;
                }
                let other = s.ports.iter().enumerate().find(|(k, q)| !q.signal && *k as u8 != port)?.0 as u8;
                pieces.iter().find(|m| m.id != l.id && numbered.contains(&m.obj) && [&m.a, &m.b].contains(&&End::Port { item: it, port: other })).map(|m| m.obj)
            });
            if let Some(o) = across {
                let old = l.obj;
                for x in p.lines.iter_mut().filter(|x| x.obj == old) {
                    x.obj = o;
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    rep.line_numbers = by_number.len();
    // Line-number objects left with no line go.
    let in_use: BTreeSet<u32> = p.lines.iter().map(|l| l.obj).collect();
    p.objects.retain(|o| o.class != LINE_CLASS || in_use.contains(&o.id));
    rep.open_ends = p.lines.iter().filter(|l| l.sheet == sid).flat_map(|l| [&l.a, &l.b]).filter(|e| matches!(e, End::Free { .. })).count();
    if !clashed.is_empty() {
        let n = clashed.len();
        clashed.truncate(8);
        notes.push(format!("{n} tags are already used in the project and were left off (tag them by hand): {}{}.", clashed.join(", "), if n > 8 { "…" } else { "" }));
    }
    if !rep.left_out.is_empty() {
        notes.push(format!("Left out: {}.", rep.left_out.iter().map(|(b, n)| format!("{n} × {b}")).collect::<Vec<_>>().join(", ")));
    }
    rep.notes = notes;
    Ok(rep)
}

/// The next free sheet number for a converted drawing.
fn next_number(p: &Project) -> String {
    (1..).map(|n| format!("CNV-{n:03}")).find(|n| !p.sheets.iter().any(|s| s.number.eq_ignore_ascii_case(n))).unwrap_or_else(|| "CNV".into())
}

/// The block → symbol choices a scan suggests.
pub fn suggested(s: &Scan) -> BTreeMap<String, String> {
    s.blocks.iter().filter(|b| !b.sym.is_empty()).map(|b| (b.block.clone(), b.sym.clone())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;

    #[test]
    fn our_own_dxf_comes_back_as_a_live_sheet() {
        let src = crate::examples::get("pump").unwrap();
        let text = crate::dxf::write(&src, &src.sheets[0]);
        let s = scan(&text).unwrap();
        // Every block is recognised (our own drawings name them SYM_<KEY>).
        assert!(s.blocks.iter().all(|b| !b.sym.is_empty()), "{:?}", s.blocks.iter().filter(|b| b.sym.is_empty()).collect::<Vec<_>>());
        assert!(s.lines > 10);
        let mut d = Doc::new(Project::new());
        d.apply(Op::ConvertDxf { text: text.clone(), name: "PID-100.dxf".into(), map: suggested(&s) }).unwrap();
        let sid = d.p.sheets.last().unwrap().id;
        assert!(d.p.sheet(sid).unwrap().underlay.is_some(), "the old drawing behind it");
        let tags = |p: &Project| -> BTreeSet<String> { p.items.iter().filter(|i| i.sheet == sid || i.sheet == p.sheets[0].id).map(|i| p.tag_of(i).to_string()).filter(|t| !t.is_empty()).collect() };
        let want: BTreeSet<String> = src.items.iter().map(|i| src.tag_of(i).to_string()).filter(|t| !t.is_empty()).collect();
        let got = tags(&d.p);
        let missing: Vec<&String> = want.difference(&got).collect();
        assert!(missing.is_empty(), "tags not read: {missing:?}");
        // Symbols where they were: P-101A within a millimetre of the original, relative to T-101.
        let at = |p: &Project, t: &str| p.items.iter().find(|i| p.tag_of(i) == t).map(|i| [i.x, i.y]).unwrap();
        let (a0, a1) = (at(&src, "P-101A"), at(&src, "T-101"));
        let (b0, b1) = (at(&d.p, "P-101A"), at(&d.p, "T-101"));
        assert!(((a0[0] - a1[0]) - (b0[0] - b1[0])).abs() < 1.0 && ((a0[1] - a1[1]) - (b0[1] - b1[1])).abs() < 1.0);
        // Lines joined to the nozzles: most ends on symbols, the pumps' suction and discharge wired.
        let on = |t: &str| { let id = d.p.items.iter().find(|i| d.p.tag_of(i) == t).unwrap().id; d.p.lines.iter().filter(|l| [&l.a, &l.b].iter().any(|e| e.item().is_some_and(|x| x.0 == id))).count() };
        assert!(on("P-101A") >= 2 && on("V-101") >= 3 && on("HV-104") == 2, "P-101A {} V-101 {} HV-104 {}", on("P-101A"), on("V-101"), on("HV-104"));
        let ends: Vec<&End> = d.p.lines.iter().filter(|l| l.sheet == sid).flat_map(|l| [&l.a, &l.b]).collect();
        let free = ends.iter().filter(|e| matches!(e, End::Free { .. })).count();
        assert!(free * 5 <= ends.len(), "{free} of {} ends open", ends.len());
        // Line numbers read from the text, shared by the pieces of one line.
        let numbers: BTreeSet<String> = d.p.lines.iter().filter(|l| l.sheet == sid && l.kind.process()).map(|l| crate::graph::obj_number(&d.p, l.obj)).collect();
        assert!(numbers.contains("3\"-P-1003-A1A") && numbers.contains("4\"-P-1001-A1A"), "{numbers:?}");
        let net = crate::graph::Net::new(&d.p);
        let hyd = crate::hydraulics::all(&d.p, &net);
        let dup = crate::checks::run(&d.p, &net, &hyd).into_iter().filter(|i| i.rule == 1).count();
        assert_eq!(dup, 0, "no duplicate tags or line numbers");
        // Undo takes the whole conversion back.
        d.undo();
        assert_eq!(d.p.sheets.len(), 1);
    }

    #[test]
    fn blocks_left_out_and_hostile_drawings() {
        let src = crate::examples::get("pump").unwrap();
        let text = crate::dxf::write(&src, &src.sheets[0]);
        let mut map = suggested(&scan(&text).unwrap());
        map.insert("SYM_GATE".into(), String::new());
        let mut p = Project::new();
        let r = convert(&mut p, &text, "x.dxf", &map, &mut vec![]).unwrap();
        assert!(r.left_out.iter().any(|(b, n)| b == "SYM_GATE" && *n >= 5), "{:?}", r.left_out);
        assert!(r.notes.iter().any(|n| n.starts_with("Left out:")));
        // Converted into a project that has those tags already: said, not silently dropped.
        let mut q = src.clone();
        let r2 = convert(&mut q, &text, "again.dxf", &suggested(&scan(&text).unwrap()), &mut vec![]).unwrap();
        assert_eq!(r2.tagged, 0);
        assert!(r2.notes.iter().any(|n| n.contains("tags are already used in the project")), "{:?}", r2.notes);
        // A drawing with no blocks: said so; nonsense: refused, never a panic.
        let s = scan(crate::dxfin::tests::SAMPLE).unwrap();
        assert_eq!(s.blocks.iter().find(|b| b.block == "VLV-GATE").map(|b| b.sym.as_str()), Some("gate"), "a client's block name");
        for bad in ["", "0\nEOF\n", "garbage", "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\nnan\n20\n1e308\n11\n-1e308\n21\n0\n0\nENDSEC\n0\nEOF\n"] {
            let _ = scan(bad);
            let _ = convert(&mut Project::new(), bad, "bad.dxf", &BTreeMap::new(), &mut vec![]);
        }
        assert!(tag_like("P-101A") && tag_like("FIC 101") && !tag_like("3\"-P-1001-A1A") && !tag_like("PUMP"));
        assert!(number_like("3\"-P-1003-A1A") && number_like("80-P-1003-A1A") && !number_like("P-101A"));
    }
}
