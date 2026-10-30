//! Process flow diagrams: streams, the stream table drawn on the sheet, and the heat and mass
//! balance checks. A PFD is a sheet marked `pfd`; its process lines are streams (objects of the
//! Stream class) carrying mass flow, temperature, pressure, vapour fraction, density and enthalpy
//! flow. Balances are a check on the numbers the engineer typed, not a simulation.

use crate::checks::Issue;
use crate::graph::Net;
use crate::model::{Data, Item, Obj, Project, Sheet, LINE_CLASS, STREAM_CLASS};
use crate::symbols::{self, Prim};
use crate::units;
use std::collections::BTreeMap;

/// The stream table's rows: (label, data key, quantity).
pub const ROWS: [(&str, &str, &str); 10] = [
    ("Stream", "number", ""),
    ("Description", "name", ""),
    ("Phase", "phase", ""),
    ("Mass flow", "mflow", "mflow"),
    ("Volume flow", "flow", "flow"),
    ("Temperature", "temp", "temp"),
    ("Pressure", "press", "press"),
    ("Vapour fraction", "vfrac", ""),
    ("Density", "density", "density"),
    ("Enthalpy flow", "hflow", "power"),
];

fn num(d: &Data, k: &str) -> Option<f64> {
    d.get(k).and_then(|v| v.num()).filter(|x| x.is_finite())
}

/// A stream's number as a sort key: numbers in order, then the rest by text.
fn order(o: &Obj) -> (u64, String) {
    let n = o.data.get("number").map(|v| v.text().trim().to_string()).unwrap_or_default();
    (n.parse().unwrap_or(u64::MAX), n)
}

/// The streams drawn on a sheet, in number order.
pub fn streams_on(p: &Project, sheet: u32) -> Vec<&Obj> {
    let mut ids: Vec<u32> = p.lines.iter().filter(|l| l.sheet == sheet && l.kind.process()).map(|l| l.obj).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut v: Vec<&Obj> = ids.into_iter().filter_map(|o| p.obj(o)).filter(|o| o.class == STREAM_CLASS).collect();
    v.sort_by_key(|o| order(o));
    v
}

/// A table cell: the value in display units.
fn cell(d: &Data, k: &str, q: &str, us: bool) -> String {
    match d.get(k) {
        None => String::new(),
        Some(v) if !q.is_empty() => v.num().map(|x| crate::fmt_sig(units::to_display(x, q, us), 4)).unwrap_or_else(|| v.text()),
        Some(v) if k == "vfrac" || k.starts_with("x:") => v.num().map(|x| crate::fmt_sig(x, 3)).unwrap_or_else(|| v.text()),
        Some(v) => v.text(),
    }
}

fn label(k: &str, q: &str, us: bool) -> String {
    let u = if q.is_empty() { String::new() } else { units::shown(us).into_iter().find(|s| s.q == q).map(|s| s.label).unwrap_or_default() };
    if u.is_empty() { k.to_string() } else { format!("{k}, {}", units::pretty(&u)) }
}

const ROW_H: f64 = 4.2;
const LABEL_W: f64 = 34.0;
const COL_W: f64 = 19.0;

fn text(p: [f64; 2], h: f64, s: impl Into<String>, anchor: u8, bold: bool) -> Prim {
    Prim::Text { p, h, s: s.into(), anchor, bold }
}
fn poly(pts: Vec<[f64; 2]>, closed: bool, w: f64) -> Prim {
    Prim::Poly { pts, closed, fill: false, dash: false, w }
}

/// The stream table of a PFD sheet: along the bottom of the drawing area, left of the title block,
/// in as many bands as its streams need (the next band above the last).
pub fn table(p: &Project, s: &Sheet) -> Vec<Prim> {
    if !s.pfd {
        return vec![];
    }
    let streams = streams_on(p, s.id);
    if streams.is_empty() {
        return vec![];
    }
    let us = p.units == "US";
    let (x0, _, _, y1) = crate::frame::inner(s);
    let (rx, _) = crate::frame::reserved(p, s);
    let x0 = x0 + 3.0;
    let per = (((rx - 5.0 - x0 - LABEL_W) / COL_W).floor() as usize).max(1);
    // The fixed rows, then a mass fraction for each component.
    let rows: Vec<(String, String, &str)> = ROWS.iter().map(|(l, k, q)| (label(l, q, us), k.to_string(), *q)).chain(p.settings.components.iter().map(|c| (format!("  {c}"), format!("x:{c}"), ""))).collect();
    let h = ROW_H * rows.len() as f64;
    let mut v = vec![];
    for (band, chunk) in streams.chunks(per).enumerate() {
        let bottom = y1 - 3.0 - band as f64 * (h + 4.0);
        let top = bottom - h;
        let right = x0 + LABEL_W + COL_W * chunk.len() as f64;
        v.push(poly(vec![[x0, top], [right, top], [right, bottom], [x0, bottom]], true, 0.35));
        for r in 1..rows.len() {
            let y = top + ROW_H * r as f64;
            v.push(poly(vec![[x0, y], [right, y]], false, if r == 1 { 0.3 } else { 0.13 }));
        }
        for c in 0..=chunk.len() {
            let x = x0 + LABEL_W + COL_W * c as f64;
            v.push(poly(vec![[x, top], [x, bottom]], false, 0.18));
        }
        for (r, (l, k, q)) in rows.iter().enumerate() {
            let y = top + ROW_H * (r as f64 + 0.5);
            if r == ROWS.len() && r > 0 {
                v.push(poly(vec![[x0, top + ROW_H * r as f64], [right, top + ROW_H * r as f64]], false, 0.3));
            }
            v.push(text([x0 + 1.2, y], 2.0, crate::clip(l, 22), 0, r == 0));
            for (c, o) in chunk.iter().enumerate() {
                let x = x0 + LABEL_W + COL_W * (c as f64 + 0.5);
                let t = crate::clip(&cell(&o.data, k, q, us), 13);
                v.push(text([x, y], 2.0, t, 1, r == 0));
            }
        }
    }
    v
}

/// The streams at an item's process ports: (stream object, flowing in).
fn at_item<'a>(p: &'a Project, net: &Net, it: &Item) -> Vec<(&'a Obj, bool)> {
    let Some(s) = symbols::get(&it.sym) else { return vec![] };
    let mut v: Vec<(&Obj, bool)> = vec![];
    for (k, port) in s.ports.iter().enumerate() {
        if port.signal {
            continue;
        }
        for &(l, at_b) in net.on_port(it.id, k as u8) {
            let Some(line) = net.lines.get(&l) else { continue };
            let Some(o) = p.obj(line.obj).filter(|o| o.class == STREAM_CLASS) else { continue };
            let into = at_b != line.flip;
            if !v.iter().any(|(x, i)| x.id == o.id && *i == into) {
                v.push((o, into));
            }
        }
    }
    // A stream through an in-line item (in on one side, out on the other) balances itself.
    let through: Vec<u32> = v.iter().filter(|(o, i)| *i && v.iter().any(|(x, j)| x.id == o.id && !*j)).map(|(o, _)| o.id).collect();
    v.retain(|(o, _)| !through.contains(&o.id));
    v
}

fn names(v: &[(&Obj, bool)]) -> String {
    let mut n: Vec<String> = v.iter().map(|(o, _)| o.data.get("number").map(|x| x.text()).unwrap_or_default()).filter(|s| !s.is_empty()).collect();
    n.sort_by_key(|s| (s.parse::<u64>().unwrap_or(u64::MAX), s.clone()));
    n.join(", ")
}

/// Rules 14 (mass) and 15 (heat): every item on a PFD with streams in and out.
pub fn balances(p: &Project, net: &Net) -> Vec<Issue> {
    let us = p.units == "US";
    let pfd: Vec<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    let mut v = vec![];
    let mut seen = std::collections::HashSet::new();
    for it in p.items.iter().filter(|i| pfd.contains(&i.sheet)) {
        // One balance per object (an item drawn twice is one piece of equipment).
        if it.obj != 0 && !seen.insert(it.obj) {
            continue;
        }
        let st = at_item(p, net, it);
        if !st.iter().any(|x| x.1) || !st.iter().any(|x| !x.1) {
            continue;
        }
        let who = {
            let t = p.tag_of(it);
            if t.trim().is_empty() { symbols::get(&it.sym).map_or("an item".into(), |s| s.name.to_lowercase()) } else { t.to_string() }
        };
        let issue = |rule: u8, level: &'static str, msg: String| Issue { rule, level, msg, ids: vec![it.id], sheet: it.sheet, at: [it.x, it.y] };
        // 14. Mass.
        let missing: Vec<(&Obj, bool)> = st.iter().filter(|(o, _)| num(&o.data, "mflow").is_none()).copied().collect();
        if !missing.is_empty() {
            if missing.len() < st.len() {
                v.push(issue(14, "note", format!("The mass balance at {who} cannot be checked: stream {} has no mass flow.", names(&missing))));
            }
        } else {
            let sum = |into: bool| st.iter().filter(|x| x.1 == into).filter_map(|(o, _)| num(&o.data, "mflow")).sum::<f64>();
            let (i, o) = (sum(true), sum(false));
            if (i - o).abs() > 0.005 * i.abs().max(o.abs()) + 1e-9 {
                let pc = if i.abs() > 0.0 { (i - o).abs() / i.abs() * 100.0 } else { 100.0 };
                v.push(issue(14, "error", format!("Mass does not balance at {who}: {} in, {} out ({}% {}).", units::show(i, "mflow", us), units::show(o, "mflow", us), crate::fmt_sig(pc, 2), if o < i { "lost" } else { "gained" })));
            }
        }
        // 15. Heat: out − in is the duty (or shaft power) put in, or nothing.
        if st.iter().all(|(o, _)| num(&o.data, "hflow").is_some()) {
            let sum = |into: bool| st.iter().filter(|x| x.1 == into).filter_map(|(o, _)| num(&o.data, "hflow")).sum::<f64>();
            let (i, o) = (sum(true), sum(false));
            let d = p.obj(it.obj).and_then(|ob| num(&ob.data, "duty").or_else(|| num(&ob.data, "power"))).unwrap_or(0.0);
            let gap = o - i;
            let tol = 0.02 * i.abs().max(o.abs()).max(d.abs()) + 1.0;
            // Balanced as drawn (both sides of an exchanger), or by the item's duty.
            if gap.abs() > tol && (gap.abs() - d.abs()).abs() > tol {
                let duty = if d != 0.0 { format!(", against a duty of {}", units::show(d.abs(), "power", us)) } else { String::new() };
                v.push(issue(15, "warning", format!("Heat does not balance at {who}: the streams take {} {}{duty}.", units::show(gap.abs(), "power", us), if gap >= 0.0 { "more out than in" } else { "less out than in" })));
            }
        }
        // 17. Each component, when every stream round the item has a composition (a component
        // left blank is none of it). Reactors change their components and are left out.
        let comps = &p.settings.components;
        let reactor = it.base_key().contains("reactor");
        let has = |o: &Obj| comps.iter().any(|c| num(&o.data, &format!("x:{c}")).is_some());
        if !comps.is_empty() && !reactor && st.iter().all(|(o, _)| has(o) && num(&o.data, "mflow").is_some()) {
            let total: f64 = st.iter().filter(|x| x.1).filter_map(|(o, _)| num(&o.data, "mflow")).sum();
            for c in comps {
                let k = format!("x:{c}");
                let sum = |into: bool| st.iter().filter(|x| x.1 == into).map(|(o, _)| num(&o.data, "mflow").unwrap_or(0.0) * num(&o.data, &k).unwrap_or(0.0)).sum::<f64>();
                let (i, o) = (sum(true), sum(false));
                if (i - o).abs() > 0.005 * total.abs() + 1e-9 {
                    v.push(issue(17, "warning", format!("{c} does not balance at {who}: {} in, {} out.", units::show(i, "mflow", us), units::show(o, "mflow", us))));
                }
            }
        }
    }
    v
}

/// Rule 17 too: a stream's mass fractions add up to 1 (within 1%).
pub fn compositions(p: &Project, net: &Net) -> Vec<Issue> {
    let comps = &p.settings.components;
    let mut v = vec![];
    if comps.is_empty() {
        return v;
    }
    for (r, lines) in net.runs.iter().enumerate() {
        let Some(o) = p.obj(net.run_obj[r]).filter(|o| o.class == STREAM_CLASS) else { continue };
        let xs: Vec<f64> = comps.iter().filter_map(|c| num(&o.data, &format!("x:{c}"))).collect();
        if xs.is_empty() {
            continue;
        }
        let sum: f64 = xs.iter().sum();
        if (sum - 1.0).abs() > 0.01 {
            let l = net.lines[&lines[0]];
            let n = o.data.get("number").map(|x| x.text()).unwrap_or_default();
            v.push(Issue { rule: 17, level: "warning", msg: format!("The composition of stream {n} adds up to {}, not 1.", crate::fmt_sig(sum, 4)), ids: lines.clone(), sheet: l.sheet, at: crate::graph::end_pt(p, &l.a) });
        }
    }
    v
}

/// The streams of the project by number.
fn by_number(p: &Project) -> BTreeMap<String, &Obj> {
    let mut m = BTreeMap::new();
    for o in p.objects.iter().filter(|o| o.class == STREAM_CLASS) {
        let n = o.data.get("number").map(|v| v.text().trim().to_string()).unwrap_or_default();
        if !n.is_empty() {
            m.entry(n).or_insert(o);
        }
    }
    m
}

/// Rule 16: a P&ID line that names a stream carries the stream's mass flow.
pub fn line_vs_stream(p: &Project, net: &Net) -> Vec<Issue> {
    let us = p.units == "US";
    let streams = by_number(p);
    let mut v = vec![];
    for (r, lines) in net.runs.iter().enumerate() {
        let Some(o) = p.obj(net.run_obj[r]).filter(|o| o.class == LINE_CLASS) else { continue };
        let want = o.data.get("stream").map(|x| x.text().trim().to_string()).unwrap_or_default();
        if want.is_empty() {
            continue;
        }
        let l = net.lines[&lines[0]];
        let n = net.numbers.get(&l.id).cloned().filter(|n| !n.is_empty()).map_or("A line".to_string(), |n| format!("Line {n}"));
        let issue = |level: &'static str, msg: String| Issue { rule: 16, level, msg, ids: lines.clone(), sheet: l.sheet, at: crate::graph::end_pt(p, &l.a) };
        let Some(s) = streams.get(&want) else {
            v.push(issue("warning", format!("{n} names stream {want}, which is on no process flow diagram.")));
            continue;
        };
        let mine = num(&o.data, "mflow").or_else(|| num(&o.data, "flow").zip(num(&o.data, "density")).map(|(q, rho)| q * rho));
        if let (Some(a), Some(b)) = (mine, num(&s.data, "mflow")) {
            if (a - b).abs() > 0.02 * a.abs().max(b.abs()) + 1e-9 {
                v.push(issue("warning", format!("{n} carries {}, but stream {want} on the PFD carries {}.", units::show(a, "mflow", us), units::show(b, "mflow", us))));
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::{End, Kind, Line, Val};

    /// A PFD sheet with a drum V-1 on it.
    fn pfd() -> (Doc, u32, u32) {
        let mut d = Doc::new(Project::new());
        let s = d.apply(Op::AddSheet { sheet: Sheet { pfd: true, ..Sheet::default() } }).unwrap()[0];
        let v = d.apply(Op::AddItem { item: Item { sheet: s, sym: "vvessel".into(), x: 200.0, y: 100.0, scale: 1.0, ..Item::default() }, on: None, tag: "V-1".into(), obj: None }).unwrap()[0];
        (d, s, v)
    }

    #[test]
    fn streams_are_numbered_and_balanced() {
        let (mut d, s, vid) = pfd();
        let v = d.p.item(vid).unwrap().clone();
        assert_eq!(d.p.tag_of(&v), "V-1");
        // Two lines into the drum's left nozzles and one out of its right: three streams.
        let mut ids = vec![];
        for k in [2u8, 3, 4] {
            let pt = crate::graph::end_pt(&d.p, &End::Port { item: v.id, port: k });
            let far = End::Free { x: pt[0] + if k == 4 { 60.0 } else { -60.0 }, y: pt[1] };
            let (a, b) = if k == 4 { (End::Port { item: v.id, port: k }, far) } else { (far, End::Port { item: v.id, port: k }) };
            ids.push(d.apply(Op::AddLine { line: Line { sheet: s, kind: Kind::Major, a, b, ..Line::default() } }).unwrap()[0]);
        }
        let obj = |d: &Doc, l: u32| d.p.line(l).unwrap().obj;
        let nums: Vec<String> = ids.iter().map(|l| d.p.obj(obj(&d, *l)).unwrap().data["number"].text()).collect();
        assert_eq!(nums, ["1", "2", "3"]);
        assert!(ids.iter().all(|l| d.p.obj(obj(&d, *l)).unwrap().class == STREAM_CLASS));
        let set = |d: &mut Doc, l: u32, k: &str, x: f64| {
            d.apply(Op::SetData { ids: vec![l], key: k.into(), val: Some(Val::Num(x)) }).unwrap();
        };
        let issues = |d: &Doc| crate::api::derived(&d.p, s).issues;
        set(&mut d, ids[0], "mflow", 2.0);
        // One stream known, the others not: a note.
        assert!(issues(&d).iter().any(|i| i.rule == 14 && i.level == "note"), "{:?}", issues(&d));
        set(&mut d, ids[1], "mflow", 1.0);
        set(&mut d, ids[2], "mflow", 2.5);
        let m = issues(&d).into_iter().find(|i| i.rule == 14).expect("mass does not balance");
        assert_eq!(m.level, "error");
        assert!(m.msg.contains("V-1") && m.msg.contains("lost"), "{}", m.msg);
        set(&mut d, ids[2], "mflow", 3.0);
        assert!(!issues(&d).iter().any(|i| i.rule == 14));
        // Heat: a drum with no duty must keep the enthalpy flows.
        set(&mut d, ids[0], "hflow", 100e3);
        set(&mut d, ids[1], "hflow", 50e3);
        set(&mut d, ids[2], "hflow", 200e3);
        assert!(issues(&d).iter().any(|i| i.rule == 15 && i.msg.contains("more out than in")));
        set(&mut d, vid, "duty", 50e3);
        assert!(!issues(&d).iter().any(|i| i.rule == 15));
        // The table: three columns, the numbers in display units.
        let sh = d.p.sheets.iter().find(|x| x.id == s).unwrap().clone();
        let t = table(&d.p, &sh);
        let texts: Vec<String> = t.iter().filter_map(|p| if let Prim::Text { s, .. } = p { Some(s.clone()) } else { None }).collect();
        assert!(texts.iter().any(|x| x == "Mass flow, kg/h"), "{texts:?}");
        assert!(texts.contains(&"7200".to_string()) && texts.contains(&"10800".to_string()), "{texts:?}");
        // P&ID rules leave the PFD alone: no line number, size or check valve wanted.
        assert!(!issues(&d).iter().any(|i| matches!(i.rule, 4 | 5 | 6 | 8 | 9 | 10)), "{:?}", issues(&d));
        // A P&ID line naming stream 3 with a different flow: rule 16.
        let pid = d.p.sheets.iter().find(|x| !x.pfd).unwrap().id;
        let l = d.apply(Op::AddLine { line: Line { sheet: pid, kind: Kind::Major, a: End::Free { x: 50.0, y: 50.0 }, b: End::Free { x: 150.0, y: 50.0 }, ..Line::default() } }).unwrap()[0];
        assert_eq!(d.p.obj(obj(&d, l)).unwrap().class, LINE_CLASS);
        let text = |d: &mut Doc, k: &str, t: &str| d.apply(Op::SetData { ids: vec![l], key: k.into(), val: Some(Val::Text(t.into())) }).unwrap();
        text(&mut d, "stream", "3");
        set(&mut d, l, "mflow", 2.0);
        assert!(issues(&d).iter().any(|i| i.rule == 16 && i.msg.contains("stream 3")));
        set(&mut d, l, "mflow", 3.0);
        assert!(!issues(&d).iter().any(|i| i.rule == 16));
        text(&mut d, "stream", "9");
        assert!(issues(&d).iter().any(|i| i.rule == 16 && i.msg.contains("on no process flow diagram")));
        // Compositions: water and salt into the drum, out as one stream.
        d.p.settings.components = vec!["Water".into(), "Salt".into()];
        let comp = |d: &mut Doc, l: u32, w: f64, s_: f64| {
            d.apply(Op::SetData { ids: vec![l], key: "x:Water".into(), val: Some(Val::Num(w)) }).unwrap();
            d.apply(Op::SetData { ids: vec![l], key: "x:Salt".into(), val: Some(Val::Num(s_)) }).unwrap();
        };
        assert!(crate::classes::attrs(Some(&d.p), STREAM_CLASS).iter().any(|a| a.key == "x:Salt" && a.group.starts_with("Composition")));
        comp(&mut d, ids[0], 1.0, 0.0);
        comp(&mut d, ids[1], 0.7, 0.3);
        // 2 kg/s water + 0.7 + 0.3 → 3 kg/s at 0.9 water, 0.1 salt.
        comp(&mut d, ids[2], 0.85, 0.15);
        let i17 = |d: &Doc| issues(d).into_iter().filter(|i| i.rule == 17).map(|i| i.msg).collect::<Vec<_>>();
        assert!(i17(&d).iter().any(|m| m.contains("Salt does not balance at V-1")), "{:?}", i17(&d));
        comp(&mut d, ids[2], 0.9, 0.1);
        assert!(i17(&d).is_empty(), "{:?}", i17(&d));
        // A composition that does not add up.
        d.apply(Op::SetData { ids: vec![ids[1]], key: "x:Salt".into(), val: Some(Val::Num(0.2)) }).unwrap();
        assert!(i17(&d).iter().any(|m| m.contains("stream 2 adds up to 0.9")), "{:?}", i17(&d));
        // The table has a row per component; the stream list a column.
        let sh = d.p.sheets.iter().find(|x| x.id == s).unwrap().clone();
        assert!(table(&d.p, &sh).iter().any(|p| matches!(p, Prim::Text { s, .. } if s == "  Salt")));
        let ll = crate::lists::build(&d.p, "streams").unwrap();
        assert!(ll.cols.iter().any(|c| c.key == "x:Water"));
        // A stream moved to a P&ID sheet becomes a line number again.
        let mut sh2 = sh.clone();
        sh2.pfd = false;
        d.apply(Op::SetSheet { sheet: sh2 }).unwrap();
        assert_eq!(d.p.obj(obj(&d, ids[0])).unwrap().class, LINE_CLASS);
    }
}
