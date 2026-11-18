//! Pressure test packs: the line numbers tested together, read from the drawing. Line numbers of
//! the same pipe class and test pressure that meet (at a branch, or through a valve, fitting or
//! in-line instrument) are one pack; equipment, relief devices, other classes and other pressures
//! are its limits. A line's test pressure is its own Test P, else 1.5 × its design pressure for a
//! hydrostatic test (1.1 × for a pneumatic one), as the usual piping code practice has it.
//!
//! For each pack the engine lists its lines with their length and fill volume, the test limits
//! (where to blind or close), what is inside and must be opened, removed or isolated (valves,
//! check valves, control valves, relief devices, instruments, strainer elements), its vents and
//! drains, and what stops a test (no vent, no drain, an open end, lines of different pressures).
//! The packs are marked on the sheets, listed, and each prints as its own sheet. A planning aid:
//! the test pressure and procedure are the engineer's and the code's to set.

use crate::graph::Net;
use crate::model::{End, Project, Val, LINE_CLASS};
use crate::symbols::{self, Cat, Prim};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The project's test packs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Packs {
    /// Mark the packs on the sheets (and print them).
    pub show: bool,
    pub packs: Vec<Pack>,
}

/// One test pack.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Pack {
    /// TP-001…
    pub id: String,
    /// The line-number objects in it.
    pub lines: Vec<u32>,
    /// Hydrostatic or Pneumatic.
    pub medium: String,
    /// Planned, Ready, Tested or Accepted.
    pub status: String,
    pub date: String,
    /// Who witnessed it.
    pub witness: String,
    pub note: String,
}

pub const MEDIA: [&str; 2] = ["Hydrostatic", "Pneumatic"];
pub const STATUSES: [&str; 4] = ["Planned", "Ready", "Tested", "Accepted"];

/// Keeps the packs sound: line numbers that exist, each in one pack, ids unique, words known.
pub fn normalize(s: &mut Packs, is_line: &dyn Fn(u32) -> bool) {
    s.packs.truncate(2000);
    let mut used = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for (k, pk) in s.packs.iter_mut().enumerate() {
        pk.id = crate::clip(pk.id.trim(), 40);
        if pk.id.is_empty() || !ids.insert(pk.id.to_uppercase()) {
            let mut n = k + 1;
            while ids.contains(&format!("TP-{n:03}")) {
                n += 1;
            }
            pk.id = format!("TP-{n:03}");
            ids.insert(pk.id.clone());
        }
        pk.lines.retain(|l| is_line(*l) && used.insert(*l));
        if !MEDIA.contains(&pk.medium.as_str()) {
            pk.medium = MEDIA[0].into();
        }
        if !STATUSES.contains(&pk.status.as_str()) {
            pk.status = STATUSES[0].into();
        }
        pk.date = crate::clip(&pk.date, 40);
        pk.witness = crate::clip(&pk.witness, 200);
        pk.note = crate::clip(&pk.note, 2000);
    }
    s.packs.retain(|p| !p.lines.is_empty());
}

fn num(p: &Project, obj: u32, k: &str) -> Option<f64> {
    match p.data_of(obj).get(k) {
        Some(Val::Num(x)) if x.is_finite() && *x > 0.0 => Some(*x),
        _ => None,
    }
}

fn text(p: &Project, obj: u32, k: &str) -> String {
    p.data_of(obj).get(k).map(|v| v.text().trim().to_string()).unwrap_or_default()
}

/// A line number's test pressure (Pa, gauge) and where it comes from.
pub fn test_pressure(p: &Project, obj: u32, medium: &str) -> Option<(f64, &'static str)> {
    if let Some(x) = num(p, obj, "test_p") {
        return Some((x, "its Test P"));
    }
    let d = num(p, obj, "design_p")?;
    Some(if medium == "Pneumatic" { (1.1 * d, "1.1 × design") } else { (1.5 * d, "1.5 × design") })
}

/// The line-number objects drawn on P&IDs.
fn line_objs(p: &Project) -> BTreeSet<u32> {
    let pfd: BTreeSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    p.lines.iter().filter(|l| l.kind.process() && l.obj != 0 && !pfd.contains(&l.sheet) && !crate::graph::instrument_connection(p, l) && p.obj(l.obj).is_some_and(|o| o.class == LINE_CLASS)).map(|l| l.obj).collect()
}

fn base(p: &Project, item: u32) -> (String, Option<&'static symbols::Sym>) {
    p.item(item).map(|i| (i.base_key(), symbols::get(&i.sym))).unwrap_or_default()
}

const RELIEF: [&str; 3] = ["psv", "rupture", "breather"];

/// Makes test packs of the line numbers not yet in one; how many packs, and the line numbers left
/// out for want of a test pressure.
pub fn make(p: &mut Project) -> Result<(usize, Vec<String>), String> {
    let net = Net::new(p);
    let taken: BTreeSet<u32> = p.settings.testpacks.packs.iter().flat_map(|k| k.lines.iter().copied()).collect();
    let all = line_objs(p);
    let mut no_p = vec![];
    let mut free: Vec<u32> = vec![];
    for &o in &all {
        if taken.contains(&o) {
            continue;
        }
        if test_pressure(p, o, "Hydrostatic").is_some() {
            free.push(o);
        } else {
            no_p.push(crate::graph::obj_number(p, o));
        }
    }
    // Joined: line numbers that meet at a branch or across a valve or fitting, same class and pressure.
    let mut parent: HashMap<u32, u32> = free.iter().map(|o| (*o, *o)).collect();
    fn find(par: &mut HashMap<u32, u32>, x: u32) -> u32 {
        let mut r = x;
        while par[&r] != r {
            r = par[&r];
        }
        let mut y = x;
        while par[&y] != r {
            let nx = par[&y];
            par.insert(y, r);
            y = nx;
        }
        r
    }
    let same = |a: u32, b: u32| {
        let pa = test_pressure(p, a, "Hydrostatic").map(|x| x.0).unwrap_or(0.0);
        let pb = test_pressure(p, b, "Hydrostatic").map(|x| x.0).unwrap_or(0.0);
        text(p, a, "class").eq_ignore_ascii_case(&text(p, b, "class")) && (pa - pb).abs() <= 0.01 * pa.max(pb)
    };
    let mut pairs: Vec<(u32, u32)> = vec![];
    for l in p.lines.iter().filter(|l| parent.contains_key(&l.obj)) {
        for e in [&l.a, &l.b] {
            match *e {
                End::Line { line, .. } => {
                    if let Some(m) = p.line(line) {
                        pairs.push((l.obj, m.obj));
                    }
                }
                End::Port { item, port } => {
                    let (b, s) = base(p, item);
                    let Some(s) = s else { continue };
                    if !s.inline || RELIEF.contains(&b.as_str()) {
                        continue;
                    }
                    for (k, q) in s.ports.iter().enumerate() {
                        if q.signal || k as u8 == port {
                            continue;
                        }
                        for &(ol, _) in net.on_port(item, k as u8) {
                            if let Some(m) = p.line(ol) {
                                pairs.push((l.obj, m.obj));
                            }
                        }
                    }
                }
                End::Free { .. } => {}
            }
        }
    }
    for (a, b) in pairs {
        if a != b && parent.contains_key(&a) && parent.contains_key(&b) && same(a, b) {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            if ra != rb {
                parent.insert(ra.max(rb), ra.min(rb));
            }
        }
    }
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &o in &free {
        let r = find(&mut parent, o);
        groups.entry(r).or_default().push(o);
    }
    if groups.is_empty() {
        return Err(if no_p.is_empty() { "Every line number is already in a test pack.".into() } else { format!("No line number has a test or design pressure yet ({}…): give them one in the line list.", no_p.iter().take(3).cloned().collect::<Vec<_>>().join(", ")) });
    }
    // In the order of their first line number.
    let mut gs: Vec<Vec<u32>> = groups.into_values().collect();
    gs.iter_mut().for_each(|g| g.sort_by_key(|o| crate::graph::obj_number(p, *o)));
    gs.sort_by_key(|g| crate::graph::obj_number(p, g[0]));
    let ids: BTreeSet<String> = p.settings.testpacks.packs.iter().map(|k| k.id.to_uppercase()).collect();
    let mut n = 1;
    let k = gs.len();
    for g in gs {
        while ids.contains(&format!("TP-{n:03}")) {
            n += 1;
        }
        p.settings.testpacks.packs.push(Pack { id: format!("TP-{n:03}"), lines: g, medium: MEDIA[0].into(), status: STATUSES[0].into(), ..Pack::default() });
        n += 1;
    }
    p.settings.testpacks.show = true;
    Ok((k, no_p))
}

/// A line of a pack, as listed.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PackLine {
    pub obj: u32,
    pub number: String,
    pub class: String,
    /// Pa, gauge.
    pub test_p: f64,
    pub source: &'static str,
    /// m and m³ (0 when the length or size is not given).
    pub length: f64,
    pub volume: f64,
}

/// One thing to do on a pack: where, what, and the action.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Step {
    pub item: u32,
    pub tag: String,
    pub what: String,
    pub action: String,
}

/// Everything about a pack, read from the drawing.
#[derive(Clone, Debug, Serialize, PartialEq, Default)]
pub struct Detail {
    pub id: String,
    pub medium: String,
    pub lines: Vec<PackLine>,
    /// The test pressure (the highest of its lines), Pa gauge.
    pub pressure: f64,
    pub volume: f64,
    /// Where the pack ends: blind or close.
    pub limits: Vec<Step>,
    /// Inside the pack: open, remove or isolate.
    pub inside: Vec<Step>,
    pub vents: Vec<String>,
    pub drains: Vec<String>,
    pub problems: Vec<String>,
    /// The drawn pieces (to show on the sheet).
    pub pieces: Vec<u32>,
}

fn name(p: &Project, item: u32) -> String {
    p.item(item).map(|i| p.tag_of(i).to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| base(p, item).1.map_or("a symbol".into(), |s| s.name.to_lowercase()))
}

/// A pack read from the drawing.
pub fn detail(p: &Project, pack: &Pack) -> Detail {
    let net = Net::new(p);
    let mine: BTreeSet<u32> = pack.lines.iter().copied().collect();
    let mut d = Detail { id: pack.id.clone(), medium: pack.medium.clone(), ..Detail::default() };
    let mut pressures = vec![];
    for &o in &pack.lines {
        let (tp, src) = test_pressure(p, o, &pack.medium).unwrap_or((0.0, "none given"));
        if tp == 0.0 {
            d.problems.push(format!("{} has no test or design pressure.", crate::graph::obj_number(p, o)));
        }
        pressures.push(tp);
        let length = num(p, o, "length").unwrap_or(0.0);
        let id = crate::pipes::inside(&text(p, o, "size"), &text(p, o, "sched")).unwrap_or(0.0);
        let volume = std::f64::consts::PI / 4.0 * id * id * length;
        d.lines.push(PackLine { obj: o, number: crate::graph::obj_number(p, o), class: text(p, o, "class"), test_p: tp, source: src, length, volume });
    }
    d.pressure = pressures.iter().copied().fold(0.0, f64::max);
    let lo = pressures.iter().copied().filter(|x| *x > 0.0).fold(f64::MAX, f64::min);
    if lo < f64::MAX && d.pressure > 1.01 * lo {
        d.problems.push("Its lines have different test pressures: split the pack, or test at the lowest and check the others.".into());
    }
    d.volume = d.lines.iter().map(|l| l.volume).sum();
    let mut seen_items = BTreeSet::new();
    // The valves that are vents or drains (named as such, not as valves to open).
    let mut vd: BTreeSet<u32> = BTreeSet::new();
    let step = |item: u32, action: String| Step { item, tag: name(p, item), what: base(p, item).1.map_or(String::new(), |s| s.name.to_string()), action };
    let pieces: Vec<&crate::model::Line> = p.lines.iter().filter(|l| mine.contains(&l.obj)).collect();
    d.pieces = pieces.iter().map(|l| l.id).collect();
    let in_pack = |l: u32| p.line(l).is_some_and(|x| mine.contains(&x.obj));
    for l in &pieces {
        // Branches onto it from lines of other packs or none.
        for &(bl, _) in net.branches.get(&l.id).into_iter().flatten() {
            // An instrument's connection: the instrument is isolated or removed for the test.
            if let Some(bline) = p.line(bl).filter(|x| crate::graph::instrument_connection(p, x)) {
                for e2 in [&bline.a, &bline.b] {
                    if let Some((it, _)) = e2.item() {
                        if seen_items.insert((it, 254)) {
                            d.inside.push(step(it, "Isolate at its root valve or remove it".into()));
                        }
                    }
                }
                continue;
            }
            if !in_pack(bl) && p.line(bl).is_some_and(|x| x.kind.process()) {
                let other = p.line(bl).map(|x| crate::graph::obj_number(p, x.obj)).unwrap_or_default();
                d.limits.push(Step { item: bl, tag: other.clone(), what: "Branch".into(), action: format!("Blind or close the branch to {other}") });
            }
        }
        for e in [&l.a, &l.b] {
            match *e {
                End::Free { .. } => d.problems.push(format!("{} has an open end: blind it, or show where it goes.", crate::graph::obj_number(p, l.obj))),
                End::Line { line, .. } => {
                    if !in_pack(line) {
                        let other = p.line(line).map(|x| crate::graph::obj_number(p, x.obj)).unwrap_or_default();
                        d.limits.push(Step { item: line, tag: other.clone(), what: "Branch".into(), action: format!("Blind or close at the branch onto {other}") });
                    }
                }
                End::Port { item, port } => {
                    if !seen_items.insert((item, port)) {
                        continue;
                    }
                    let (b, s) = base(p, item);
                    let Some(s) = s else { continue };
                    let it = p.item(item).cloned().unwrap_or_default();
                    let pos = text(p, it.obj, "position").to_uppercase();
                    if RELIEF.contains(&b.as_str()) {
                        d.inside.push(step(item, "Remove and blind its nozzle (or gag it), refit after the test".into()));
                        continue;
                    }
                    if b == "vent" {
                        // Behind a valve, the valve names it.
                        let other = if l.a == *e { &l.b } else { &l.a };
                        if !other.item().is_some_and(|x| base(p, x.0).1.is_some_and(|s| s.cat == Cat::Valve)) {
                            d.vents.push(format!("vent on {}", crate::graph::obj_number(p, l.obj)));
                        }
                        continue;
                    }
                    if b == "drain" {
                        // Behind a valve, the valve names it.
                        let other = if l.a == *e { &l.b } else { &l.a };
                        if !other.item().is_some_and(|x| base(p, x.0).1.is_some_and(|s| s.cat == Cat::Valve)) {
                            d.drains.push(format!("drain on {}", crate::graph::obj_number(p, l.obj)));
                        }
                        continue;
                    }
                    if s.cat.equipment() {
                        let nozzle = s.ports.get(port as usize).map_or("", |q| q.name);
                        d.limits.push(step(item, format!("Blind at the {nozzle} nozzle (or test with the equipment if its design allows)")));
                        continue;
                    }
                    if s.cat.instrument() && !s.inline {
                        d.inside.push(step(item, "Isolate at its root valve or remove it".into()));
                        continue;
                    }
                    if s.cat == Cat::Connector {
                        let partner_in = it.link != 0 && net.on_port(it.link, 0).iter().any(|&(pl, _)| in_pack(pl));
                        if !partner_in {
                            d.limits.push(step(item, if it.link != 0 { "Blind where it continues on the other sheet".into() } else { "Blind at the off-page connector (it is not paired)".into() }));
                        }
                        continue;
                    }
                    if s.cat == Cat::Mark {
                        d.limits.push(step(item, "Blind at the battery limit".into()));
                        continue;
                    }
                    if !s.inline {
                        continue;
                    }
                    // An in-line item: inside when every line on its other side is in the pack.
                    let others: Vec<u32> = s.ports.iter().enumerate().filter(|(k, q)| !q.signal && *k as u8 != port).flat_map(|(k, _)| net.on_port(item, k as u8).iter().map(|x| x.0)).collect();
                    let inside = !others.is_empty() && others.iter().all(|x| in_pack(*x));
                    let goes_nowhere = others.iter().all(|x| p.line(*x).is_some_and(|ol| [&ol.a, &ol.b].iter().all(|e2| match e2 {
                        End::Port { item: i2, .. } if *i2 == item => true,
                        End::Port { item: i2, .. } => matches!(base(p, *i2).0.as_str(), "vent" | "drain" | "blind"),
                        End::Free { .. } => true,
                        _ => false,
                    })));
                    if s.cat == Cat::Valve && goes_nowhere && !others.is_empty() {
                        let what = others.iter().filter_map(|x| p.line(*x)).flat_map(|ol| [&ol.a, &ol.b]).find_map(|e2| e2.item().map(|x| base(p, x.0).0).filter(|k| k == "vent" || k == "drain"));
                        let label = format!("{} ({})", name(p, item), if what.as_deref() == Some("vent") { "vent" } else { "drain" });
                        if what.as_deref() == Some("vent") { d.vents.push(label) } else { d.drains.push(label) }
                        vd.insert(item);
                        continue;
                    }
                    if inside {
                        if !seen_items.insert((item, 255)) {
                            continue;
                        }
                        let action = match (s.cat, b.as_str()) {
                            (_, "check" | "nonslam") => "Remove the disc, or fill and pressurise from upstream".to_string(),
                            (Cat::Control, _) => "Remove and fit a spool, or test through it fully open if its body is rated".into(),
                            (Cat::Valve, _) if pos == "NC" || pos == "LC" => format!("Open for the test (normally {pos}), close again after"),
                            (Cat::Valve, _) => "Open".into(),
                            (Cat::Primary | Cat::Instrument, _) => "Remove and fit a spool, or isolate".into(),
                            (Cat::Fitting, k) if k.starts_with("strainer") => "Remove the element".into(),
                            (Cat::Fitting, "spectacle") => "Check the blind is swung open".into(),
                            _ => continue,
                        };
                        d.inside.push(step(item, action));
                    } else if s.cat.valve() {
                        d.limits.push(step(item, "Close: test limit (blind beyond it if its seat is not rated for the test pressure)".into()));
                    } else {
                        d.limits.push(step(item, "Blind here: test limit".into()));
                    }
                }
            }
        }
    }
    d.inside.retain(|s| !vd.contains(&s.item));
    d.vents.dedup();
    d.drains.dedup();
    d.limits.dedup();
    d.problems.dedup();
    if d.vents.is_empty() {
        d.problems.push("No vent: add a high-point vent to fill and vent the pack.".into());
    }
    if d.drains.is_empty() {
        d.problems.push("No drain: add a low-point drain to empty the pack after the test.".into());
    }
    d
}

/// Every pack's detail.
pub fn details(p: &Project) -> Vec<Detail> {
    p.settings.testpacks.packs.iter().map(|k| detail(p, k)).collect()
}

/// The packs with their details, and the line numbers in none (with why).
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Overview {
    pub packs: Vec<Detail>,
    pub left_out: Vec<(String, String)>,
}

pub fn overview(p: &Project) -> Overview {
    let taken: BTreeSet<u32> = p.settings.testpacks.packs.iter().flat_map(|k| k.lines.iter().copied()).collect();
    let left_out = line_objs(p)
        .into_iter()
        .filter(|o| !taken.contains(o))
        .map(|o| (crate::graph::obj_number(p, o), if test_pressure(p, o, "Hydrostatic").is_some() { "not in a pack yet".into() } else { "no test or design pressure".into() }))
        .collect();
    Overview { packs: details(p), left_out }
}

/// The packs marked on a sheet: a dashed outline round each pack's pieces there, with its id.
pub fn prims(p: &Project, sheet: u32) -> Vec<Prim> {
    let s = &p.settings.testpacks;
    if !s.show || s.packs.is_empty() {
        return vec![];
    }
    let routes = crate::view::routes(p, sheet, None);
    let mut out = vec![];
    for k in &s.packs {
        let objs: BTreeSet<u32> = k.lines.iter().copied().collect();
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for l in p.lines.iter().filter(|l| l.sheet == sheet && objs.contains(&l.obj)) {
            for q in routes.get(&l.id).into_iter().flatten() {
                b = [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])];
            }
        }
        if b[0] > b[2] {
            continue;
        }
        let m = 3.0;
        let (x0, y0, x1, y1) = (b[0] - m, b[1] - m, b[2] + m, b[3] + m);
        out.push(Prim::Poly { pts: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], closed: true, fill: false, dash: true, w: 0.3 });
        out.push(Prim::Text { p: [x1, y1 + 2.5], h: 2.8, s: k.id.clone(), anchor: 2, bold: true });
    }
    out
}

/// The test pack list, and one pack's sheet ("testpack:TP-001").
pub fn list(p: &Project, kind: &str) -> Result<crate::lists::List, String> {
    use crate::lists::{Col, List, Row};
    let us = p.units == "US";
    let col = |k: &str, l: &str, q: &'static str| {
        let u = if q.is_empty() { String::new() } else { crate::units::shown(us).into_iter().find(|s| s.q == q).map(|s| s.label).unwrap_or_default() };
        Col { key: k.into(), label: if u.is_empty() { l.into() } else { format!("{l} ({u})") }, q: "", edit: false, num: !q.is_empty() }
    };
    let show = |v: f64, q: &str| if v > 0.0 { crate::fmt_sig(crate::units::to_display(v, q, us), 4) } else { String::new() };
    let pressure = |v: f64| if v > 0.0 { format!("{} g", crate::fmt_sig(crate::units::to_display(v, "press", us), 4)) } else { String::new() };
    let first_piece = |d: &Detail| d.pieces.first().and_then(|i| p.line(*i)).map_or((vec![], 0), |l| (vec![l.id], l.sheet));
    if let Some(id) = kind.strip_prefix("testpack:") {
        let pack = p.settings.testpacks.packs.iter().find(|k| k.id.eq_ignore_ascii_case(id)).ok_or_else(|| format!("There is no test pack {id}."))?;
        let d = detail(p, pack);
        let cols = vec![col("section", "Section", ""), col("item", "Item", ""), col("what", "What", ""), col("action", "Action or value", "")];
        let mut rows = vec![];
        let heads = vec![];
        let r = |a: &str, b: String, c: String, e: String, ids: Vec<u32>| Row { ids, sheet: 0, cells: vec![a.into(), b, c, e], status: None };
        rows.push(r("Test", pack.id.clone(), pack.medium.clone(), format!("Test pressure {}, fill volume {} — status {}{}", pressure(d.pressure), show(d.volume, "volume"), pack.status, if pack.witness.is_empty() { String::new() } else { format!(", witnessed by {}", pack.witness) }), vec![]));
        for l in &d.lines {
            rows.push(r("Line", l.number.clone(), l.class.clone(), format!("{} ({}), {} m, {} m³", pressure(l.test_p), l.source, crate::fmt_sig(l.length, 4), crate::fmt_sig(l.volume, 3)), vec![]));
        }
        for s in &d.limits {
            rows.push(r("Limit", s.tag.clone(), s.what.clone(), s.action.clone(), vec![s.item]));
        }
        for s in &d.inside {
            rows.push(r("Inside", s.tag.clone(), s.what.clone(), s.action.clone(), vec![s.item]));
        }
        for v in &d.vents {
            rows.push(r("Vent", v.clone(), String::new(), "Open to fill and vent, close before pressurising".into(), vec![]));
        }
        for v in &d.drains {
            rows.push(r("Drain", v.clone(), String::new(), "Drain after the test".into(), vec![]));
        }
        for x in &d.problems {
            rows.push(Row { ids: vec![], sheet: 0, cells: vec!["To resolve".into(), String::new(), String::new(), x.clone()], status: Some(1) });
        }
        if !pack.note.is_empty() {
            rows.push(r("Note", String::new(), String::new(), pack.note.clone(), vec![]));
        }
        return Ok(List { kind: kind.into(), title: format!("Test pack {}", pack.id), cols, rows, heads });
    }
    let cols = vec![col("pack", "Test pack", ""), col("lines", "Line numbers", ""), col("class", "Class", ""), col("medium", "Medium", ""), col("pressure", "Test pressure", "press"), col("volume", "Volume", "volume"), col("limits", "Limits", ""), col("inside", "To open or remove", ""), col("status", "Status", ""), col("problems", "To resolve", "")];
    let mut rows = vec![];
    for (k, d) in p.settings.testpacks.packs.iter().zip(details(p)) {
        let classes: BTreeSet<String> = d.lines.iter().map(|l| l.class.clone()).collect();
        let (ids, sheet) = first_piece(&d);
        rows.push(Row { ids, sheet, cells: vec![k.id.clone(), d.lines.iter().map(|l| l.number.clone()).collect::<Vec<_>>().join(", "), classes.into_iter().collect::<Vec<_>>().join(", "), k.medium.clone(), show(d.pressure, "press"), show(d.volume, "volume"), d.limits.len().to_string(), d.inside.len().to_string(), k.status.clone(), d.problems.join(" ")], status: (!d.problems.is_empty()).then_some(1) });
    }
    Ok(List { kind: kind.into(), title: "Test packs".into(), cols, rows, heads: vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    #[test]
    fn the_pump_transfer_makes_packs_with_their_limits_vents_and_drains() {
        let mut d = Doc::new(crate::examples::get("worked").unwrap());
        d.apply(Op::MakeTestPacks).unwrap();
        let packs = d.p.settings.testpacks.packs.clone();
        assert!(!packs.is_empty() && packs.iter().all(|k| k.id.starts_with("TP-")), "{packs:?}");
        // Every line number with a pressure is in exactly one pack.
        let all: Vec<u32> = packs.iter().flat_map(|k| k.lines.clone()).collect();
        let set: BTreeSet<u32> = all.iter().copied().collect();
        assert_eq!(all.len(), set.len());
        // The discharge (3"-P-1003, 15 bar test) runs through LV-101 to V-101: the vessel is a limit,
        // the control valve inside, the gauge to isolate.
        let dis = d.p.objects.iter().find(|o| crate::graph::obj_number(&d.p, o.id) == "3\"-P-1003-A1A").unwrap().id;
        let pk = packs.iter().find(|k| k.lines.contains(&dis)).unwrap();
        let det = detail(&d.p, pk);
        assert!((det.pressure - 15e5).abs() < 1.0, "{}", det.pressure);
        assert!(det.limits.iter().any(|s| s.tag == "V-101"), "{:#?}", det.limits);
        assert!(det.inside.iter().any(|s| s.tag == "LV-101" && s.action.starts_with("Remove and fit a spool")), "{:#?}", det.inside);
        assert!(det.inside.iter().any(|s| s.tag == "PI-102"), "{:#?}", det.inside);
        assert!(det.volume > 0.0 && det.lines.iter().any(|l| l.source == "its Test P"));
        // The suction pack has the pump's drain.
        let suc = d.p.objects.iter().find(|o| crate::graph::obj_number(&d.p, o.id) == "4\"-P-1001-A1A").unwrap().id;
        let sp = packs.iter().find(|k| k.lines.contains(&suc)).unwrap();
        let sd = detail(&d.p, sp);
        assert!(sd.limits.iter().any(|s| s.tag == "T-101") && sd.limits.iter().any(|s| s.tag == "P-101A"), "{:#?}", sd.limits);
        // Lists: the packs, and one pack's sheet.
        let l = crate::lists::build(&d.p, "testpacks").unwrap();
        assert_eq!(l.rows.len(), packs.len());
        let one = crate::lists::build(&d.p, &format!("testpack:{}", pk.id)).unwrap();
        assert!(one.rows.iter().any(|r| r.cells[0] == "Limit") && one.rows.iter().any(|r| r.cells[0] == "Inside"));
        // Marked on the sheet; kept through save and open; making again adds nothing.
        assert!(prims(&d.p, d.p.sheets[0].id).iter().any(|x| matches!(x, Prim::Text { s, .. } if s.starts_with("TP-"))));
        assert!(d.apply(Op::MakeTestPacks).is_err());
        let path = std::env::temp_dir().join(format!("tp-{}.dschema", std::process::id()));
        crate::file::write(&path, &d.p, &[]).unwrap();
        let (back, _, w) = crate::file::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(back.settings.testpacks, d.p.settings.testpacks);
        // A line number deleted leaves its pack.
        let piece = d.p.lines.iter().find(|l| l.obj == dis).unwrap().id;
        let n0 = d.p.settings.testpacks.packs.iter().map(|k| k.lines.len()).sum::<usize>();
        let pieces: Vec<u32> = d.p.lines.iter().filter(|l| l.obj == d.p.line(piece).unwrap().obj).map(|l| l.id).collect();
        d.apply(Op::Delete { ids: pieces, keep: false }).unwrap();
        assert!(d.p.settings.testpacks.packs.iter().map(|k| k.lines.len()).sum::<usize>() < n0);
    }

    #[test]
    fn test_pressure_from_design_and_differences_said() {
        let p = crate::examples::get("compressor").unwrap();
        for o in line_objs(&p) {
            if let Some((tp, src)) = test_pressure(&p, o, "Hydrostatic") {
                if src == "1.5 × design" {
                    assert!((tp - 1.5 * num(&p, o, "design_p").unwrap()).abs() < 1e-6);
                }
                let (pn, _) = test_pressure(&p, o, "Pneumatic").unwrap();
                assert!(pn <= tp);
            }
        }
        let mut q = p.clone();
        let objs: Vec<u32> = line_objs(&q).into_iter().take(2).collect();
        q.settings.testpacks.packs.push(Pack { id: "TP-900".into(), lines: objs.clone(), ..Pack::default() });
        if let Some(o) = q.obj_mut(objs[0]) {
            o.data.insert("test_p".into(), Val::Num(99e5));
        }
        let d = detail(&q, &q.settings.testpacks.packs[0]);
        assert!(d.problems.iter().any(|x| x.contains("different test pressures")), "{:?}", d.problems);
    }

    #[test]
    fn every_example_makes_packs_without_panic() {
        for ex in crate::examples::list().iter().map(|e| e.id) {
            let mut p = crate::examples::get(ex).unwrap();
            let _ = make(&mut p);
            for k in &p.settings.testpacks.packs {
                let d = detail(&p, k);
                assert!(!d.lines.is_empty(), "{ex}");
            }
            let _ = crate::lists::build(&p, "testpacks").unwrap();
        }
    }
}
