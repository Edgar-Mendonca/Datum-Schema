//! The live checks (brief §2.4): one function per rule, each returning issues with a level, a short
//! reason, the things it is about and where they are. Rules switched off in the project's settings
//! are skipped.

use crate::graph::{end_pt, Net};
use crate::hydraulics::Hyd;
use crate::model::{End, Item, Line, Project};
use crate::symbols::{self, Cat};
use crate::tags;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Issue {
    pub rule: u8,
    pub level: &'static str,
    pub msg: String,
    pub ids: Vec<u32>,
    pub sheet: u32,
    pub at: [f64; 2],
}

const ERROR: &str = "error";
const WARNING: &str = "warning";
const NOTE: &str = "note";

fn name(it: &Item) -> String {
    if it.tag.trim().is_empty() { symbols::get(&it.sym).map_or("a symbol", |s| s.name).to_lowercase() } else { it.tag.clone() }
}
fn on_item(rule: u8, level: &'static str, it: &Item, msg: String) -> Issue {
    Issue { rule, level, msg, ids: vec![it.id], sheet: it.sheet, at: [it.x, it.y] }
}
fn mid(p: &Project, l: &Line) -> [f64; 2] {
    let (a, b) = (end_pt(p, &l.a), end_pt(p, &l.b));
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}
fn on_lines(rule: u8, level: &'static str, p: &Project, net: &Net, ids: &[u32], msg: String) -> Issue {
    let l = net.lines[&ids[0]];
    Issue { rule, level, msg, ids: ids.to_vec(), sheet: l.sheet, at: mid(p, l) }
}
fn text(l: &Line, k: &str) -> String {
    l.data.get(k).map(|v| v.text().trim().to_string()).unwrap_or_default()
}
fn label(net: &Net, l: u32) -> String {
    let n = net.numbers.get(&l).cloned().unwrap_or_default();
    if n.is_empty() { "a line".into() } else { format!("line {n}") }
}

/// The process lines joined at an in-line item's two process ports (or an off-page pair), with the
/// end of each line that is there. None unless there is exactly one line on each side.
fn sides(net: &Net, it: &Item) -> Option<[(u32, bool); 2]> {
    let s = symbols::get(&it.sym)?;
    let proc = |item: u32, port: u8| -> Vec<(u32, bool)> { net.on_port(item, port).iter().copied().filter(|x| net.lines[&x.0].kind.process()).collect() };
    let (a, b) = if s.inline {
        let ports: Vec<u8> = s.ports.iter().enumerate().filter(|(_, x)| !x.signal).map(|(k, _)| k as u8).collect();
        (proc(it.id, ports[0]), proc(it.id, ports[1]))
    } else if s.cat == Cat::Connector && it.link != 0 && it.id < it.link {
        (proc(it.id, 0), proc(it.link, 0))
    } else {
        return None;
    };
    (a.len() == 1 && b.len() == 1).then(|| [a[0], b[0]])
}

pub fn run(p: &Project, net: &Net, hyd: &[Option<Hyd>]) -> Vec<Issue> {
    let mut v = vec![];
    let on = |n: u8| p.rule_on(n);
    let sym = |it: &Item| symbols::get(&it.sym);

    // 1. Duplicate tags and line numbers.
    if on(1) {
        for (t, ids) in tags::duplicates(p) {
            let it = net.items[&ids[0]];
            v.push(Issue { rule: 1, level: ERROR, msg: format!("Tag {t} is used {} times.", ids.len()), ids, sheet: it.sheet, at: [it.x, it.y] });
        }
        let mut by_number: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (r, lines) in net.runs.iter().enumerate() {
            let n = net.numbers[&lines[0]].as_str();
            if !n.is_empty() && !n.contains('?') {
                by_number.entry(n).or_default().push(r);
            }
        }
        for (n, runs) in by_number.into_iter().filter(|x| x.1.len() > 1) {
            let ids: Vec<u32> = runs.iter().flat_map(|r| net.runs[*r].iter().copied()).collect();
            v.push(on_lines(1, ERROR, p, net, &ids, format!("Line number {n} is used by {} separate lines.", runs.len())));
        }
    }

    // 2. Open ends.
    if on(2) {
        for l in &p.lines {
            for e in [&l.a, &l.b] {
                if let End::Free { x, y } = e {
                    v.push(Issue { rule: 2, level: ERROR, msg: format!("{} has an open end: join it to a symbol, a line, a battery limit, a drain or vent, or an off-page connector.", cap(&label(net, l.id))), ids: vec![l.id], sheet: l.sheet, at: [*x, *y] });
                }
            }
        }
        for it in &p.items {
            let Some(s) = sym(it) else { continue };
            if !s.inline || s.terminal {
                continue;
            }
            let used: Vec<bool> = s.ports.iter().enumerate().filter(|(_, x)| !x.signal).map(|(k, _)| !net.on_port(it.id, k as u8).is_empty()).collect();
            if used.iter().filter(|u| **u).count() == 1 {
                v.push(on_item(2, ERROR, it, format!("{} is joined on one side only: the other side is an open end.", cap(&name(it)))));
            }
        }
    }

    // 3. Off-page connectors: a partner on another sheet, carrying the same line number.
    if on(3) {
        for id in crate::graph::lonely_connectors(p) {
            let it = net.items[&id];
            v.push(on_item(3, ERROR, it, "This off-page connector has no partner on another sheet.".into()));
        }
        for it in p.items.iter().filter(|i| i.link != 0 && i.id < i.link && sym(i).is_some_and(|s| s.cat == Cat::Connector)) {
            let num = |id: u32| net.on_port(id, 0).first().map(|x| net.numbers[&x.0].clone()).unwrap_or_default();
            let (a, b) = (num(it.id), num(it.link));
            if !a.is_empty() && !b.is_empty() && a != b {
                v.push(Issue { rule: 3, level: ERROR, msg: format!("Off-page partners disagree on the line number: {a} here, {b} on the other sheet."), ids: vec![it.id, it.link], sheet: it.sheet, at: [it.x, it.y] });
            }
        }
    }

    // 4 and 5. Size and class changes, and 7. flow directions, across in-line items and off-page pairs.
    let mut clashes: Vec<(u32, u32, u32, bool)> = vec![];
    for it in &p.items {
        let Some(s) = sym(it) else { continue };
        let Some([(la, ba), (lb, bb)]) = sides(net, it) else { continue };
        let (l1, l2) = (net.lines[&la], net.lines[&lb]);
        let (s1, s2) = (text(l1, "size"), text(l2, "size"));
        let same_size = crate::pipes::nps_inches(&s1).zip(crate::pipes::nps_inches(&s2)).map_or(s1 == s2, |(a, b)| (a - b).abs() < 1e-9);
        if on(4) && !s1.is_empty() && !s2.is_empty() && !same_size && !s.key.starts_with("reducer") {
            v.push(on_item(4, ERROR, it, format!("The size changes from {s1}\" to {s2}\" at {} without a reducer.", name(it))));
        }
        let (c1, c2) = (text(l1, "class"), text(l2, "class"));
        if on(5) && !c1.is_empty() && !c2.is_empty() && !c1.eq_ignore_ascii_case(&c2) && s.key != "specbreak" {
            v.push(on_item(5, WARNING, it, format!("The pipe class changes from {c1} to {c2} at {} without a spec-break mark.", name(it))));
        }
        // A line flows into the item when the item is at its downstream end.
        let into = |l: &Line, at_b: bool| at_b != l.flip;
        if into(l1, ba) == into(l2, bb) {
            clashes.push((it.id, la, lb, into(l1, ba)));
        }
    }
    // 7. One issue per clash; a reversed piece between two clashes is named once.
    if on(7) {
        let count = |l: u32| clashes.iter().filter(|c| c.1 == l || c.2 == l).count();
        let mut done: Vec<u32> = vec![];
        for &(it, la, lb, inward) in &clashes {
            let item = net.items[&it];
            if let Some(l) = [la, lb].into_iter().find(|l| count(*l) > 1) {
                if !done.contains(&l) {
                    done.push(l);
                    let line = net.lines[&l];
                    v.push(Issue { rule: 7, level: WARNING, msg: format!("The flow arrows on a piece of {} point against the rest of the line.", label(net, l)), ids: vec![l], sheet: line.sheet, at: mid(p, line) });
                }
                continue;
            }
            let how = if inward { "both towards" } else { "both away from" };
            v.push(Issue { rule: 7, level: WARNING, msg: format!("The flow arrows on either side of {} point {how} it.", name(item)), ids: vec![la, lb, it], sheet: item.sheet, at: [item.x, item.y] });
        }
    }

    // 6. Missing tags, line numbers, sizes and classes.
    if on(6) {
        for it in &p.items {
            let Some(s) = sym(it) else { continue };
            let wants = matches!(s.cat, Cat::Vessel | Cat::Rotating | Cat::Heat | Cat::Control | Cat::Instrument | Cat::Primary) || (s.cat == Cat::Valve);
            if wants && it.tag.trim().is_empty() {
                v.push(on_item(6, WARNING, it, format!("This {} has no tag.", s.name.to_lowercase())));
            }
        }
        for lines in &net.runs {
            let l = net.lines[&lines[0]];
            if !text(l, "number").is_empty() {
                continue;
            }
            let missing: Vec<&str> = [("size", "size"), ("fluid", "fluid code"), ("seq", "sequence number"), ("class", "pipe class")].iter().filter(|(k, _)| text(l, k).is_empty()).map(|x| x.1).collect();
            if !missing.is_empty() {
                v.push(on_lines(6, WARNING, p, net, lines, format!("{} has no {}.", cap(&label(net, l.id)), join(&missing))));
            }
        }
    }

    // 8. Pump discharge without a check valve before the next equipment or branch.
    if on(8) {
        for it in p.items.iter().filter(|i| i.sym.starts_with("pump")) {
            let Some(s) = sym(it) else { continue };
            let Some(k) = s.ports.iter().position(|x| x.name == "discharge") else { continue };
            if net.on_port(it.id, k as u8).is_empty() {
                continue;
            }
            let found = net.reach(it.id, k as u8, false, 8, |_, ns| !ns.inline && ns.cat != Cat::Connector).iter().any(|(n, _, _)| matches!(net.items[n].sym.as_str(), "check" | "nonslam"));
            if !found {
                v.push(on_item(8, WARNING, it, format!("Pump {} has no check valve on its discharge.", name(it))));
            }
        }
    }

    // 9. Vessels and exchanger shells need a relief path.
    if on(9) {
        const RELIEF: [&str; 4] = ["psv", "rupture", "breather", "vent"];
        for it in &p.items {
            let Some(s) = sym(it) else { continue };
            let ports: Vec<u8> = match s.cat {
                Cat::Vessel => (0..s.ports.len() as u8).collect(),
                Cat::Heat if it.sym.starts_with("hx_") || it.sym == "jacketed" => s.ports.iter().enumerate().filter(|(_, x)| x.name.starts_with("shell") || x.name.starts_with("vapour") || x.name.starts_with("liquid") || !it.sym.starts_with("hx_")).map(|(k, _)| k as u8).collect(),
                _ => continue,
            };
            if it.data.get("relief").is_some_and(|x| !x.text().trim().is_empty()) {
                continue;
            }
            let found = ports.iter().any(|&k| net.reach(it.id, k, true, 10, |_, ns| ns.cat.equipment()).iter().any(|(n, _, _)| RELIEF.contains(&net.items[n].sym.as_str())));
            if !found {
                v.push(on_item(9, WARNING, it, format!("{} has no relief path: add a safety valve or a rupture disc, or say where relief is in its Relief path field.", cap(&name(it)))));
            }
        }
    }

    // 10. Control valves need isolation valves on both sides.
    if on(10) {
        const ISOLATE: [&str; 6] = ["gate", "globe", "ball", "butterfly", "plug", "diaphragm"];
        for it in p.items.iter().filter(|i| matches!(i.sym.as_str(), "cv_pneu" | "cv_motor")) {
            let Some(s) = sym(it) else { continue };
            let sides: Vec<bool> = s.ports.iter().enumerate().filter(|(_, x)| !x.signal).map(|(k, _)| net.reach(it.id, k as u8, false, 6, |ni, ns| (!ns.inline && ns.cat != Cat::Connector) || ISOLATE.contains(&ni.sym.as_str())).iter().any(|(n, _, _)| ISOLATE.contains(&net.items[n].sym.as_str()))).collect();
            if sides.iter().any(|x| !x) {
                v.push(on_item(10, NOTE, it, format!("Control valve {} has no isolation valve {}.", name(it), if sides.iter().all(|x| !x) { "on either side" } else { "on one side" })));
            }
        }
    }

    // 11. A controller needs a measurement and a final element in its loop.
    // 12. Instrument letters that do not match the symbol or the loop.
    let insts: Vec<(&Item, String, String, Cat)> = p.items.iter().filter_map(|i| { let s = sym(i)?; (s.cat.instrument() || s.cat == Cat::Control).then(|| { let (l, n) = tags::loop_of(&i.tag); (i, l, n, s.cat) }) }).collect();
    if on(11) {
        for (it, l, n, cat) in &insts {
            if *cat != Cat::Instrument || !tags::is_controller(l) || n.is_empty() {
                continue;
            }
            let in_loop = |f: &dyn Fn(&str, Cat) -> bool| insts.iter().any(|(o, ol, on_, oc)| o.id != it.id && on_ == n && f(ol, *oc));
            let meas = in_loop(&|ol, oc| oc.instrument() && tags::is_measurement(ol));
            let fin = insts.iter().any(|(o, ol, on_, _)| on_ == n && matches!(o.sym.as_str(), "cv_pneu" | "cv_motor" | "cv_solenoid" | "regulator") && tags::is_final(ol));
            if !meas || !fin {
                let what = match (meas, fin) { (false, false) => "a measurement or a final element", (false, true) => "a measurement (a transmitter or element)", _ => "a final element (a control valve)" };
                v.push(on_item(11, WARNING, it, format!("Controller {} has no {what} in loop {n}.", it.tag)));
            }
        }
    }
    if on(12) {
        for (it, l, n, _) in &insts {
            if it.tag.trim().is_empty() {
                continue;
            }
            if !tags::letters_ok(l) {
                v.push(on_item(12, NOTE, it, format!("{} does not start with ISA-5.1 letters.", it.tag)));
                continue;
            }
            if let Some(ends) = tags::expected_end(&it.sym) {
                if !ends.iter().any(|e| l.ends_with(e)) {
                    let s = sym(it).map_or("", |s| s.name);
                    v.push(on_item(12, NOTE, it, format!("{} is a {}; its letters would end in {}.", it.tag, s.to_lowercase(), ends.join(" or "))));
                    continue;
                }
            }
            if let Some(lp) = it.data.get("loop").map(|x| x.text()).filter(|x| !x.trim().is_empty()) {
                if lp.trim() != n {
                    v.push(on_item(12, NOTE, it, format!("{} carries loop number {} in its data but {n} in its tag.", it.tag, lp.trim())));
                }
            }
        }
    }

    // 13. Velocity or pressure drop outside the limits.
    if on(13) {
        for (r, h) in hyd.iter().enumerate() {
            if let Some(h) = h.as_ref().filter(|h| h.status > 0) {
                let lines = &net.runs[r];
                v.push(on_lines(13, WARNING, p, net, lines, format!("{}: {} ({}).", cap(&label(net, lines[0])), h.reason, h.service_name.to_lowercase())));
            }
        }
    }
    v
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}
fn join(v: &[&str]) -> String {
    match v.len() {
        0 => String::new(),
        1 => v[0].into(),
        _ => format!("{} or {}", v[..v.len() - 1].join(", "), v[v.len() - 1]),
    }
}

#[cfg(test)]
pub(crate) mod tests;
