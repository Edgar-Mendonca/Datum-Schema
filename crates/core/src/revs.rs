//! Sheet revisions: the sheet as issued at each revision, what has changed since, and revision
//! clouds round the changes (with the revision's triangle), drawn, printed and exported.

use crate::model::{Item, Line, Note, Project, Sheet, SnapBody};
use crate::symbols::{self, Prim};
use serde::Serialize;
use std::collections::BTreeMap;

/// The sheet as it is now, to keep as an issued revision.
pub fn snapshot(p: &Project, sheet: u32) -> SnapBody {
    let items: Vec<Item> = p.items.iter().filter(|i| i.sheet == sheet).cloned().collect();
    let lines: Vec<Line> = p.lines.iter().filter(|l| l.sheet == sheet).cloned().collect();
    let notes: Vec<Note> = p.notes.iter().filter(|n| n.sheet == sheet).cloned().collect();
    let routes = crate::view::routes(p, sheet, None).into_iter().collect();
    let mut objs = BTreeMap::new();
    for o in items.iter().map(|i| i.obj).chain(lines.iter().map(|l| l.obj)).filter(|o| *o != 0) {
        if let Some(ob) = p.obj(o) {
            objs.insert(o, (ob.tag.clone(), ob.data.clone()));
        }
    }
    SnapBody { items, lines, notes, routes, objs }
}

/// One difference between the sheet now and as issued.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Change {
    /// "added", "removed" or "changed".
    pub kind: &'static str,
    pub what: String,
    /// What it is about now (empty for something removed).
    pub ids: Vec<u32>,
    pub bbox: [f64; 4],
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Compare {
    pub rev: String,
    /// The revision the changes belong to (the one after `rev`).
    pub next: String,
    pub changes: Vec<Change>,
    /// The clouds' areas: the changes gathered into rectangles.
    pub areas: Vec<[f64; 4]>,
}

/// The revision after another: A → B, Z → AA, 0 → 1, P1 → P2.
pub fn next_rev(r: &str) -> String {
    let r = r.trim();
    let split = r.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if split < r.len() {
        let n: u64 = r[split..].parse().unwrap_or(0);
        return format!("{}{}", &r[..split], n + 1);
    }
    let mut cs: Vec<char> = r.chars().collect();
    for i in (0..cs.len()).rev() {
        match cs[i] {
            'Z' => cs[i] = 'A',
            'z' => cs[i] = 'a',
            c if c.is_ascii_alphabetic() => {
                cs[i] = (c as u8 + 1) as char;
                return cs.into_iter().collect();
            }
            _ => return format!("{r}1"),
        }
    }
    if r.is_empty() { "A".into() } else { format!("A{}", cs.into_iter().collect::<String>()) }
}

fn grow(b: [f64; 4], q: [f64; 4]) -> [f64; 4] {
    [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[2]), b[3].max(q[3])]
}
fn pts_box(ps: &[[f64; 2]]) -> Option<[f64; 4]> {
    ps.iter().map(|p| [p[0], p[1], p[0], p[1]]).reduce(grow)
}
fn same_pts(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x[0] - y[0]).abs() < 0.01 && (x[1] - y[1]).abs() < 0.01)
}

fn item_name(tag: &str, sym: &str) -> String {
    let n = symbols::get(sym).map_or("symbol", |s| s.name).to_lowercase();
    if tag.trim().is_empty() { format!("a {n}") } else { format!("{tag} ({n})") }
}

/// What has changed on a sheet since it was issued at `rev`.
pub fn compare(p: &Project, s: &Sheet, rev: &str) -> Result<Compare, String> {
    let snap = s.snaps.iter().find(|x| x.rev == rev).ok_or_else(|| format!("Sheet {} was not issued at revision {rev}.", s.number))?;
    let old = snap.body.as_ref().ok_or("That revision's copy is missing.")?;
    let now = snapshot(p, s.id);
    // The revision the changes belong to: the next one issued after `rev`, else the one after it.
    let k = s.revs.iter().position(|r| r.rev == rev);
    let next = k.and_then(|k| s.revs.get(k + 1)).map(|r| r.rev.clone()).unwrap_or_else(|| next_rev(rev));
    let mut v: Vec<Change> = vec![];
    let obj_same = |a: u32, b: u32| old.objs.get(&a) == now.objs.get(&b);
    let tag_of = |m: &BTreeMap<u32, (String, crate::model::Data)>, o: u32| m.get(&o).map(|x| x.0.clone()).unwrap_or_default();
    let old_items: BTreeMap<u32, &Item> = old.items.iter().map(|i| (i.id, i)).collect();
    let now_items: BTreeMap<u32, &Item> = now.items.iter().map(|i| (i.id, i)).collect();
    for (id, it) in &now_items {
        let bb = crate::view::bbox(it);
        match old_items.get(id) {
            None => v.push(Change { kind: "added", what: format!("Added {}", item_name(&tag_of(&now.objs, it.obj), &it.sym)), ids: vec![*id], bbox: bb }),
            Some(o) => {
                let moved = (o.x - it.x).abs() > 0.01 || (o.y - it.y).abs() > 0.01 || o.rot != it.rot || o.flip != it.flip || (o.scale - it.scale).abs() > 1e-6;
                let swapped = o.sym != it.sym && o.base_key() != it.base_key();
                let data = !obj_same(o.obj, it.obj);
                if moved || swapped || data {
                    let mut why = vec![];
                    if swapped {
                        why.push("symbol");
                    }
                    if moved {
                        why.push("moved");
                    }
                    if data {
                        why.push(if tag_of(&old.objs, o.obj) != tag_of(&now.objs, it.obj) { "tag" } else { "data" });
                    }
                    v.push(Change { kind: "changed", what: format!("{}: {}", item_name(&tag_of(&now.objs, it.obj), &it.sym), why.join(", ")), ids: vec![*id], bbox: grow(bb, crate::view::bbox(o)) });
                }
            }
        }
    }
    for (id, it) in &old_items {
        if !now_items.contains_key(id) {
            v.push(Change { kind: "removed", what: format!("Removed {}", item_name(&tag_of(&old.objs, it.obj), &it.sym)), ids: vec![], bbox: crate::view::bbox(it) });
        }
    }
    let number = |m: &BTreeMap<u32, (String, crate::model::Data)>, o: u32| -> String {
        let d = m.get(&o).map(|x| &x.1);
        let get = |k: &str| d.and_then(|d| d.get(k)).map(|v| v.text()).unwrap_or_default();
        let n = get("number");
        if !n.is_empty() { n } else { [get("size"), get("fluid"), get("seq")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join("-") }
    };
    let line_name = |m: &BTreeMap<u32, (String, crate::model::Data)>, l: &Line| {
        let n = number(m, l.obj);
        if n.is_empty() { "a line".to_string() } else { format!("line {n}") }
    };
    let old_lines: BTreeMap<u32, &Line> = old.lines.iter().map(|l| (l.id, l)).collect();
    for l in &now.lines {
        let route = now.routes.get(&l.id).cloned().unwrap_or_default();
        let Some(bb) = pts_box(&route) else { continue };
        match old_lines.get(&l.id) {
            None => v.push(Change { kind: "added", what: format!("Added {}", line_name(&now.objs, l)), ids: vec![l.id], bbox: bb }),
            Some(o) => {
                let was = old.routes.get(&l.id).cloned().unwrap_or_default();
                let rerouted = !same_pts(&was, &route);
                let data = o.kind != l.kind || !obj_same(o.obj, l.obj);
                if rerouted || data {
                    let why = match (rerouted, data) { (true, true) => "rerouted, data", (true, false) => "rerouted", _ => "data" };
                    // A data change clouds the line where it is labelled, not its whole length.
                    let area = if rerouted { pts_box(&was).map_or(bb, |w| grow(bb, w)) } else { label_box(&route).unwrap_or(bb) };
                    v.push(Change { kind: "changed", what: format!("{}: {why}", cap(&line_name(&now.objs, l))), ids: vec![l.id], bbox: area });
                }
            }
        }
    }
    let now_ids: std::collections::HashSet<u32> = now.lines.iter().map(|l| l.id).collect();
    for l in old.lines.iter().filter(|l| !now_ids.contains(&l.id)) {
        if let Some(bb) = old.routes.get(&l.id).and_then(|r| pts_box(r)) {
            v.push(Change { kind: "removed", what: format!("Removed {}", line_name(&old.objs, l)), ids: vec![], bbox: bb });
        }
    }
    let note_box = |n: &Note| [n.x, n.y, n.x + n.w.max(30.0), n.y + n.h.max(8.0)];
    let old_notes: BTreeMap<u32, &Note> = old.notes.iter().map(|n| (n.id, n)).collect();
    for n in &now.notes {
        match old_notes.get(&n.id) {
            None => v.push(Change { kind: "added", what: "Added a note".into(), ids: vec![n.id], bbox: note_box(n) }),
            Some(o) if *o != n => v.push(Change { kind: "changed", what: "Changed a note".into(), ids: vec![n.id], bbox: grow(note_box(n), note_box(o)) }),
            _ => {}
        }
    }
    for n in old.notes.iter().filter(|n| !now.notes.iter().any(|x| x.id == n.id)) {
        v.push(Change { kind: "removed", what: "Removed a note".into(), ids: vec![], bbox: note_box(n) });
    }
    let areas = gather(v.iter().map(|c| c.bbox).collect());
    Ok(Compare { rev: rev.to_string(), next, changes: v, areas })
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

/// Where a line's number sits: the middle of its longest segment, as a small box.
fn label_box(route: &[[f64; 2]]) -> Option<[f64; 4]> {
    let w = route.windows(2).max_by(|a, b| {
        let l = |w: &[[f64; 2]]| (w[1][0] - w[0][0]).abs() + (w[1][1] - w[0][1]).abs();
        l(a).total_cmp(&l(b))
    })?;
    let c = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
    Some([c[0] - 8.0, c[1] - 4.0, c[0] + 8.0, c[1] + 4.0])
}

/// Boxes grown by a margin and merged while any two overlap.
fn gather(mut bs: Vec<[f64; 4]>) -> Vec<[f64; 4]> {
    const M: f64 = 3.0;
    for b in &mut bs {
        *b = [b[0] - M, b[1] - M, b[2] + M, b[3] + M];
    }
    let mut merged = true;
    while merged {
        merged = false;
        'outer: for i in 0..bs.len() {
            for j in i + 1..bs.len() {
                let (a, b) = (bs[i], bs[j]);
                if a[0] <= b[2] + M && b[0] <= a[2] + M && a[1] <= b[3] + M && b[1] <= a[3] + M {
                    bs[i] = grow(a, b);
                    bs.remove(j);
                    merged = true;
                    break 'outer;
                }
            }
        }
    }
    bs
}

/// A revision cloud round an area, with the revision in a triangle at its top right corner.
pub fn cloud(b: [f64; 4], rev: &str) -> Vec<Prim> {
    let mut v = vec![];
    let r = 2.5;
    let mut edge = |a: [f64; 2], e: [f64; 2], start: f64| {
        let len = (e[0] - a[0]).hypot(e[1] - a[1]);
        let k = (len / (2.0 * r)).round().max(1.0) as usize;
        let step = [(e[0] - a[0]) / k as f64, (e[1] - a[1]) / k as f64];
        let rr = step[0].hypot(step[1]) / 2.0;
        for i in 0..k {
            let c = [a[0] + step[0] * (i as f64 + 0.5), a[1] + step[1] * (i as f64 + 0.5)];
            v.push(Prim::Arc { c, r: rr, a0: start, a1: start + 180.0, w: 0.35 });
        }
    };
    let [x0, y0, x1, y1] = b;
    edge([x0, y0], [x1, y0], 180.0);
    edge([x1, y0], [x1, y1], 270.0);
    edge([x1, y1], [x0, y1], 0.0);
    edge([x0, y1], [x0, y0], 90.0);
    // The triangle, standing on the cloud's top right corner.
    let (cx, base, side) = (x1, y0 - r - 1.0, 7.0);
    let h = side * 3f64.sqrt() / 2.0;
    v.push(Prim::Poly { pts: vec![[cx - side / 2.0, base], [cx + side / 2.0, base], [cx, base - h]], closed: true, fill: false, dash: false, w: 0.35 });
    v.push(Prim::Text { p: [cx, base - h * 0.36], h: if rev.chars().count() > 1 { 2.0 } else { 2.6 }, s: crate::clip(rev, 3), anchor: 1, bold: true });
    v
}

/// The clouds a sheet shows: round its changes since the revision it clouds against.
pub fn clouds(p: &Project, s: &Sheet) -> Vec<Prim> {
    if s.clouds.is_empty() {
        return vec![];
    }
    match compare(p, s, &s.clouds) {
        Ok(c) => c.areas.iter().flat_map(|b| cloud(*b, &c.next)).collect(),
        Err(_) => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::{Rev, Val};

    #[test]
    fn revisions_follow_on() {
        assert_eq!(next_rev("A"), "B");
        assert_eq!(next_rev("Z"), "AA");
        assert_eq!(next_rev("0"), "1");
        assert_eq!(next_rev("P9"), "P10");
        assert_eq!(next_rev(""), "A");
    }

    #[test]
    fn issued_changes_are_listed_and_clouded() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let s = d.p.sheets[0].id;
        let rev = |r: &str| Rev { rev: r.into(), date: "30-Oct-26".into(), desc: format!("Issued as {r}"), by: "EM".into(), ..Rev::default() };
        d.apply(Op::IssueRevision { sheet: s, rev: rev("B") }).unwrap();
        assert_eq!(d.p.sheets[0].clone().rev, "B");
        assert!(compare(&d.p, &d.p.sheets[0].clone(), "B").unwrap().changes.is_empty());
        // Move the receiver, change a pump's data, delete a valve, add a note.
        let v = d.p.items.iter().find(|i| d.p.tag_of(i) == "V-101").unwrap().id;
        let pump = d.p.items.iter().find(|i| d.p.tag_of(i) == "P-101B").unwrap().id;
        let gate = d.p.items.iter().find(|i| d.p.tag_of(i) == "HV-105").map(|i| i.id);
        d.apply(Op::Move { ids: vec![v], dx: 10.0, dy: 0.0 }).unwrap();
        d.apply(Op::SetData { ids: vec![pump], key: "head".into(), val: Some(Val::Num(50.0)) }).unwrap();
        if let Some(g) = gate {
            d.apply(Op::Delete { ids: vec![g], keep: false }).unwrap();
        }
        let c = compare(&d.p, &d.p.sheets[0].clone(), "B").unwrap();
        let has = |k: &str, t: &str| c.changes.iter().any(|x| x.kind == k && x.what.contains(t));
        assert!(has("changed", "V-101"), "{:#?}", c.changes);
        assert!(has("changed", "P-101B"), "{:#?}", c.changes);
        if gate.is_some() {
            assert!(has("removed", "HV-105"), "{:#?}", c.changes);
        }
        // Lines joined to the moved vessel are rerouted.
        assert!(c.changes.iter().any(|x| x.what.contains("rerouted")), "{:#?}", c.changes);
        assert_eq!(c.next, "C");
        assert!(!c.areas.is_empty() && c.areas.len() <= c.changes.len());
        // Clouds: off until asked for, then drawn with the triangle C; printed in the DXF.
        assert!(clouds(&d.p, &d.p.sheets[0].clone()).is_empty());
        let mut x = d.p.sheets[0].clone();
        x.clouds = "B".into();
        d.apply(Op::SetSheet { sheet: x }).unwrap();
        let cl = clouds(&d.p, &d.p.sheets[0].clone());
        assert!(cl.iter().any(|p| matches!(p, Prim::Text { s, .. } if s == "C")));
        assert!(crate::dxf::write(&d.p, &d.p.sheets[0].clone()).contains("REVISION\r\n"));
        // The window's copy of the sheet (no snapshot contents) keeps the issued copy.
        let from_window: Sheet = serde_json::from_str(&serde_json::to_string(&d.p.sheets[0].clone()).unwrap()).unwrap();
        assert!(from_window.snaps[0].body.is_none());
        d.apply(Op::SetSheet { sheet: from_window }).unwrap();
        assert!(d.p.sheets[0].clone().snaps[0].body.is_some() && !clouds(&d.p, &d.p.sheets[0].clone()).is_empty());
        // Saved and opened again: the same comparison.
        let path = std::env::temp_dir().join(format!("dschema-revs-{}.dschema", std::process::id()));
        crate::file::write(&path, &d.p, &[]).unwrap();
        let (q, _, warn) = crate::file::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(warn.is_empty(), "{warn:?}");
        assert_eq!(compare(&q, &q.sheets[0], "B").unwrap(), c);
        // Issued again: nothing changed since C.
        d.apply(Op::IssueRevision { sheet: s, rev: rev("C") }).unwrap();
        assert!(compare(&d.p, &d.p.sheets[0].clone(), "C").unwrap().changes.is_empty());
        assert_eq!(compare(&d.p, &d.p.sheets[0].clone(), "B").unwrap().next, "C");
        assert!(d.apply(Op::IssueRevision { sheet: s, rev: rev(" ") }).is_err());
        assert!(compare(&d.p, &d.p.sheets[0].clone(), "X").is_err());
    }
}
