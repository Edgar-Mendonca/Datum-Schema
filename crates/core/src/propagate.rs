//! Values passed along a line: a valve's or fitting's size and pipe class follow the line it sits
//! in (the class into "pipe_class": a valve's own "class" is its rating). Control and relief
//! valves, reducers, vents and drains are sized on their own. The value last written is
//! remembered (data key "~size", "~pipe_class"), so the engine can tell
//! its own value from one typed by the user: a typed value is kept, and if it differs from the
//! line it goes on the to-do list. Also the to-do list itself: what needs looking at after a change.

use crate::graph::Net;
use crate::model::{Project, Val};
use crate::symbols;
use serde::Serialize;

/// The keys a component takes from its line: its size, and its pipe class (from the line's "class").
pub const KEYS: [&str; 2] = ["size", "pipe_class"];

/// Whether a data key is the engine's own bookkeeping (never shown, listed or recorded).
pub fn hidden(k: &str) -> bool {
    k.starts_with('~')
}

/// The values a component's line gives it: the largest size of the lines it sits in (a vent,
/// drain or sample point takes no size: it is a small branch) and the first pipe class.
fn from_line(p: &Project, net: &Net, it: &crate::model::Item) -> [Option<String>; 2] {
    let txt = |d: &crate::model::Data, k: &str| d.get(k).map(|v| v.text().trim().to_string()).filter(|s| !s.is_empty());
    let Some(s) = symbols::get(&it.sym) else { return [None, None] };
    let (mut size, mut class): (Option<String>, Option<String>) = (None, None);
    for k in (0..s.ports.len()).filter(|k| !s.ports[*k].signal) {
        for &(l, _) in net.on_port(it.id, k as u8) {
            let Some(line) = net.lines.get(&l).filter(|x| x.kind.process() && x.obj != 0) else { continue };
            let d = p.data_of(line.obj);
            if let Some(sz) = txt(d, "size") {
                let bigger = size.as_ref().is_none_or(|cur| crate::pipes::nps_inches(&sz).unwrap_or(0.0) > crate::pipes::nps_inches(cur).unwrap_or(0.0));
                if bigger {
                    size = Some(sz);
                }
            }
            if class.is_none() {
                class = txt(d, "class");
            }
        }
    }
    // Sized on their own: control and relief valves (often below line size), reducers (two
    // sizes), vents, drains and sample points (small branches).
    if s.terminal || s.cat == crate::symbols::Cat::Control || it.base_key().starts_with("reducer") {
        size = None;
    }
    [size, class]
}

/// Writes each component's line values where the component has not been given its own.
pub fn run(p: &mut Project) {
    let pfd: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    let mut todo: Vec<(u32, [Option<String>; 2])> = vec![];
    {
        let net = Net::new(p);
        let mut seen = std::collections::HashSet::new();
        for it in p.items.iter().filter(|i| i.obj != 0 && !pfd.contains(&i.sheet)) {
            if !symbols::get(&it.sym).is_some_and(crate::specs::is_piping) || !seen.insert(it.obj) {
                continue;
            }
            todo.push((it.obj, from_line(p, &net, it)));
        }
    }
    for (obj, vals) in todo {
        let Some(o) = p.obj_mut(obj) else { continue };
        for (k, v) in KEYS.iter().zip(vals) {
            let Some(v) = v else { continue };
            let mark = format!("~{k}");
            let cur = o.data.get(*k).map(|x| x.text());
            let last = o.data.get(&mark).map(|x| x.text());
            // Ours to change: empty, or still the value we wrote last.
            if cur.is_none() || cur == last {
                o.data.insert(k.to_string(), Val::Text(v.clone()));
                o.data.insert(mark, Val::Text(v));
            }
        }
    }
}

/// One thing to look at after a change.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Todo {
    /// "override", "datasheet" or "sheet".
    pub kind: &'static str,
    pub what: String,
    /// The symbols it is about (an override's), else empty.
    pub ids: Vec<u32>,
    pub sheet: u32,
    /// For an override: the key and the line's value, to take it with one click.
    pub key: String,
    pub value: String,
    /// For a sheet: the revision it changed since; for a datasheet: its object.
    pub rev: String,
    pub obj: u32,
}

/// The to-do list: components whose own size or class differs from their line's; datasheets
/// changed since they were issued; sheets changed since their last issued revision.
pub fn todo(p: &Project) -> Vec<Todo> {
    let mut v = vec![];
    let pfd: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    let net = Net::new(p);
    let mut seen = std::collections::HashSet::new();
    for it in p.items.iter().filter(|i| i.obj != 0 && !pfd.contains(&i.sheet)) {
        if !symbols::get(&it.sym).is_some_and(crate::specs::is_piping) || !seen.insert(it.obj) {
            continue;
        }
        let d = p.data_of(it.obj);
        for (k, line) in KEYS.iter().zip(from_line(p, &net, it)) {
            let (Some(line), Some(own)) = (line, d.get(*k).map(|x| x.text())) else { continue };
            let same = match (crate::pipes::nps_inches(&line), crate::pipes::nps_inches(&own)) {
                (Some(a), Some(b)) if *k == "size" => (a - b).abs() < 1e-9,
                _ => line.trim().eq_ignore_ascii_case(own.trim()),
            };
            if !same {
                let tag = p.tag_of(it).trim().to_string();
                let who = if tag.is_empty() { symbols::get(&it.sym).map_or("A component".to_string(), |s| format!("A {}", s.name.to_lowercase())) } else { tag };
                let what = if *k == "size" { "size" } else { "pipe class" };
                v.push(Todo { kind: "override", what: format!("{who}'s {what} is {own}, its line's is {line}."), ids: vec![it.id], sheet: it.sheet, key: k.to_string(), value: line, rev: String::new(), obj: it.obj });
            }
        }
    }
    // Only issued datasheets can have changed since their issue.
    let issued: std::collections::HashSet<u32> = p.issues.iter().map(|i| i.obj).collect();
    let mut shown = std::collections::HashSet::new();
    let sheets: Vec<crate::datasheets::Datasheet> = p.items.iter().filter(|i| issued.contains(&i.obj) && shown.insert(i.obj)).filter_map(|i| crate::datasheets::build_in(p, &net, i.obj).ok()).collect();
    for ds in sheets.into_iter().filter(|d| !d.rev.is_empty() && d.changed > 0) {
        v.push(Todo { kind: "datasheet", what: format!("{}'s datasheet has {} change{} since revision {}: issue it again.", ds.tag, ds.changed, if ds.changed == 1 { "" } else { "s" }, ds.rev), ids: vec![ds.item], sheet: ds.sheet, key: String::new(), value: String::new(), rev: ds.rev.clone(), obj: ds.obj });
    }
    for s in &p.sheets {
        let Some(last) = s.snaps.last() else { continue };
        let Ok(c) = crate::revs::compare(p, s, &last.rev) else { continue };
        if !c.changes.is_empty() {
            v.push(Todo { kind: "sheet", what: format!("{} has {} change{} since revision {}: compare, cloud and issue it.", s.number, c.changes.len(), if c.changes.len() == 1 { "" } else { "s" }, last.rev), ids: vec![], sheet: s.id, key: String::new(), value: String::new(), rev: last.rev.clone(), obj: 0 });
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    #[test]
    fn line_values_pass_to_components_and_overrides_are_listed() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let tag = |d: &Doc, t: &str| d.p.items.iter().find(|i| d.p.tag_of(i) == t).unwrap().clone();
        let get = |d: &Doc, t: &str, k: &str| d.p.data_of(tag(d, t).obj).get(k).map(|v| v.text()).unwrap_or_default();
        // The example's valves carry their lines' size and class.
        assert_eq!((get(&d, "HV-104", "size").as_str(), get(&d, "HV-104", "pipe_class").as_str()), ("3", "A1A"));
        // A control valve keeps its own size; its rating stays its rating.
        assert_eq!((get(&d, "LV-101", "class").as_str(), get(&d, "LV-101", "pipe_class").as_str()), ("300#", "A1A"));
        assert_eq!(get(&d, "HV-101", "size"), "4");
        assert!(todo(&d.p).is_empty(), "{:?}", todo(&d.p));
        // The line grows: its valves follow.
        let hv = tag(&d, "HV-104");
        let line = d.p.lines.iter().find(|l| l.a.item().map(|x| x.0) == Some(hv.id) || l.b.item().map(|x| x.0) == Some(hv.id)).unwrap().id;
        d.apply(Op::SetData { ids: vec![line], key: "size".into(), val: Some(Val::Text("4".into())) }).unwrap();
        assert_eq!(get(&d, "HV-104", "size"), "4");
        // A size typed on the valve is kept, and listed while it differs from the line.
        d.apply(Op::SetData { ids: vec![hv.id], key: "size".into(), val: Some(Val::Text("3".into())) }).unwrap();
        assert_eq!(get(&d, "HV-104", "size"), "3");
        let t = todo(&d.p);
        assert!(t.iter().any(|x| x.kind == "override" && x.what.contains("HV-104's size is 3, its line's is 4") && x.value == "4"), "{t:?}");
        // Taking the line's value ends the override: the valve follows the line again.
        d.apply(Op::SetData { ids: vec![hv.id], key: "size".into(), val: Some(Val::Text("4".into())) }).unwrap();
        assert!(!todo(&d.p).iter().any(|x| x.kind == "override"));
        d.apply(Op::SetData { ids: vec![line], key: "size".into(), val: Some(Val::Text("6".into())) }).unwrap();
        assert_eq!(get(&d, "HV-104", "size"), "6");
        // A vent takes no size from its line.
        let vent = d.p.items.iter().find(|i| i.base_key() == "vent").unwrap().obj;
        assert!(d.p.data_of(vent).get("size").is_none());
        // Hidden keys are not recorded in the history.
        assert!(d.history.iter().all(|c| !hidden(&c.key)));
        // A sheet changed since its issue, and a datasheet changed since its issue.
        let s = d.p.sheets[0].id;
        d.apply(Op::IssueRevision { sheet: s, rev: crate::model::Rev { rev: "B".into(), ..Default::default() } }).unwrap();
        assert!(!todo(&d.p).iter().any(|x| x.kind == "sheet"));
        d.apply(Op::Move { ids: vec![hv.id], dx: 5.0, dy: 0.0 }).unwrap();
        assert!(todo(&d.p).iter().any(|x| x.kind == "sheet" && x.rev == "B"));
        let lt = tag(&d, "LT-101");
        d.apply(Op::IssueDatasheet { ids: vec![lt.id], rev: "A".into(), status: "Preliminary".into(), date: String::new(), note: String::new(), by: String::new() }).unwrap();
        d.apply(Op::SetData { ids: vec![lt.id], key: "range".into(), val: Some(Val::Text("0–2500 mm".into())) }).unwrap();
        assert!(todo(&d.p).iter().any(|x| x.kind == "datasheet" && x.what.contains("LT-101")), "{:?}", todo(&d.p));
    }
}
