//! Datasheets: one per instrument, primary element, control valve or relief device, built from
//! the object's class attributes and the process conditions of the line it measures or sits in.
//! The layout is Datum Schema's own: General · Process conditions · the class's attributes ·
//! Specification · the project's own attributes. Values the datasheet takes from elsewhere (the
//! line, the drawing) are marked with their source and are edited there; the object's own values
//! are edited on the datasheet.
//!
//! A datasheet is issued with a revision and a status; the issue keeps what the datasheet said
//! (in SI display units: bar, °C…), so the next revision marks every value changed since (Δ) and the revision block
//! lists every issue.

use crate::classes::{self, Attr};
use crate::graph::{instrument_connection, Net};
use crate::model::{End, Issue, Item, Project, Val, STATUSES};
use std::collections::BTreeMap;
use crate::symbols::{self, Cat};
use crate::{tags, units};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Row {
    /// The attribute key (editable rows), empty for derived ones.
    pub key: String,
    pub label: String,
    pub value: String,
    pub unit: String,
    /// The quantity, for converting typed values back to SI.
    pub q: String,
    pub choices: Vec<String>,
    pub required: bool,
    /// Where a derived value comes from ("line 3\"-P-1003-A1A", "drawing"); empty: the object's own.
    pub source: String,
    /// Changed since the last issued revision (Δ), and what it was then (in SI display units).
    pub changed: bool,
    pub was: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Section {
    pub title: String,
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Datasheet {
    /// The object, and a symbol showing it (for "go to").
    pub obj: u32,
    pub item: u32,
    pub sheet: u32,
    pub tag: String,
    pub title: String,
    pub sections: Vec<Section>,
    /// The last issued revision (empty: never issued) and its status, date and author.
    pub rev: String,
    pub status: String,
    pub date: String,
    pub by: String,
    /// Every issue of this datasheet, oldest first (the revision block).
    pub issues: Vec<IssueInfo>,
    /// Rows changed since the last issue.
    pub changed: usize,
    /// The revision to suggest for the next issue, per status.
    pub next_rev: BTreeMap<String, String>,
    /// The company layout's form number, logo (data URL) and footer.
    pub form: String,
    pub logo: String,
    pub footer: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct IssueInfo {
    pub rev: String,
    pub status: String,
    pub date: String,
    pub by: String,
    pub note: String,
}

/// Statuses issued with numbered revisions (0, 1, 2…); the others take letters (A, B, C…).
fn numbered(status: &str) -> bool {
    status == "Issued for Purchase" || status == "As-built"
}

/// The next revision of an object's datasheet for a status: letters before purchase (A, B…, Z,
/// AA…), numbers from purchase on (0, 1…).
pub fn next_rev(p: &Project, obj: u32, status: &str) -> String {
    let revs: Vec<&str> = p.issues.iter().filter(|i| i.obj == obj).map(|i| i.rev.as_str()).collect();
    if numbered(status) {
        return revs.iter().filter_map(|r| r.parse::<u32>().ok()).max().map_or("0".into(), |n| (n + 1).to_string());
    }
    let letters = |r: &str| !r.is_empty() && r.chars().all(|c| c.is_ascii_uppercase());
    // Base-26 with A = 1: A … Z, AA, AB …
    let value = |r: &str| r.chars().fold(0u64, |n, c| n.saturating_mul(26).saturating_add(c as u64 - 'A' as u64 + 1));
    let Some(n) = revs.iter().filter(|r| letters(r) && r.len() <= 6).map(|r| value(r)).max() else { return "A".into() };
    let mut n = n + 1;
    let mut s = vec![];
    while n > 0 {
        let d = ((n - 1) % 26) as u8;
        s.push((b'A' + d) as char);
        n = (n - 1) / 26;
    }
    s.iter().rev().collect()
}

/// Instruments, primary elements, control valves and relief devices: the instrument datasheets
/// (what a loop and a process source are found among).
pub fn has_datasheet(cat: Cat) -> bool {
    cat.instrument() || cat == Cat::Control
}

/// Everything with a datasheet: the instrument datasheets and equipment (vessels, machines, exchangers).
pub fn has_sheet(cat: Cat) -> bool {
    has_datasheet(cat) || cat.equipment()
}

/// The ISA-5.1 measured variable of a first letter.
fn variable(letters: &str) -> &'static str {
    match letters.chars().next() {
        Some('A') => "Analysis",
        Some('F') => "Flow",
        Some('L') => "Level",
        Some('P') => "Pressure",
        Some('T') => "Temperature",
        Some('D') => "Density",
        Some('W') => "Weight",
        Some('S') => "Speed",
        Some('V') => "Vibration",
        Some('Z') => "Position",
        Some('U') => "Multivariable",
        Some('H') => "Hand",
        Some('E') => "Voltage",
        Some('I') => "Current",
        Some('J') => "Power",
        _ => "",
    }
}

/// Where an item's process conditions come from: the line-number object it measures or sits in
/// (its own line for in-line elements, the line its impulse line ends on, the line through a
/// primary element it is tapped to), else the equipment it is tapped to (a level transmitter on a
/// vessel), else another member of its loop (a controller takes its transmitter's line). Returns
/// the object and how to name it ("line 3\"-P-1003-A1A", "V-101").
pub fn process_source(p: &Project, net: &Net, it: &Item) -> Option<(u32, String)> {
    let as_line = |o: u32| (o, format!("line {}", crate::graph::obj_number(p, o)));
    let direct = |it: &Item| -> Option<(u32, String)> {
        let s = symbols::get(&it.sym)?;
        let mut equipment = None;
        for k in 0..s.ports.len() {
            for &(l, b) in net.on_port(it.id, k as u8) {
                let Some(line) = net.lines.get(&l) else { continue };
                if !line.kind.process() {
                    // Signal lines carry no process: a controller is found through its loop.
                    continue;
                }
                if !instrument_connection(p, line) && line.obj != 0 {
                    return Some(as_line(line.obj));
                }
                match if b { &line.a } else { &line.b } {
                    End::Line { line: m, .. } => {
                        if let Some(ml) = net.lines.get(m).filter(|x| x.kind.process() && x.obj != 0) {
                            return Some(as_line(ml.obj));
                        }
                    }
                    End::Port { item, .. } => {
                        let Some(other) = net.items.get(item) else { continue };
                        let Some(os) = symbols::get(&other.sym) else { continue };
                        if os.inline {
                            // Tapped to an in-line element: the element's line.
                            for kk in 0..os.ports.len() {
                                if let Some(y) = net.on_port(other.id, kk as u8).iter().filter_map(|x| net.lines.get(&x.0)).find(|y| y.kind.process() && y.obj != 0 && !instrument_connection(p, y)) {
                                    return Some(as_line(y.obj));
                                }
                            }
                        } else if !has_datasheet(os.cat) && other.obj != 0 && equipment.is_none() {
                            equipment = Some((other.obj, p.obj(other.obj).map(|o| o.tag.clone()).unwrap_or_default()));
                        }
                    }
                    End::Free { .. } => {}
                }
            }
        }
        equipment
    };
    if let Some(o) = direct(it) {
        return Some(o);
    }
    // A controller or indicator: the source of another member of its loop (same first letter
    // and number: LIC-101 goes with LT-101, not with PVSV-101).
    let key = |tag: &str| {
        let (letters, n) = tags::loop_of(tag);
        (letters.chars().next(), n)
    };
    let me = key(p.tag_of(it));
    if me.1.is_empty() {
        return None;
    }
    // The measurement first (a transmitter or an element), then the rest of the loop.
    let mut members: Vec<&Item> = p.items.iter().filter(|o| o.id != it.id && key(p.tag_of(o)) == me).collect();
    members.sort_by_key(|o| !tags::loop_of(p.tag_of(o)).0.chars().skip(1).any(|c| c == 'T' || c == 'E'));
    members.into_iter().find_map(direct)
}

fn show(v: Option<&Val>, q: &str, us: bool) -> (String, String) {
    let unit = if q.is_empty() { String::new() } else { units::shown(us).into_iter().find(|s| s.q == q).map(|s| s.label).unwrap_or_default() };
    let val = match v {
        None => String::new(),
        Some(x) if !q.is_empty() => x.num().map(|n| crate::fmt_sig(units::to_display(n, q, us), 4)).unwrap_or_else(|| x.text()),
        Some(x) => x.text(),
    };
    (val, unit)
}

fn derived(label: &str, value: String, unit: String, source: &str) -> Row {
    Row { key: String::new(), label: label.into(), value, unit, q: String::new(), choices: vec![], required: false, source: source.into(), changed: false, was: String::new() }
}

/// The datasheet of an object (by object id or the id of a symbol showing it).
pub fn build(p: &Project, id: u32) -> Result<Datasheet, String> {
    build_in(p, &Net::new(p), id)
}

/// A datasheet as the company layout shows it: rows left out, sections in order, the form number,
/// logo and footer.
pub fn build_in(p: &Project, net: &Net, id: u32) -> Result<Datasheet, String> {
    let mut d = full(p, net, id)?;
    let l = &p.settings.ds_layout;
    for s in &mut d.sections {
        let t = s.title.clone();
        s.rows.retain(|r| !l.hidden.contains(&key(&t, &r.label)));
    }
    d.sections.retain(|s| !s.rows.is_empty());
    // Stable: named sections first in the layout's order, then the rest as they were.
    d.sections.sort_by_key(|s| l.order.iter().position(|o| *o == s.title).unwrap_or(usize::MAX));
    d.changed = d.sections.iter().flat_map(|s| &s.rows).filter(|r| r.changed).count();
    (d.form, d.logo, d.footer) = (l.form.clone(), l.logo.clone(), l.footer.clone());
    Ok(d)
}

/// Every section and row a datasheet in this project can have ("Section", ["Label", …]), in the
/// usual order: what the layout can leave out or reorder.
pub fn layout_rows(p: &Project) -> Vec<(String, Vec<String>)> {
    let net = Net::new(p);
    let mut out: Vec<(String, Vec<String>)> = vec![];
    let mut seen = std::collections::HashSet::new();
    for i in p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| has_sheet(s.cat)) && seen.insert(i.obj)) {
        let Ok(d) = full(p, &net, i.obj) else { continue };
        for s in d.sections {
            let at = match out.iter().position(|x| x.0 == s.title) {
                Some(k) => k,
                None => {
                    out.push((s.title.clone(), vec![]));
                    out.len() - 1
                }
            };
            for r in s.rows {
                if !out[at].1.contains(&r.label) {
                    out[at].1.push(r.label);
                }
            }
        }
    }
    out
}

/// A datasheet with every row and its revision state: the last issue, the revision block and the
/// Δ marks (what the import reads and the layout chooses from).
pub fn full(p: &Project, net: &Net, id: u32) -> Result<Datasheet, String> {
    let us = p.units == "US";
    let mut d = sheet(p, net, id, us)?;
    let si = if us { sheet(p, net, id, false)? } else { d.clone() };
    let mine: Vec<&Issue> = p.issues.iter().filter(|i| i.obj == d.obj).collect();
    if let Some(last) = mine.last() {
        let now = values(&si);
        for (s, ss) in d.sections.iter_mut().zip(&si.sections) {
            for (r, rs) in s.rows.iter_mut().zip(&ss.rows) {
                let k = key(&ss.title, &rs.label);
                let cur = now.get(&k).cloned().unwrap_or_default();
                match last.values.get(&k) {
                    Some(w) if *w != cur => (r.changed, r.was) = (true, if w.is_empty() { "blank".into() } else { w.clone() }),
                    None if !cur.is_empty() => (r.changed, r.was) = (true, "not on the datasheet".into()),
                    _ => {}
                }
            }
        }
        d.changed = d.sections.iter().flat_map(|s| &s.rows).filter(|r| r.changed).count();
        (d.rev, d.status, d.date, d.by) = (last.rev.clone(), last.status.clone(), last.date.clone(), last.by.clone());
    }
    d.issues = mine.iter().map(|i| IssueInfo { rev: i.rev.clone(), status: i.status.clone(), date: i.date.clone(), by: i.by.clone(), note: i.note.clone() }).collect();
    d.next_rev = STATUSES.iter().map(|s| (s.to_string(), next_rev(p, d.obj, s))).collect();
    Ok(d)
}

fn key(section: &str, label: &str) -> String {
    format!("{section} › {label}")
}

/// What a datasheet says, row by row ("Section › Label" → "value unit").
fn values(d: &Datasheet) -> BTreeMap<String, String> {
    d.sections.iter().flat_map(|s| s.rows.iter().map(move |r| (key(&s.title, &r.label), format!("{} {}", r.value, r.unit).trim().to_string()))).collect()
}

/// What an object's datasheet says now, in SI display units: what an issue keeps.
pub fn snapshot(p: &Project, id: u32) -> Result<BTreeMap<String, String>, String> {
    Ok(values(&sheet(p, &Net::new(p), id, false)?))
}

/// The datasheet's rows, in SI or US units.
fn sheet(p: &Project, net: &Net, id: u32, us: bool) -> Result<Datasheet, String> {
    let obj = p.obj_id(id).and_then(|o| p.obj(o)).ok_or("That object no longer exists.")?;
    let it = p.items.iter().find(|i| i.obj == obj.id).ok_or("Draw the object on a sheet first: its datasheet reads its connections.")?;
    let sym = symbols::get(&it.sym).ok_or("There is no such symbol.")?;
    if !has_sheet(sym.cat) {
        return Err(format!("A {} has no datasheet; equipment, instruments, control valves and relief devices do.", sym.name.to_lowercase()));
    }
    if sym.cat.equipment() {
        return equipment(p, net, obj, it, sym, us);
    }
    let (letters, lp) = tags::loop_of(&obj.tag);
    let class_label = classes::get(&obj.class).map_or(obj.class.clone(), |c| c.label.clone());
    let sheet = p.sheet(it.sheet).map(|s| s.number.clone()).unwrap_or_default();
    let source = process_source(p, net, it);
    let line_no = source.as_ref().filter(|(o, _)| p.obj(*o).is_some_and(|x| x.class == crate::model::LINE_CLASS)).map(|(o, _)| crate::graph::obj_number(p, *o)).unwrap_or_default();
    // General.
    let mut general = vec![
        derived("Tag", obj.tag.clone(), String::new(), ""),
        derived("Type", format!("{class_label} · {}", sym.name), String::new(), "class"),
        derived("Measured variable", variable(&letters).into(), String::new(), "tag letters"),
        derived("Loop", lp.clone(), String::new(), "tag"),
        derived("P&ID", sheet, String::new(), "drawing"),
        derived("Area", p.node_path(obj.node), String::new(), "plant"),
        derived("Line", line_no.clone(), String::new(), "drawing"),
    ];
    if let Some((o, name)) = source.as_ref().filter(|_| line_no.is_empty()) {
        if p.obj(*o).is_some_and(|x| x.class != crate::model::LINE_CLASS) {
            general.push(derived("Mounted on", name.clone(), String::new(), "drawing"));
        }
    }
    general.retain(|r| !r.value.is_empty() || r.label == "Line");
    // Process conditions from the line.
    let mut process = vec![];
    if let Some((lo, src)) = &source {
        let d = p.data_of(*lo);
        for (k, label, q) in [("service", "Fluid / service", ""), ("fluid", "Fluid code", ""), ("phase", "Phase", ""), ("flow", "Flow, normal", "flow"), ("mflow", "Mass flow, normal", "mflow"), ("density", "Density", "density"), ("viscosity", "Viscosity", "visc"), ("oper_p", "Operating pressure", "press"), ("oper_t", "Operating temperature", "temp"), ("design_p", "Design pressure", "press"), ("design_t", "Design temperature", "temp"), ("size", "Line size (NPS)", ""), ("sched", "Schedule", ""), ("class", "Pipe class", "")] {
            let (v, u) = show(d.get(k), q, us);
            if !v.is_empty() {
                process.push(derived(label, v, u, src));
            }
        }
    }
    // The class's attributes, by group: the class sections, then Specification, then the project's.
    let attrs = classes::attrs(Some(p), &obj.class);
    let row = |a: &Attr| {
        let (value, unit) = show(obj.data.get(&a.key), &a.q, us);
        Row { key: a.key.clone(), label: a.label.clone(), value, unit, q: a.q.clone(), choices: a.choices.clone(), required: a.required, source: String::new(), changed: false, was: String::new() }
    };
    let own_title = match sym.cat { Cat::Control => "Valve", Cat::Primary => "Element", _ => "Instrument" };
    let user: std::collections::HashSet<String> = p.settings.user_attrs.iter().map(|u| u.attr.key.clone()).collect();
    let own: Vec<Row> = attrs.iter().filter(|a| a.group.is_empty() && !user.contains(&a.key) && a.key != "service").map(row).collect();
    let spec: Vec<Row> = attrs.iter().filter(|a| a.group == "Specification" && !user.contains(&a.key)).map(row).collect();
    let company: Vec<Row> = attrs.iter().filter(|a| user.contains(&a.key)).map(row).collect();
    let mut sections = vec![Section { title: "General".into(), rows: general }];
    sections.push(Section { title: "Process conditions".into(), rows: if process.is_empty() { vec![derived("Process line", "not connected".into(), String::new(), "drawing")] } else { process } });
    sections.push(Section { title: own_title.into(), rows: own });
    sections.extend(groups(&attrs, &user, row));
    sections.push(Section { title: "Specification".into(), rows: spec });
    if !company.is_empty() {
        sections.push(Section { title: "Company attributes".into(), rows: company });
    }
    let title = format!("{} datasheet", if sym.cat == Cat::Control { "Valve" } else { "Instrument" });
    Ok(Datasheet { obj: obj.id, item: it.id, sheet: it.sheet, tag: obj.tag.clone(), title, sections, rev: String::new(), status: String::new(), date: String::new(), by: String::new(), issues: vec![], changed: 0, next_rev: BTreeMap::new(), form: String::new(), logo: String::new(), footer: String::new() })
}

/// Sections for the class's own groups (Sizing, Construction, Shell side…), in the order they first appear.
fn groups(attrs: &[Attr], user: &std::collections::HashSet<String>, row: impl Fn(&Attr) -> Row) -> Vec<Section> {
    let mut out: Vec<Section> = vec![];
    for a in attrs.iter().filter(|a| !a.group.is_empty() && a.group != "Specification" && !user.contains(&a.key)) {
        match out.iter_mut().find(|s| s.title == a.group) {
            Some(s) => s.rows.push(row(a)),
            None => out.push(Section { title: a.group.clone(), rows: vec![row(a)] }),
        }
    }
    out
}

/// "Suction" from "suction".
fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

/// An equipment datasheet: General · Nozzles (each process nozzle's line and its conditions,
/// from the drawing) · Operating (a machine's differential from its suction and discharge lines)
/// · the class's attributes and groups · Specification · the project's own attributes.
fn equipment(p: &Project, net: &Net, obj: &crate::model::Obj, it: &Item, sym: &symbols::Sym, us: bool) -> Result<Datasheet, String> {
    let class_label = classes::get(&obj.class).map_or(obj.class.clone(), |c| c.label.clone());
    let sheet = p.sheet(it.sheet).map(|s| s.number.clone()).unwrap_or_default();
    let mut general = vec![
        derived("Tag", obj.tag.clone(), String::new(), ""),
        derived("Type", format!("{class_label} · {}", sym.name), String::new(), "class"),
        derived("P&ID", sheet, String::new(), "drawing"),
        derived("Area", p.node_path(obj.node), String::new(), "plant"),
    ];
    general.retain(|r| !r.value.is_empty());
    // Each process nozzle on any sheet showing the object: its line and the line's conditions.
    let mut nozzles = vec![];
    let mut at: BTreeMap<String, &crate::model::Data> = BTreeMap::new();
    for i in p.items.iter().filter(|i| i.obj == obj.id) {
        let Some(s) = symbols::get(&i.sym) else { continue };
        for (k, port) in s.ports.iter().enumerate().filter(|(_, x)| !x.signal) {
            for &(l, _) in net.on_port(i.id, k as u8) {
                let Some(line) = net.lines.get(&l).filter(|x| x.kind.process() && x.obj != 0) else { continue };
                if at.contains_key(port.name) || instrument_connection(p, line) {
                    continue;
                }
                let d = p.data_of(line.obj);
                at.insert(port.name.to_string(), d);
                let mut parts = vec![crate::graph::obj_number(p, line.obj)];
                for (key, q) in [("flow", "flow"), ("mflow", "mflow"), ("oper_p", "press"), ("oper_t", "temp")] {
                    let (v, u) = show(d.get(key), q, us);
                    if !v.is_empty() {
                        parts.push(format!("{v} {u}"));
                    }
                }
                nozzles.push(derived(&cap(port.name), parts.join(" · "), String::new(), "drawing"));
            }
        }
    }
    // A machine: the differential pressure and head its lines give.
    let mut operating = vec![];
    let num = |d: &crate::model::Data, k: &str| d.get(k).and_then(|v| v.num()).filter(|x| x.is_finite());
    let pair = at.get("suction").zip(at.get("discharge")).or_else(|| if sym.cat == Cat::Rotating { at.get("inlet").zip(at.get("outlet")) } else { None });
    if let Some((s, d)) = pair {
        if let (Some(ps), Some(pd)) = (num(s, "oper_p"), num(d, "oper_p")) {
            let (v, u) = show(Some(&Val::Num(pd - ps)), "dp", us);
            operating.push(derived("Differential pressure", v, u, "lines"));
            if let Some(rho) = num(s, "density").filter(|r| *r > 0.0) {
                let (v, u) = show(Some(&Val::Num((pd - ps) / (rho * 9.80665))), "head", us);
                operating.push(derived("Differential head", v, u, "lines"));
            }
        }
    }
    let attrs = classes::attrs(Some(p), &obj.class);
    let row = |a: &Attr| {
        let (value, unit) = show(obj.data.get(&a.key), &a.q, us);
        Row { key: a.key.clone(), label: a.label.clone(), value, unit, q: a.q.clone(), choices: a.choices.clone(), required: a.required, source: String::new(), changed: false, was: String::new() }
    };
    let user: std::collections::HashSet<String> = p.settings.user_attrs.iter().map(|u| u.attr.key.clone()).collect();
    let own: Vec<Row> = attrs.iter().filter(|a| a.group.is_empty() && !user.contains(&a.key)).map(row).collect();
    let spec: Vec<Row> = attrs.iter().filter(|a| a.group == "Specification" && !user.contains(&a.key)).map(row).collect();
    let company: Vec<Row> = attrs.iter().filter(|a| user.contains(&a.key)).map(row).collect();
    let chain = classes::chain(&obj.class);
    let is = |n: &str| chain.iter().any(|c| c.name == n);
    let kind = if is("Pump") { "Pump" } else if is("Compressor") { "Compressor" } else if is("HeatExchanger") || is("Heater") { "Heat exchanger" } else if is("Vessel") { "Vessel" } else { "Equipment" };
    let mut sections = vec![Section { title: "General".into(), rows: general }];
    sections.push(Section { title: "Nozzles".into(), rows: if nozzles.is_empty() { vec![derived("Process lines", "not connected".into(), String::new(), "drawing")] } else { nozzles } });
    let mut own = own;
    own.splice(0..0, operating);
    sections.push(Section { title: "Operating and design".into(), rows: own });
    sections.extend(groups(&attrs, &user, row));
    sections.push(Section { title: "Specification".into(), rows: spec });
    if !company.is_empty() {
        sections.push(Section { title: "Company attributes".into(), rows: company });
    }
    Ok(Datasheet { obj: obj.id, item: it.id, sheet: it.sheet, tag: obj.tag.clone(), title: format!("{kind} datasheet"), sections, rev: String::new(), status: String::new(), date: String::new(), by: String::new(), issues: vec![], changed: 0, next_rev: BTreeMap::new(), form: String::new(), logo: String::new(), footer: String::new() })
}

/// Every datasheet with every row (the layout's hidden rows too), for the import.
pub fn all_full(p: &Project, net: &Net) -> Vec<Datasheet> {
    let mut seen = std::collections::HashSet::new();
    p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| has_sheet(s.cat)) && seen.insert(i.obj)).filter_map(|i| full(p, net, i.obj).ok()).collect()
}

/// Every datasheet in the project, in tag order.
pub fn all(p: &Project) -> Vec<Datasheet> {
    let net = Net::new(p);
    let mut seen = std::collections::HashSet::new();
    let mut v: Vec<Datasheet> = p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| has_sheet(s.cat)) && seen.insert(i.obj)).filter_map(|i| build_in(p, &net, i.obj).ok()).collect();
    v.sort_by_key(|d| { let (a, n, _) = tags::split(&d.tag); (a, n, d.tag.clone()) });
    v
}

/// Every datasheet as an Excel workbook: one worksheet per datasheet, plus an index.
pub fn xlsx(p: &Project) -> Result<Vec<u8>, String> {
    use rust_xlsxwriter::{Format, Workbook};
    let e = |x: rust_xlsxwriter::XlsxError| x.to_string();
    let sheets = all(p);
    if sheets.is_empty() {
        return Err("There are no instruments, control valves or relief devices to make datasheets for.".into());
    }
    let mut wb = Workbook::new();
    let bold = Format::new().set_bold();
    let head = Format::new().set_bold().set_background_color(0xF4DCEE);
    let derived_fmt = Format::new().set_font_color(0x5B6878);
    {
        let ix = wb.add_worksheet();
        ix.set_name("Index").map_err(e)?;
        ix.write_string_with_format(0, 0, format!("Datasheets · {}", if p.title.project.is_empty() { &p.name } else { &p.title.project }), &bold).map_err(e)?;
        for (c, h) in ["Tag", "Datasheet", "Rev", "Status", "Date", "Changed since issue"].iter().enumerate() {
            ix.write_string_with_format(2, c as u16, *h, &head).map_err(e)?;
        }
        for (r, d) in sheets.iter().enumerate() {
            let r = r as u32 + 3;
            ix.write_string(r, 0, &d.tag).map_err(e)?;
            ix.write_string(r, 1, &d.title).map_err(e)?;
            ix.write_string(r, 2, &d.rev).map_err(e)?;
            ix.write_string(r, 3, if d.status.is_empty() { "Not issued" } else { &d.status }).map_err(e)?;
            ix.write_string(r, 4, &d.date).map_err(e)?;
            if !d.rev.is_empty() {
                ix.write_string(r, 5, if d.changed == 0 { "none".to_string() } else { format!("Δ {}", d.changed) }).map_err(e)?;
            }
        }
        ix.set_column_width(0, 18).map_err(e)?;
        ix.set_column_width(1, 22).map_err(e)?;
        ix.set_column_width(3, 22).map_err(e)?;
    }
    let logo = p.settings.ds_layout.logo.split_once(";base64,").filter(|(h, _)| *h == "data:image/png" || *h == "data:image/jpeg").and_then(|(_, b)| crate::base64_decode(b).ok());
    let mut names = std::collections::HashSet::new();
    for d in &sheets {
        let ws = wb.add_worksheet();
        let mut name: String = d.tag.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(28).collect();
        if name.is_empty() {
            name = "Datasheet".into();
        }
        let mut k = 2;
        let base = name.clone();
        while !names.insert(name.to_lowercase()) {
            name = format!("{base} {k}");
            k += 1;
        }
        ws.set_name(&name).map_err(e)?;
        ws.write_string_with_format(0, 0, format!("{} · {}", d.title, d.tag), &bold).map_err(e)?;
        ws.write_string(1, 0, format!("{} · {}", p.title.company, if p.title.project.is_empty() { &p.name } else { &p.title.project })).map_err(e)?;
        if !d.form.is_empty() {
            ws.write_string(0, 3, format!("Form {}", d.form)).map_err(e)?;
        }
        // The company logo (PNG or JPEG; Excel takes no SVG), in the top right corner.
        if let Some(bytes) = logo.as_ref() {
            if let Ok(img) = rust_xlsxwriter::Image::new_from_buffer(bytes) {
                ws.insert_image(0, 4, &img.set_scale_to_size(160, 40, true)).map_err(e)?;
            }
        }
        ws.write_string(1, 3, if d.rev.is_empty() { "Not issued".to_string() } else { format!("Rev {} · {} · {}", d.rev, d.status, d.date) }).map_err(e)?;
        let mut r = 3u32;
        for s in &d.sections {
            ws.write_string_with_format(r, 0, &s.title, &head).map_err(e)?;
            ws.write_string_with_format(r, 1, "", &head).map_err(e)?;
            ws.write_string_with_format(r, 2, "", &head).map_err(e)?;
            ws.write_string_with_format(r, 3, "", &head).map_err(e)?;
            ws.write_string_with_format(r, 4, "", &head).map_err(e)?;
            r += 1;
            for row in &s.rows {
                ws.write_string(r, 0, &row.label).map_err(e)?;
                match row.value.parse::<f64>() {
                    Ok(v) if !row.unit.is_empty() && v.is_finite() => ws.write_number(r, 1, v).map_err(e)?,
                    _ => ws.write_string(r, 1, &row.value).map_err(e)?,
                };
                ws.write_string(r, 2, &row.unit).map_err(e)?;
                if !row.source.is_empty() {
                    ws.write_string_with_format(r, 3, format!("from {}", row.source), &derived_fmt).map_err(e)?;
                }
                if row.changed {
                    ws.write_string_with_format(r, 4, format!("Δ was {}", row.was), &bold).map_err(e)?;
                }
                r += 1;
            }
            r += 1;
        }
        if !d.issues.is_empty() {
            ws.write_string_with_format(r, 0, "Revisions", &head).map_err(e)?;
            for c in 1..5 {
                ws.write_string_with_format(r, c, "", &head).map_err(e)?;
            }
            r += 1;
            for i in d.issues.iter().rev() {
                ws.write_string(r, 0, format!("{} · {}", i.rev, i.status)).map_err(e)?;
                ws.write_string(r, 1, &i.date).map_err(e)?;
                ws.write_string(r, 2, &i.by).map_err(e)?;
                ws.write_string(r, 3, &i.note).map_err(e)?;
                r += 1;
            }
        }
        if !d.footer.is_empty() {
            ws.write_string(r + 1, 0, &d.footer).map_err(e)?;
        }
        ws.set_column_width(0, 32).map_err(e)?;
        ws.set_column_width(1, 26).map_err(e)?;
        ws.set_column_width(3, 30).map_err(e)?;
        ws.set_column_width(4, 28).map_err(e)?;
    }
    wb.save_to_buffer().map_err(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instrument_datasheets_read_their_line() {
        let p = crate::examples::get("pump").unwrap();
        let lt = p.objects.iter().find(|o| o.tag == "LT-101").unwrap();
        let d = build(&p, lt.id).unwrap();
        let sec = |t: &str| d.sections.iter().find(|s| s.title == t).unwrap();
        let get = |s: &Section, l: &str| s.rows.iter().find(|r| r.label == l).map(|r| r.value.clone()).unwrap_or_default();
        assert_eq!(get(sec("General"), "Measured variable"), "Level");
        assert_eq!(get(sec("General"), "Loop"), "101");
        assert_eq!(get(sec("Instrument"), "Range"), "0–100 %");
        // The pressure gauge on the discharge reads the discharge line's conditions.
        let pi = p.objects.iter().find(|o| o.tag == "PI-102").unwrap();
        let d = build(&p, pi.id).unwrap();
        let pc = d.sections.iter().find(|s| s.title == "Process conditions").unwrap();
        assert!(pc.rows.iter().any(|r| r.label == "Operating pressure" && r.value == "4.5" && r.unit == "bar" && r.source.contains("P-1003")), "{:?}", pc.rows);
        // The level transmitter is tapped to the vessel: the vessel's conditions; the controller
        // takes its loop's (LT-101's), not those of PVSV-101, which shares only the number.
        for tag in ["LT-101", "LIC-101"] {
            let o = p.objects.iter().find(|o| o.tag == tag).unwrap();
            let d = build(&p, o.id).unwrap();
            let pc = d.sections.iter().find(|s| s.title == "Process conditions").unwrap();
            assert!(!pc.rows.is_empty() && pc.rows.iter().all(|r| r.source == "V-101"), "{tag}: {:?}", pc.rows);
            assert!(d.sections[0].rows.iter().any(|r| r.label == "Mounted on" && r.value == "V-101"));
        }
        // The control valve has valve sections and its sizing data.
        let lv = p.objects.iter().find(|o| o.tag == "LV-101").unwrap();
        let d = build(&p, lv.id).unwrap();
        assert!(d.sections.iter().any(|s| s.title == "Valve" && s.rows.iter().any(|r| r.key == "fail" && r.value == "FC")));
        assert!(d.sections.iter().any(|s| s.title == "Specification" && s.rows.iter().any(|r| r.key == "characteristic")));
        assert!(d.sections.iter().any(|s| s.title == "Sizing" && s.rows.iter().any(|r| r.key == "dp_design" && r.value == "80")));
        // A pump's datasheet: its nozzles' lines and conditions, its differential from them.
        let pump = p.objects.iter().find(|o| o.tag == "P-101A").unwrap();
        let d = build(&p, pump.id).unwrap();
        assert_eq!(d.title, "Pump datasheet");
        let nz = d.sections.iter().find(|s| s.title == "Nozzles").unwrap();
        assert!(nz.rows.iter().any(|r| r.label == "Suction" && r.value.contains("P-1001")) && nz.rows.iter().any(|r| r.label == "Discharge"), "{:?}", nz.rows);
        let op = d.sections.iter().find(|s| s.title == "Operating and design").unwrap();
        assert!(op.rows.iter().any(|r| r.label == "Differential pressure" && r.source == "lines"), "{:?}", op.rows);
        // A vessel and a gate valve.
        let v = p.objects.iter().find(|o| o.tag == "V-101").unwrap();
        assert_eq!(build(&p, v.id).unwrap().title, "Vessel datasheet");
        let gate = p.items.iter().find(|i| i.base_key() == "gate").unwrap();
        assert!(build(&p, gate.id).is_err());
        assert!(all(&p).len() >= 5);
        assert_eq!(&xlsx(&p).unwrap()[..2], b"PK");
    }

    #[test]
    fn company_attributes_get_their_own_section() {
        let mut p = crate::examples::get("pump").unwrap();
        p.settings.user_attrs.push(crate::classes::UserAttr { class: "Instrument".into(), attr: Attr { key: "sil".into(), label: "SIL rating".into(), choices: vec!["1".into(), "2".into(), "3".into()], ..Default::default() } });
        let lt = p.objects.iter().find(|o| o.tag == "LT-101").unwrap().id;
        let d = build(&p, lt).unwrap();
        assert!(d.sections.iter().any(|s| s.title == "Company attributes" && s.rows.iter().any(|r| r.key == "sil")));
    }

    #[test]
    fn revisions_run_by_status() {
        let mut p = crate::examples::get("pump").unwrap();
        let o = p.objects.iter().find(|o| o.tag == "LT-101").unwrap().id;
        assert_eq!(next_rev(&p, o, "Preliminary"), "A");
        assert_eq!(next_rev(&p, o, "Issued for Purchase"), "0");
        let issue = |rev: &str, status: &str| Issue { obj: o, rev: rev.into(), status: status.into(), ..Default::default() };
        p.issues = vec![issue("A", "Preliminary"), issue("B", "Issued for Enquiry")];
        assert_eq!(next_rev(&p, o, "Issued for Enquiry"), "C");
        assert_eq!(next_rev(&p, o, "Issued for Purchase"), "0");
        p.issues.push(issue("0", "Issued for Purchase"));
        assert_eq!(next_rev(&p, o, "As-built"), "1");
        p.issues = vec![issue("Z", "Preliminary")];
        assert_eq!(next_rev(&p, o, "Preliminary"), "AA");
        p.issues = vec![issue("AZ", "Preliminary")];
        assert_eq!(next_rev(&p, o, "Preliminary"), "BA");
    }

    #[test]
    fn issued_datasheets_mark_what_changed() {
        use crate::doc::{Doc, Op};
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        d.user = "E. M.".into();
        let id = |d: &Doc, t: &str| d.p.objects.iter().find(|o| o.tag == t).unwrap().id;
        let (lt, pi) = (id(&d, "LT-101"), id(&d, "PI-102"));
        let issue = |d: &mut Doc, ids: Vec<u32>, rev: &str, status: &str| d.apply(Op::IssueDatasheet { ids, rev: rev.into(), status: status.into(), date: "2026-10-30".into(), note: "For comment".into(), by: String::new() });
        issue(&mut d, vec![lt, pi], "", "Preliminary").unwrap();
        let ds = build(&d.p, lt).unwrap();
        assert_eq!((ds.rev.as_str(), ds.status.as_str(), ds.by.as_str(), ds.changed), ("A", "Preliminary", "E. M.", 0));
        assert_eq!(ds.next_rev["Issued for Enquiry"], "B");
        assert!(d.history.iter().any(|c| c.obj == lt && c.what == "issue" && c.new == "A · Preliminary"));
        // The object's own value and a value on the line both show as changed.
        d.apply(Op::SetData { ids: vec![lt], key: "manufacturer".into(), val: Some(Val::Text("Acme".into())) }).unwrap();
        let ds = build(&d.p, lt).unwrap();
        let m = ds.sections.iter().flat_map(|s| &s.rows).find(|r| r.key == "manufacturer").unwrap();
        assert!(m.changed && m.was == "blank" && ds.changed == 1, "{m:?}");
        let line = d.p.objects.iter().find(|o| o.class == crate::model::LINE_CLASS && crate::graph::obj_number(&d.p, o.id).contains("P-1003")).unwrap().id;
        d.apply(Op::SetData { ids: vec![line], key: "oper_p".into(), val: Some(Val::Num(6.0e5)) }).unwrap();
        let ds = build(&d.p, pi).unwrap();
        let r = ds.sections.iter().flat_map(|s| &s.rows).find(|r| r.label == "Operating pressure").unwrap();
        assert!(r.changed && r.was == "4.5 bar", "{r:?}");
        // Showing US units changes nothing on the datasheet's content.
        d.apply(Op::SetProject { name: d.p.name.clone(), units: "US".into() }).unwrap();
        assert_eq!(build(&d.p, lt).unwrap().changed, 1);
        // The next issue clears the marks; a revision name is used once per datasheet.
        issue(&mut d, vec![lt], "", "Issued for Enquiry").unwrap();
        assert_eq!(build(&d.p, lt).unwrap().rev, "B");
        assert_eq!(build(&d.p, lt).unwrap().changed, 0);
        assert!(issue(&mut d, vec![lt], "b", "Issued for Purchase").unwrap_err().contains("already issued"));
        assert!(issue(&mut d, vec![lt], "", "Approved").is_err());
        issue(&mut d, vec![lt], "", "Issued for Purchase").unwrap();
        let ds = build(&d.p, lt).unwrap();
        assert_eq!(ds.issues.iter().map(|i| i.rev.as_str()).collect::<Vec<_>>(), ["A", "B", "0"]);
        // The index lists them; undo withdraws an issue and the history says so.
        let ix = crate::lists::build(&d.p, "datasheets").unwrap();
        let row = ix.rows.iter().find(|r| r.cells[0] == "LT-101").unwrap();
        assert_eq!(&row.cells[2..5], ["0", "Issued for Purchase", "2026-10-30"]);
        d.undo();
        assert_eq!(build(&d.p, lt).unwrap().rev, "B");
        assert!(d.history.iter().any(|c| c.what == "undo withdraw" && c.old == "0 · Issued for Purchase"));
        // Saved and opened again, the issues are kept.
        let path = std::env::temp_dir().join(format!("ds-issues-{}.dschema", std::process::id()));
        d.save(&path).unwrap();
        let (e, w) = Doc::open(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(e.p.issues, d.p.issues);
        // Deleting the object takes its issues with it.
        d.apply(Op::DeleteObjects { ids: vec![lt] }).unwrap();
        assert!(d.p.issues.iter().all(|i| i.obj != lt) && d.p.issues.iter().any(|i| i.obj == pi));
        assert_eq!(&xlsx(&d.p).unwrap()[..2], b"PK");
    }

    #[test]
    fn the_company_layout_hides_rows_and_orders_sections() {
        let mut p = crate::examples::get("pump").unwrap();
        let lt = p.objects.iter().find(|o| o.tag == "LT-101").unwrap().id;
        let rows = layout_rows(&p);
        assert!(rows.iter().any(|(s, ls)| s == "Specification" && ls.iter().any(|l| l == "Manufacturer")), "{rows:?}");
        p.settings.ds_layout = crate::model::DsLayout { form: "IDS-001".into(), footer: "Datum Process Ltd".into(), hidden: vec!["Specification › Manufacturer".into(), "General › Area".into()], order: vec!["Specification".into(), "Process conditions".into()], logo: String::new() };
        let d = build(&p, lt).unwrap();
        let titles: Vec<&str> = d.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(&titles[..3], ["Specification", "Process conditions", "General"]);
        let all: Vec<&str> = d.sections.iter().flat_map(|s| s.rows.iter().map(|r| r.label.as_str())).collect();
        assert!(!all.contains(&"Manufacturer") && !all.contains(&"Area") && all.contains(&"Model"));
        assert_eq!((d.form.as_str(), d.footer.as_str()), ("IDS-001", "Datum Process Ltd"));
        // A hidden row changed since an issue does not count as a visible change.
        p.issues.push(crate::model::Issue { obj: lt, rev: "A".into(), status: "Preliminary".into(), values: snapshot(&p, lt).unwrap(), ..Default::default() });
        p.obj_mut(lt).unwrap().data.insert("manufacturer".into(), Val::Text("Acme".into()));
        assert_eq!(build(&p, lt).unwrap().changed, 0);
        assert_eq!(full(&p, &Net::new(&p), lt).unwrap().changed, 1);
        // A PNG logo goes into every datasheet's worksheet.
        p.settings.ds_layout.logo = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==".into();
        assert!(p.clone().normalize().is_empty());
        let book = xlsx(&p).unwrap();
        assert!(book.windows(19).any(|w| w == b"xl/media/image1.png"), "the logo is in the workbook");
        // A logo that is not an image is refused.
        p.settings.ds_layout.logo = "javascript:alert(1)".into();
        assert!(p.normalize().iter().any(|w| w.contains("logo")) && p.settings.ds_layout.logo.is_empty());
        assert_eq!(&xlsx(&p).unwrap()[..2], b"PK");
    }
}
