//! Pipe specs: each pipe class (A1A, B2A…) with its rating, material, size range and limits, and
//! the components it allows, by size, with a description and commodity code. Checks 21–23 hold
//! the drawing to them; the take-off names each component from its class.

use crate::checks::Issue;
use crate::graph::Net;
use crate::model::{Item, Project};
use crate::symbols::{self, Cat};
use serde::{Deserialize, Serialize};

/// A component a class allows: a kind of symbol (its base key, such as "gate") in a size range.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct SpecItem {
    pub kind: String,
    /// Smallest and largest size (NPS); empty is no limit.
    pub min: String,
    pub max: String,
    pub desc: String,
    pub code: String,
    /// How it joins its line: "flanged", "butt weld", "socket weld", "threaded" or "none"; empty
    /// is read from the description.
    pub ends: String,
}

/// What each end of a given kind brings with it (a weld-neck flange, a gasket and a bolt set at
/// every flanged end), in a size range.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Implied {
    pub what: String,
    pub ends: String,
    /// How many per end.
    pub per: f64,
    pub min: String,
    pub max: String,
    pub desc: String,
    pub code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct PipeSpec {
    pub name: String,
    pub desc: String,
    pub rating: String,
    pub material: String,
    /// Corrosion allowance (m).
    pub corrosion: f64,
    /// The class's limits: design pressure (Pa) and temperature (K) at most; 0 is none.
    pub max_p: f64,
    pub max_t: f64,
    /// The smallest and largest line size (NPS); empty is no limit.
    pub min: String,
    pub max: String,
    /// The schedule its pipe is made in.
    pub sched: String,
    pub items: Vec<SpecItem>,
    /// What each end of a component brings with it, for the take-off.
    pub implied: Vec<Implied>,
}

pub const ENDS: [&str; 5] = ["flanged", "butt weld", "socket weld", "threaded", "none"];

/// A component's end type: as set, else read from its description.
pub fn ends_of(i: &SpecItem) -> &'static str {
    if let Some(e) = ENDS.iter().find(|e| i.ends.eq_ignore_ascii_case(e)) {
        return e;
    }
    let d = i.desc.to_lowercase();
    if d.contains("flanged") || d.contains(" rf") || d.contains(", rf") || d.contains("wafer") || d.contains("lug") {
        "flanged"
    } else if d.contains("socket") || d.contains(" sw") {
        "socket weld"
    } else if d.contains("thread") || d.contains("npt") || d.contains("screwed") {
        "threaded"
    } else if d.contains("bw") || d.contains("butt") {
        "butt weld"
    } else {
        "none"
    }
}

pub const MAX_SPECS: usize = 200;
pub const MAX_ITEMS: usize = 500;

/// Specs made sound: names unique (case aside) and clipped, numbers finite, lists bounded.
pub fn normalize(specs: &mut Vec<PipeSpec>) {
    let mut seen = std::collections::HashSet::new();
    specs.truncate(MAX_SPECS);
    for s in specs.iter_mut() {
        s.name = crate::clip(s.name.trim(), 30);
    }
    specs.retain(|s| !s.name.is_empty() && seen.insert(s.name.to_uppercase()));
    let fin = |v: f64| if v.is_finite() { v.max(0.0) } else { 0.0 };
    for s in specs.iter_mut() {
        s.desc = crate::clip(&s.desc, 200);
        s.rating = crate::clip(&s.rating, 30);
        s.material = crate::clip(&s.material, 100);
        s.corrosion = fin(s.corrosion).min(0.1);
        s.max_p = fin(s.max_p).min(1e9);
        s.max_t = fin(s.max_t).min(2000.0);
        s.min = crate::clip(s.min.trim(), 12);
        s.max = crate::clip(s.max.trim(), 12);
        s.sched = crate::clip(s.sched.trim(), 12);
        s.items.truncate(MAX_ITEMS);
        s.items.retain(|i| !i.kind.trim().is_empty());
        s.implied.truncate(50);
        s.implied.retain(|i| !i.what.trim().is_empty());
        for i in &mut s.implied {
            i.what = crate::clip(i.what.trim(), 60);
            i.ends = crate::clip(i.ends.trim(), 20);
            i.per = if i.per.is_finite() { i.per.clamp(0.0, 100.0) } else { 1.0 };
            i.min = crate::clip(i.min.trim(), 12);
            i.max = crate::clip(i.max.trim(), 12);
            i.desc = crate::clip(&i.desc, 200);
            i.code = crate::clip(&i.code, 60);
        }
        for i in &mut s.items {
            i.ends = crate::clip(i.ends.trim(), 20);
            i.kind = crate::clip(i.kind.trim(), 40);
            i.min = crate::clip(i.min.trim(), 12);
            i.max = crate::clip(i.max.trim(), 12);
            i.desc = crate::clip(&i.desc, 200);
            i.code = crate::clip(&i.code, 60);
        }
    }
}

fn inches(s: &str) -> Option<f64> {
    crate::pipes::nps_inches(s)
}

/// Whether a size (NPS text) is within a range (an empty end is open; an unreadable size is let be).
fn within(size: &str, min: &str, max: &str) -> bool {
    let Some(x) = inches(size) else { return true };
    inches(min).is_none_or(|m| x >= m - 1e-9) && inches(max).is_none_or(|m| x <= m + 1e-9)
}

pub fn find<'a>(p: &'a Project, class: &str) -> Option<&'a PipeSpec> {
    let c = class.trim();
    p.settings.specs.iter().find(|s| s.name.eq_ignore_ascii_case(c))
}

/// An in-line item's pipe class and size: its own size, else that of the lines it sits in (the
/// largest; terminals such as vents only their own); the class from those lines (the first with one).
pub fn item_class_size(p: &Project, net: &Net, it: &Item) -> (String, String) {
    let txt = |d: &crate::model::Data, k: &str| d.get(k).map(|v| v.text().trim().to_string()).unwrap_or_default();
    let Some(s) = symbols::get(&it.sym) else { return Default::default() };
    let mut class = String::new();
    let mut size = String::new();
    for k in (0..s.ports.len()).filter(|k| !s.ports[*k].signal) {
        for &(l, _) in net.on_port(it.id, k as u8) {
            let Some(line) = net.lines.get(&l).filter(|x| x.kind.process() && x.obj != 0) else { continue };
            let d = p.data_of(line.obj);
            if class.is_empty() {
                class = txt(d, "class");
            }
            let sz = txt(d, "size");
            if inches(&sz).unwrap_or(0.0) > inches(&size).unwrap_or(-1.0) {
                size = sz;
            }
        }
    }
    let own = txt(p.data_of(it.obj), "size");
    // A vent, drain or sample point is a small branch: only its own size counts.
    if s.terminal {
        return (class, own);
    }
    (class, if own.is_empty() { size } else { own })
}

/// The class's entry for an item, if it allows it at that size.
pub fn entry<'a>(spec: &'a PipeSpec, kind: &str, size: &str) -> Option<&'a SpecItem> {
    spec.items.iter().find(|i| i.kind == kind && within(size, &i.min, &i.max))
}

/// Piping components the specs speak about: valves, fittings, control and relief valves, and
/// in-line primary elements.
pub fn is_piping(s: &crate::symbols::Sym) -> bool {
    matches!(s.cat, Cat::Valve | Cat::Control | Cat::Fitting) || (s.cat == Cat::Primary && s.inline)
}

/// Checks 21 (a line's class is not one of the project's specs), 22 (a component its class does
/// not allow at its size) and 23 (a line outside its class's sizes or design limits). Only when
/// the project has specs.
pub fn checks(p: &Project, net: &Net, on: &dyn Fn(u8) -> bool) -> Vec<Issue> {
    let mut v = vec![];
    if p.settings.specs.is_empty() {
        return v;
    }
    let us = p.units == "US";
    let pfd: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    for (r, lines) in net.runs.iter().enumerate() {
        let l = net.lines[&lines[0]];
        if pfd.contains(&l.sheet) {
            continue;
        }
        let Some(o) = p.obj(net.run_obj[r]).filter(|o| o.class == crate::model::LINE_CLASS) else { continue };
        let class = o.data.get("class").map(|v| v.text().trim().to_string()).unwrap_or_default();
        if class.is_empty() {
            continue;
        }
        let name = net.numbers.get(&l.id).cloned().filter(|n| !n.is_empty()).map_or("A line".to_string(), |n| format!("Line {n}"));
        let at = crate::graph::end_pt(p, &l.a);
        let issue = |rule: u8, level: &'static str, msg: String| Issue { rule, level, msg, ids: lines.clone(), sheet: l.sheet, at };
        let Some(spec) = find(p, &class) else {
            if on(21) {
                v.push(issue(21, "warning", format!("{name} is in pipe class {class}, which is not one of the project's pipe specs.")));
            }
            continue;
        };
        if on(23) {
            let size = o.data.get("size").map(|v| v.text()).unwrap_or_default();
            if !size.trim().is_empty() && !within(&size, &spec.min, &spec.max) {
                v.push(issue(23, "warning", format!("{name} is {}\" but pipe class {} runs from {} to {}.", size.trim_end_matches('"'), spec.name, if spec.min.is_empty() { "any size" } else { &spec.min }, if spec.max.is_empty() { "any size" } else { &spec.max })));
            }
            let num = |k: &str| o.data.get(k).and_then(|v| v.num());
            if let Some(dp) = num("design_p").filter(|x| spec.max_p > 0.0 && *x > spec.max_p * 1.0001) {
                v.push(issue(23, "warning", format!("{name}'s design pressure {} is above pipe class {}'s {}.", crate::units::show(dp, "press", us), spec.name, crate::units::show(spec.max_p, "press", us))));
            }
            if let Some(dt) = num("design_t").filter(|x| spec.max_t > 0.0 && *x > spec.max_t + 1e-6) {
                v.push(issue(23, "warning", format!("{name}'s design temperature {} is above pipe class {}'s {}.", crate::units::show(dt, "temp", us), spec.name, crate::units::show(spec.max_t, "temp", us))));
            }
        }
    }
    // 22. Components in their line's class.
    if on(22) {
        let mut seen = std::collections::HashSet::new();
        for it in p.items.iter().filter(|i| !pfd.contains(&i.sheet)) {
            let Some(s) = symbols::get(&it.sym).filter(|s| is_piping(s)) else { continue };
            if it.obj != 0 && !seen.insert(it.obj) {
                continue;
            }
            let (class, size) = item_class_size(p, net, it);
            let Some(spec) = find(p, &class) else { continue };
            let kind = it.base_key();
            if entry(spec, &kind, &size).is_some() {
                continue;
            }
            let what = symbols::get(&kind).map_or(s.name, |x| x.name).to_lowercase();
            let tag = p.tag_of(it).trim().to_string();
            let who = if tag.is_empty() { format!("A {what}") } else { format!("{tag} ({what})") };
            let sz = if size.is_empty() { String::new() } else { format!(" at {}\"", size.trim_end_matches('"')) };
            let ranges: Vec<String> = spec.items.iter().filter(|i| i.kind == kind).map(|i| format!("{}–{}", if i.min.is_empty() { "any" } else { &i.min }, if i.max.is_empty() { "any" } else { &i.max })).collect();
            let msg = if ranges.is_empty() { format!("{who} is not in pipe class {}.", spec.name) } else { format!("{who}{sz} is outside pipe class {}'s sizes for it ({}).", spec.name, ranges.join(", ")) };
            v.push(Issue { rule: 22, level: "warning", msg, ids: vec![it.id], sheet: it.sheet, at: [it.x, it.y] });
        }
    }
    v
}

/// One line of implied items in the take-off: what, size, class, how many, and the components
/// that bring them.
#[derive(Clone, Debug, PartialEq)]
pub struct ImpliedRow {
    pub what: String,
    pub size: String,
    pub class: String,
    pub qty: f64,
    pub desc: String,
    pub code: String,
    pub ids: Vec<u32>,
    pub from: Vec<String>,
}

/// The implied items of every piping component on the P&IDs: for each of its ends whose type a
/// rule of its class names, that rule's items (one count per object). Rows are by item, size and
/// class.
pub fn implied(p: &Project, net: &Net) -> Vec<ImpliedRow> {
    let pfd: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    let mut rows: std::collections::BTreeMap<(String, (u64, String), String), ImpliedRow> = std::collections::BTreeMap::new();
    let mut seen = std::collections::HashSet::new();
    for it in p.items.iter().filter(|i| !pfd.contains(&i.sheet)) {
        let Some(s) = symbols::get(&it.sym).filter(|s| is_piping(s)) else { continue };
        if it.obj != 0 && !seen.insert(it.obj) {
            continue;
        }
        let (class, size) = item_class_size(p, net, it);
        let Some(spec) = find(p, &class) else { continue };
        let kind = it.base_key();
        let Some(e) = entry(spec, &kind, &size) else { continue };
        let ends = ends_of(e);
        let n_ends = s.ports.iter().filter(|q| !q.signal).count() as f64;
        let name = symbols::get(&kind).map_or(s.name, |x| x.name).to_lowercase();
        for r in spec.implied.iter().filter(|r| r.ends.eq_ignore_ascii_case(ends) && within(&size, &r.min, &r.max)) {
            let order = (inches(&size).map_or(u64::MAX, |x| (x * 1000.0) as u64), size.clone());
            let row = rows.entry((r.what.clone(), order, spec.name.clone())).or_insert_with(|| ImpliedRow { what: r.what.clone(), size: size.clone(), class: spec.name.clone(), qty: 0.0, desc: r.desc.clone(), code: r.code.clone(), ids: vec![], from: vec![] });
            row.qty += r.per * n_ends;
            row.ids.push(it.id);
            if !row.from.contains(&name) {
                row.from.push(name.clone());
            }
        }
    }
    rows.into_values().collect()
}

/// Spec items read from a CSV table (one row per component: Class, Component, From, To,
/// Description, Code; headers matched by name, in any order, comma or semicolon separated).
/// Components are matched to symbols by key or name. The classes named replace their items in
/// `specs`; new classes are added. Returns the specs and what was not read.
pub fn import_csv(mut specs: Vec<PipeSpec>, text: &str) -> Result<(Vec<PipeSpec>, Vec<String>), String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let head = lines.next().ok_or("The file is empty.")?;
    let sep = if head.matches(';').count() > head.matches(',').count() { ';' } else { ',' };
    let split = |l: &str| -> Vec<String> {
        // Quoted fields may hold the separator.
        let mut out = vec![];
        let mut cur = String::new();
        let mut q = false;
        let mut cs = l.chars().peekable();
        while let Some(c) = cs.next() {
            match c {
                '"' if q && cs.peek() == Some(&'"') => {
                    cur.push('"');
                    cs.next();
                }
                '"' => q = !q,
                c if c == sep && !q => out.push(std::mem::take(&mut cur)),
                c => cur.push(c),
            }
        }
        out.push(cur);
        out.into_iter().map(|s| s.trim().to_string()).collect()
    };
    let h: Vec<String> = split(head).into_iter().map(|s| s.to_lowercase()).collect();
    let col = |names: &[&str]| h.iter().position(|x| names.iter().any(|n| x.starts_with(n)));
    let (ci, ki) = (col(&["class", "spec"]).ok_or("No Class column.")?, col(&["component", "item", "type"]).ok_or("No Component column.")?);
    let (fi, ti, di, oi) = (col(&["from", "min", "size from"]), col(&["to", "max", "size to"]), col(&["desc"]), col(&["code", "commodity"]));
    let ei = col(&["end"]);
    let syms: Vec<&'static crate::symbols::Sym> = symbols::all().iter().filter(|s| is_piping(s)).collect();
    let mut w = vec![];
    let mut got: std::collections::BTreeMap<String, Vec<SpecItem>> = std::collections::BTreeMap::new();
    for (n, l) in lines.enumerate().take(20_000) {
        let r = split(l);
        let get = |i: Option<usize>| i.and_then(|i| r.get(i)).cloned().unwrap_or_default();
        let (class, comp) = (get(Some(ci)), get(Some(ki)));
        if class.is_empty() || comp.is_empty() {
            continue;
        }
        let Some(sym) = syms.iter().find(|s| s.key.eq_ignore_ascii_case(&comp) || s.name.eq_ignore_ascii_case(&comp)) else {
            w.push(format!("Row {}: “{}” is not a valve or fitting Datum Schema knows.", n + 2, crate::clip(&comp, 40)));
            continue;
        };
        got.entry(class).or_default().push(SpecItem { kind: sym.key.to_string(), min: get(fi), max: get(ti), desc: get(di), code: get(oi), ends: get(ei).to_lowercase() });
    }
    if got.is_empty() {
        return Err(if w.is_empty() { "No components were found in the file.".into() } else { w.join(" ") });
    }
    for (class, items) in got {
        match specs.iter_mut().find(|s| s.name.eq_ignore_ascii_case(&class)) {
            Some(s) => s.items = items,
            None => specs.push(PipeSpec { name: class, items, ..PipeSpec::default() }),
        }
    }
    normalize(&mut specs);
    Ok((specs, w))
}

/// A typical carbon steel class to start from: 150# RF, ½"–24", the usual valves and fittings.
pub fn starter(name: &str) -> PipeSpec {
    let i = |kind: &str, min: &str, max: &str, desc: &str, code: &str| SpecItem { kind: kind.into(), min: min.into(), max: max.into(), desc: desc.into(), code: code.into(), ends: String::new() };
    PipeSpec {
        name: name.into(),
        desc: "Carbon steel, ASME B16.5 class 150, raised face".into(),
        rating: "150#".into(),
        material: "ASTM A106 Gr. B".into(),
        corrosion: 0.0015,
        max_p: 19.6e5,
        // 19.6 bar at ambient; usable to 425 °C at lower pressures (ASME B16.5 P-T table).
        max_t: 425.0 + 273.15,
        min: "1/2".into(),
        max: "24".into(),
        sched: "40".into(),
        items: vec![
            i("gate", "2", "24", "Gate valve, flanged RF, A216 WCB, trim 8", "VGA150-01"),
            i("gate", "1/2", "1-1/2", "Gate valve, socket weld, A105, trim 8", "VGA800-01"),
            i("globe", "1/2", "12", "Globe valve, flanged RF, A216 WCB", "VGL150-01"),
            i("ball", "1/2", "8", "Ball valve, full bore, flanged RF, A216 WCB", "VBA150-01"),
            i("butterfly", "3", "24", "Butterfly valve, double offset, wafer, WCB", "VBF150-01"),
            i("check", "2", "24", "Swing check valve, flanged RF, A216 WCB", "VCH150-01"),
            i("check", "1/2", "1-1/2", "Piston check valve, socket weld, A105", "VCH800-01"),
            i("plug", "1/2", "12", "Plug valve, lubricated, flanged RF", "VPL150-01"),
            i("strainer_y", "1/2", "12", "Y-strainer, flanged RF, WCB, 40 mesh", "SY150-01"),
            i("strainer_basket", "2", "24", "Basket strainer, flanged RF, WCB", "SB150-01"),
            i("reducer_con", "3/4", "24", "Concentric reducer, BW, A234 WPB", "RC-01"),
            i("reducer_ecc", "3/4", "24", "Eccentric reducer, BW, A234 WPB", "RE-01"),
            SpecItem { ends: "none".into(), ..i("blind", "1/2", "24", "Blind flange, RF, A105", "FB150-01") },
            SpecItem { ends: "none".into(), ..i("spectacle", "1/2", "12", "Spectacle blind, RF, A516 Gr. 70", "SP150-01") },
            i("cv_pneu", "1", "16", "Control valve, globe, flanged RF, WCB (see datasheet)", "CV-SEE-DS"),
            i("cv_motor", "1", "16", "Motor-operated valve, flanged RF, WCB", "MOV-01"),
            i("psv", "1/2", "8", "Pressure safety valve, flanged RF (see datasheet)", "PSV-SEE-DS"),
            i("vent", "1/2", "1", "Vent, gate valve and plug", "VNT-01"),
            i("drain", "1/2", "1", "Drain, gate valve and plug", "DRN-01"),
            SpecItem { ends: "none".into(), ..i("orifice", "1", "24", "Orifice plate and flanges (see datasheet)", "FE-SEE-DS") },
        ],
        implied: vec![
            Implied { what: "Weld-neck flange".into(), ends: "flanged".into(), per: 1.0, min: "1/2".into(), max: "24".into(), desc: "Weld-neck flange, RF, class 150, A105, bore to schedule".into(), code: "FWN150-01".into() },
            Implied { what: "Gasket".into(), ends: "flanged".into(), per: 1.0, min: "1/2".into(), max: "24".into(), desc: "Spiral wound gasket, 316/graphite, CS outer ring, class 150".into(), code: "GSW150-01".into() },
            Implied { what: "Bolt set".into(), ends: "flanged".into(), per: 1.0, min: "1/2".into(), max: "24".into(), desc: "Stud bolts A193 B7 with two A194 2H nuts, class 150".into(), code: "BS150-01".into() },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::Val;

    fn issues(p: &Project) -> Vec<Issue> {
        let net = Net::new(p);
        checks(p, &net, &|_| true)
    }

    #[test]
    fn sizes_and_ranges() {
        assert!(within("3", "2", "24") && within("2", "2", "24") && !within("1-1/2", "2", "24"));
        assert!(within("3", "", "") && within("odd", "2", "4"));
        let s = starter("A1A");
        assert_eq!(entry(&s, "gate", "1").unwrap().code, "VGA800-01");
        assert_eq!(entry(&s, "gate", "6").unwrap().code, "VGA150-01");
        assert!(entry(&s, "ball", "10").is_none());
    }

    #[test]
    fn flanged_components_bring_flanges_gaskets_and_bolts() {
        let s = starter("A1A");
        assert_eq!(ends_of(entry(&s, "gate", "6").unwrap()), "flanged");
        assert_eq!(ends_of(entry(&s, "gate", "1").unwrap()), "socket weld");
        assert_eq!(ends_of(entry(&s, "reducer_con", "4").unwrap()), "butt weld");
        assert_eq!(ends_of(entry(&s, "blind", "4").unwrap()), "none");
        let p = crate::examples::get("pump").unwrap();
        let net = Net::new(&p);
        let rows = implied(&p, &net);
        // Seven 3" flanged components (four gates, two checks, the level control valve), two ends
        // each: 14 gaskets, 14 flanges, 14 bolt sets.
        let g3 = rows.iter().find(|r| r.what == "Gasket" && r.size == "3").unwrap();
        assert_eq!(g3.qty, 14.0, "{rows:#?}");
        assert!(g3.from.contains(&"gate valve".to_string()) && g3.from.contains(&"check valve".to_string()));
        assert_eq!(g3.code, "GSW150-01");
        assert!(rows.iter().any(|r| r.what == "Bolt set" && r.size == "4"));
        // In the take-off, after the components.
        let l = crate::lists::build(&p, "takeoff").unwrap();
        let r = l.rows.iter().find(|r| r.cells[0] == "Gasket" && r.cells[1] == "3\"").unwrap();
        assert_eq!(r.cells[4], "14");
        assert!(r.cells[5].starts_with("implied by"));
        // Without specs, nothing is implied.
        let mut q = p.clone();
        q.settings.specs.clear();
        assert!(implied(&q, &Net::new(&q)).is_empty());
    }

    #[test]
    fn a_spec_table_is_read_from_csv() {
        let csv = "Class;Component;From;To;Description;Commodity code\nA1A;Gate valve;2;24;\"Gate valve; RF, WCB\";VGA-01\nA1A;ball;1/2;8;Ball valve;VBA-01\nB2A;Check valve;2;12;Swing check;VCH-02\nB2A;Teapot;1;2;x;y\n";
        let (s, w) = import_csv(vec![starter("A1A")], csv).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].items.len(), 2, "A1A's items are replaced");
        assert_eq!(s[0].rating, "150#", "its other data kept");
        assert_eq!(s[0].items[0].desc, "Gate valve; RF, WCB");
        assert_eq!((s[1].name.as_str(), s[1].items[0].kind.as_str()), ("B2A", "check"));
        assert!(w.len() == 1 && w[0].contains("Teapot"));
        assert!(import_csv(vec![], "Nothing,here\n1,2").is_err());
        assert!(import_csv(vec![], "").is_err());
    }

    #[test]
    fn pipe_specs_hold_the_drawing() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        // No specs: nothing to say.
        assert!(issues(&d.p).is_empty());
        let mut st = d.p.settings.clone();
        st.specs = vec![starter("A1A")];
        d.apply(Op::SetSettings { settings: st.clone() }).unwrap();
        let w = issues(&d.p);
        // The pump example's components are all in the starter class, except the breather.
        assert!(w.iter().all(|i| i.rule == 22 && i.msg.contains("PVSV-101")), "{w:#?}");
        // 21: a class that is not a spec.
        let hv = d.p.items.iter().find(|i| d.p.tag_of(i) == "HV-104").unwrap().id;
        let line = d.p.lines.iter().find(|l| matches!(l.a, crate::model::End::Port { item, .. } if item == hv) || matches!(l.b, crate::model::End::Port { item, .. } if item == hv)).unwrap().id;
        d.apply(Op::SetData { ids: vec![line], key: "class".into(), val: Some(Val::Text("Z9Z".into())) }).unwrap();
        assert!(issues(&d.p).iter().any(|i| i.rule == 21 && i.msg.contains("Z9Z")));
        d.apply(Op::SetData { ids: vec![line], key: "class".into(), val: Some(Val::Text("a1a".into())) }).unwrap();
        assert!(!issues(&d.p).iter().any(|i| i.rule == 21), "class names match whatever their case");
        // 22: a ball valve above its range, and a kind the class does not list.
        let mut st2 = st.clone();
        st2.specs[0].items.retain(|i| i.kind != "gate");
        d.apply(Op::SetSettings { settings: st2 }).unwrap();
        assert!(issues(&d.p).iter().any(|i| i.rule == 22 && i.msg.contains("HV-104") && i.msg.contains("is not in pipe class A1A")));
        // 23: design pressure above the class, and a line bigger than the class.
        let mut st3 = st.clone();
        st3.specs[0].max_p = 5e5;
        st3.specs[0].max = "3".into();
        d.apply(Op::SetSettings { settings: st3 }).unwrap();
        let w = issues(&d.p);
        assert!(w.iter().any(|i| i.rule == 23 && i.msg.contains("design pressure")), "{w:#?}");
        assert!(w.iter().any(|i| i.rule == 23 && i.msg.contains("4\" but pipe class A1A runs from 1/2 to 3")), "{w:#?}");
        // Specs are kept sound.
        let mut bad = vec![starter("A1A"), starter("a1a"), PipeSpec { name: " ".into(), ..starter("X") }, PipeSpec { max_p: f64::NAN, corrosion: -1.0, ..starter("B1") }];
        normalize(&mut bad);
        assert_eq!(bad.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["A1A", "B1"]);
        assert!(bad[1].max_p == 0.0 && bad[1].corrosion == 0.0);
    }
}
