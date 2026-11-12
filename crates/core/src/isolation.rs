//! The isolation planner: what to close, blind and open to make a piece of equipment, a valve or a
//! line safe to work on. From the chosen symbols and lines the process network is walked outwards
//! through lines, fittings, in-line instruments, check valves and off-page pairs until a valve or
//! a blind stops it. Those are the isolation points; valves inside whose far side goes nowhere are
//! the vents and drains. What cannot be isolated is said: an open end, another piece of equipment
//! reached with no valve between, a relief device on the section. A planning aid: the isolation is
//! the engineer's and the site's procedure to decide and sign.

use crate::graph::Net;
use crate::model::{End, Project};
use crate::symbols::Cat;
use serde::Serialize;
use std::collections::{BTreeSet, HashSet, VecDeque};

/// One isolation point.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Point {
    pub item: u32,
    pub tag: String,
    /// The symbol's name (Gate valve, Spectacle blind…).
    pub what: String,
    /// What to do: Close, Open (vent or drain), Swing to closed, Fit a blind, Leave in place.
    pub action: String,
    /// Its normal position from its data (NO, NC, LC…), "" if not given.
    pub normal: String,
    /// Positive isolation (a blind), not a valve alone.
    pub positive: bool,
    /// A remark (a control valve on its own, already closed…).
    pub note: String,
    /// "boundary", "vent" or "relief".
    pub role: &'static str,
}

/// The plan.
#[derive(Clone, Debug, Serialize, PartialEq, Default)]
pub struct Plan {
    /// What is isolated, in words.
    pub target: String,
    pub points: Vec<Point>,
    /// The line numbers inside the isolation.
    pub lines: Vec<String>,
    /// The symbols and lines inside (to show on the sheet).
    pub inside: Vec<u32>,
    /// What stops a safe isolation, or needs a decision.
    pub problems: Vec<String>,
    /// Remarks that need no change (a check valve walked through).
    pub notes: Vec<String>,
    /// The order of work, in words.
    pub steps: Vec<String>,
}

const CHECKS: [&str; 2] = ["check", "nonslam"];
const RELIEF: [&str; 3] = ["psv", "rupture", "breather"];
const BLINDS: [&str; 3] = ["blind", "spectacle", "spacer"];

fn name(p: &Project, item: u32) -> String {
    p.item(item).map(|i| p.tag_of(i).to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| p.item(item).and_then(|i| crate::symbols::get(&i.sym)).map_or("a symbol".into(), |s| s.name.to_lowercase()))
}

/// Does the far side of a valve (from `port`) go nowhere: only open ends, blinds, drains or vents?
fn goes_nowhere(net: &Net, item: u32, port: u8) -> bool {
    let Some(s) = net.sym(item) else { return false };
    let far: Vec<u8> = s.ports.iter().enumerate().filter(|(k, q)| !q.signal && *k as u8 != port).map(|(k, _)| k as u8).collect();
    let mut any = false;
    for k in far {
        for &(l, b) in net.on_port(item, k) {
            any = true;
            match net.other(l, b) {
                Some(End::Free { .. }) => {}
                Some(End::Port { item: n, .. }) if net.items.get(n).is_some_and(|i| ["blind", "drain", "vent"].contains(&i.base_key().as_str())) => {}
                _ => return false,
            }
            if net.branches.get(&l).is_some_and(|v| !v.is_empty()) {
                return false;
            }
        }
    }
    // A valve with nothing drawn on its far side is an open vent or drain too.
    any || s.ports.iter().filter(|q| !q.signal).count() >= 2
}

/// Plans the isolation of the chosen symbols and lines.
pub fn plan(p: &Project, ids: &[u32]) -> Result<Plan, String> {
    let net = Net::new(p);
    let targets: BTreeSet<u32> = ids.iter().copied().filter(|i| p.item(*i).is_some() || p.line(*i).is_some()).collect();
    if targets.is_empty() {
        return Err("Select the equipment, valve or line to isolate.".into());
    }
    let mut out = Plan::default();
    let words: Vec<String> = targets.iter().map(|&i| if p.item(i).is_some() { name(p, i) } else { p.line(i).map(|l| net.numbers.get(&l.id).cloned().unwrap_or_default()).filter(|n| !n.is_empty()).map_or("a line".into(), |n| format!("line {n}")) }).collect();
    out.target = words.join(", ");

    let mut inside: BTreeSet<u32> = targets.clone();
    let mut seen_lines: HashSet<u32> = HashSet::new();
    let mut points: Vec<(u32, u8)> = vec![];
    let mut q: VecDeque<(u32, Option<bool>)> = VecDeque::new();
    let mut problems: Vec<String> = vec![];
    let mut notes: Vec<String> = vec![];
    // From an item inside: every process line on every process port.
    let enter_item = |it: u32, q: &mut VecDeque<(u32, Option<bool>)>| {
        if let Some(s) = net.sym(it) {
            for (k, port) in s.ports.iter().enumerate() {
                if !port.signal {
                    q.extend(net.on_port(it, k as u8).iter().map(|&(l, b)| (l, Some(b))));
                }
            }
        }
    };
    for &t in &targets {
        if p.item(t).is_some() {
            enter_item(t, &mut q);
        } else {
            q.push_back((t, None));
        }
    }
    while let Some((l, entered)) = q.pop_front() {
        let Some(line) = net.lines.get(&l) else { continue };
        if !line.kind.process() || !seen_lines.insert(l) {
            continue;
        }
        inside.insert(l);
        q.extend(net.branches.get(&l).into_iter().flatten().map(|&(bl, bb)| (bl, Some(bb))));
        let ends: Vec<&End> = match entered { Some(at_b) => vec![if at_b { &line.a } else { &line.b }], None => vec![&line.a, &line.b] };
        for e in ends {
            match *e {
                End::Free { .. } => problems.push(format!("Line {} has an open end: blind it, or show where it goes.", net.numbers.get(&l).filter(|n| !n.is_empty()).map_or("?", |n| n.as_str()))),
                End::Line { line: m, .. } => q.push_back((m, None)),
                End::Port { item: n, port: np } => {
                    if inside.contains(&n) && !targets.contains(&n) || targets.contains(&n) {
                        continue;
                    }
                    let (Some(it), Some(s)) = (net.items.get(&n), net.sym(n)) else { continue };
                    let base = it.base_key();
                    if s.cat.valve() && !CHECKS.contains(&base.as_str()) || BLINDS.contains(&base.as_str()) || RELIEF.contains(&base.as_str()) {
                        if !points.iter().any(|x| x.0 == n) {
                            points.push((n, np));
                        }
                    } else if s.cat.equipment() {
                        problems.push(format!("The isolation reaches {} with no valve or blind between: add one, or isolate {} too.", name(p, n), name(p, n)));
                    } else if s.cat == Cat::Connector {
                        inside.insert(n);
                        if it.link != 0 {
                            inside.insert(it.link);
                            q.extend(net.on_port(it.link, 0).iter().map(|&(ml, mb)| (ml, Some(mb))));
                        } else {
                            problems.push(format!("Off-page connector {} is not paired: the isolation cannot follow it.", name(p, n)));
                        }
                    } else {
                        // Fittings, in-line instruments, check valves, instrument bubbles: inside.
                        if CHECKS.contains(&base.as_str()) {
                            notes.push(format!("{} is a check valve: not an isolation, so it is inside.", name(p, n)));
                        }
                        inside.insert(n);
                        enter_item(n, &mut q);
                    }
                }
            }
        }
    }
    // The points: boundaries, vents and drains, relief devices.
    let data_pos = |it: u32| p.item(it).map(|i| p.data_of(i.obj).get("position").map(|v| v.text().trim().to_uppercase()).unwrap_or_default()).unwrap_or_default();
    for (it, port) in points {
        let Some(i) = p.item(it) else { continue };
        let s = crate::symbols::get(&i.sym);
        let base = i.base_key();
        let normal = data_pos(it);
        let mut pt = Point { item: it, tag: p.tag_of(i).to_string(), what: s.map_or(String::new(), |s| s.name.to_string()), action: "Close".into(), normal: normal.clone(), positive: false, note: String::new(), role: "boundary" };
        if RELIEF.contains(&base.as_str()) {
            pt.role = "relief";
            pt.action = "Decide".into();
            pt.note = "A relief device on the isolated section: keep it in service while the section can be pressurised, or isolate it under the site's procedure.".into();
        } else if BLINDS.contains(&base.as_str()) {
            pt.positive = true;
            pt.action = if base == "spectacle" { "Swing to closed".into() } else if base == "spacer" { "Replace with a blind".into() } else { "Leave in place".into() };
        } else if s.is_some_and(|s| s.inline) && goes_nowhere(&net, it, port) {
            pt.role = "vent";
            pt.action = "Open (vent or drain)".into();
            pt.note = "Opened after the boundary is closed, to depressurise and drain; closed again before the section is put back.".into();
        } else {
            if s.is_some_and(|s| s.cat == Cat::Control) {
                pt.note = "A control valve is not an isolation on its own: close the block valve beside it, or add one.".into();
            }
            if normal == "NC" || normal == "LC" {
                pt.action = "Confirm closed".into();
                pt.note = format!("Normally {}: confirm it is closed and lock it.", if normal == "LC" { "locked closed" } else { "closed" });
            }
        }
        if pt.tag.is_empty() {
            pt.tag = "(no tag)".into();
            problems.push(format!("A {} on the boundary has no tag: tag it so the isolation can be written down.", pt.what.to_lowercase()));
        }
        out.points.push(pt);
    }
    out.points.sort_by(|a, b| (a.role != "boundary", a.role == "relief", &a.tag).cmp(&(b.role != "boundary", b.role == "relief", &b.tag)));
    if out.points.iter().any(|x| x.role == "boundary" && x.note.starts_with("A control valve")) {
        problems.push("A control valve is on the boundary.".into());
    }
    if !out.points.iter().any(|x| x.role == "boundary") && problems.is_empty() {
        problems.push("Nothing bounds this: no valve or blind was found round it.".into());
    }
    if !out.points.iter().any(|x| x.role == "vent") {
        problems.push("No vent or drain inside the isolation: add one to depressurise the section and prove it.".into());
    }
    problems.dedup();
    out.problems = problems;
    notes.dedup();
    out.notes = notes;
    out.lines = inside.iter().filter_map(|i| p.line(*i)).filter_map(|l| net.numbers.get(&l.id).cloned()).filter(|n| !n.is_empty()).collect::<BTreeSet<_>>().into_iter().collect();
    out.inside = inside.into_iter().collect();
    let tags = |role: &str, f: &dyn Fn(&Point) -> bool| out.points.iter().filter(|x| x.role == role && f(x)).map(|x| x.tag.clone()).collect::<Vec<_>>().join(", ");
    let close = tags("boundary", &|x| !x.positive);
    let blinds = tags("boundary", &|x| x.positive && x.action != "Leave in place");
    let vents = tags("vent", &|_| true);
    let mut steps = vec![];
    if !close.is_empty() {
        steps.push(format!("Close and lock: {close}."));
    }
    if !vents.is_empty() {
        steps.push(format!("Open to depressurise and drain: {vents}; prove zero pressure."));
    }
    if !blinds.is_empty() {
        steps.push(format!("For positive isolation: {blinds}."));
    }
    steps.push("Hand back in reverse order: blinds out, vents and drains closed, then the boundary valves opened.".into());
    out.steps = steps;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(p: &Project, t: &str) -> u32 {
        p.items.iter().find(|i| p.tag_of(i) == t).unwrap_or_else(|| panic!("{t}")).id
    }

    #[test]
    fn a_pump_is_isolated_by_its_valves() {
        let p = crate::examples::get("pump").unwrap();
        let plan = plan(&p, &[tag(&p, "P-101A")]).unwrap();
        let bound: Vec<&str> = plan.points.iter().filter(|x| x.role == "boundary").map(|x| x.tag.as_str()).collect();
        assert!(bound.len() >= 2, "{plan:#?}");
        // Its check valve is inside, not a boundary; the walk does not reach the tank or the vessel.
        assert!(plan.points.iter().all(|x| !x.what.to_lowercase().contains("check")), "{plan:#?}");
        assert!(plan.notes.iter().any(|x| x.contains("check valve")));
        assert!(plan.problems.iter().all(|x| !x.contains("reaches")), "{:?}", plan.problems);
        assert!(plan.inside.contains(&tag(&p, "P-101A")) && !plan.lines.is_empty());
        assert!(plan.steps[0].starts_with("Close and lock"));
        // Hand back is always the last step.
        assert!(plan.steps.last().unwrap().starts_with("Hand back"));
    }

    #[test]
    fn the_worked_example_isolates_a_pump_with_its_drain() {
        let p = crate::examples::get("worked").unwrap();
        let plan = plan(&p, &[tag(&p, "P-101A")]).unwrap();
        let roles: Vec<(&str, &str)> = plan.points.iter().map(|x| (x.role, x.action.as_str())).collect();
        assert!(roles.contains(&("vent", "Open (vent or drain)")) && roles.iter().filter(|r| r.0 == "boundary").count() == 2, "{roles:?}");
        assert!(plan.problems.is_empty(), "{:?}", plan.problems);
        assert!(plan.steps[1].starts_with("Open to depressurise"));
    }

    #[test]
    fn what_cannot_be_isolated_is_said() {
        let mut p = crate::examples::get("pump").unwrap();
        // A line with an open end, straight off a vessel: no valve, no vent.
        let s = p.sheets[0].id;
        let v = tag(&p, "V-101");
        let id = p.id();
        let free = crate::model::Line { id, sheet: s, a: End::Port { item: v, port: 0 }, b: End::Free { x: 0.0, y: 0.0 }, ..Default::default() };
        p.lines.push(free);
        let plan = plan(&p, &[v]).unwrap();
        assert!(plan.problems.iter().any(|x| x.contains("open end")), "{:?}", plan.problems);
        assert!(plan.problems.iter().any(|x| x.contains("No vent or drain")) || plan.points.iter().any(|x| x.role == "vent"));
        assert!(super::plan(&p, &[]).is_err());
        assert!(super::plan(&p, &[999_999]).is_err());
    }

    #[test]
    fn every_symbol_and_line_of_every_example_plans_without_panic() {
        for ex in crate::examples::list().iter().map(|e| e.id) {
            let p = crate::examples::get(ex).unwrap();
            for id in p.items.iter().map(|i| i.id).chain(p.lines.iter().map(|l| l.id)) {
                let r = plan(&p, &[id]).unwrap();
                assert!(!r.steps.is_empty() && r.inside.contains(&id), "{ex} {id}");
            }
        }
    }
}
