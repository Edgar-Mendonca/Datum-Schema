//! HAZOP support: the study's nodes marked on the P&ID, a worksheet for each, and suggestions read
//! from the drawing. A node is a set of symbols and lines with its design intent. For each node the
//! usual deviations (no, less, more and reverse flow; more and less pressure; temperature; level;
//! contamination; maintenance) can be suggested with the causes the drawing shows (the pumps,
//! valves, strainers, control valves and their fail positions in it) and the safeguards it shows
//! (relief devices, check valves, alarms and trips on its lines and equipment, a standby pump, the
//! isolation the planner finds). Suggestions are marked as such until the team confirms them; the
//! consequences, the risk and the recommendations are the team's. Tags written in the worksheet
//! follow a renamed tag. The worksheet and the recommendations are lists (print, Excel, CSV).
//! An aid to the study: the HAZOP and its findings are the team's.

use crate::graph::Net;
use crate::model::Project;
use crate::symbols::{self, Cat};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};

/// The study.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Study {
    pub title: String,
    pub date: String,
    pub team: String,
    /// Draw the nodes on the sheets (and print them).
    pub show: bool,
    pub nodes: Vec<Node>,
}

/// A node: part of the plant studied as one, with its design intent and its worksheet.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Node {
    /// 1, 2, 3… (shown as N1, N2…).
    pub id: u32,
    pub name: String,
    pub intent: String,
    /// The symbols and lines in it.
    pub ids: Vec<u32>,
    pub rows: Vec<Row>,
}

/// A worksheet row: one deviation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Row {
    pub param: String,
    pub guide: String,
    pub deviation: String,
    pub causes: String,
    pub consequences: String,
    pub safeguards: String,
    /// Severity and likelihood, as the company's risk matrix writes them (1–5, A–E…).
    pub severity: String,
    pub likelihood: String,
    pub rec: String,
    /// Who acts on the recommendation.
    pub by: String,
    /// Open or Closed ("" when there is no recommendation).
    pub status: String,
    /// Suggested from the drawing and not yet confirmed by the team.
    pub suggested: bool,
}

const MAX_NODES: usize = 300;
const MAX_ROWS: usize = 300;

/// Keeps a study sound: ids that exist, unique node numbers, sizes bounded, statuses known.
pub fn normalize(s: &mut Study, exists: &dyn Fn(u32) -> bool) {
    s.title = crate::clip(&s.title, 200);
    s.date = crate::clip(&s.date, 40);
    s.team = crate::clip(&s.team, 2000);
    s.nodes.truncate(MAX_NODES);
    let mut seen = HashSet::new();
    let mut next = s.nodes.iter().map(|n| n.id).max().unwrap_or(0);
    for n in &mut s.nodes {
        if n.id == 0 || !seen.insert(n.id) {
            next += 1;
            n.id = next;
            seen.insert(n.id);
        }
        n.name = crate::clip(&n.name, 200);
        n.intent = crate::clip(&n.intent, 2000);
        n.ids.retain(|i| exists(*i));
        n.ids.sort_unstable();
        n.ids.dedup();
        n.rows.truncate(MAX_ROWS);
        for r in &mut n.rows {
            for f in [&mut r.param, &mut r.guide, &mut r.deviation, &mut r.severity, &mut r.likelihood, &mut r.by, &mut r.status] {
                *f = crate::clip(f, 200);
            }
            for f in [&mut r.causes, &mut r.consequences, &mut r.safeguards, &mut r.rec] {
                *f = crate::clip(f, 4000);
            }
            if !r.rec.trim().is_empty() && !["Open", "Closed"].contains(&r.status.as_str()) {
                r.status = "Open".into();
            }
            if r.rec.trim().is_empty() {
                r.status.clear();
            }
        }
    }
}

/// A new node from the chosen symbols and lines.
pub fn add_node(p: &mut Project, ids: &[u32], name: &str) -> Result<u32, String> {
    let ids: Vec<u32> = ids.iter().copied().filter(|i| p.item(*i).is_some() || p.line(*i).is_some()).collect();
    if ids.is_empty() {
        return Err("Select the symbols and lines of the node first.".into());
    }
    let s = &mut p.settings.hazop;
    if s.nodes.len() >= MAX_NODES {
        return Err(format!("A study has at most {MAX_NODES} nodes."));
    }
    let id = s.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
    let name = if name.trim().is_empty() { format!("Node {id}") } else { name.trim().to_string() };
    s.nodes.push(Node { id, name, intent: String::new(), ids, rows: vec![] });
    s.show = true;
    Ok(id)
}

/// Is `t` at `i` in `s` a whole tag (not part of a longer one)?
fn whole(s: &str, i: usize, len: usize) -> bool {
    let tagch = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    !s[..i].chars().next_back().is_some_and(tagch) && !s[i + len..].chars().next().is_some_and(tagch)
}

/// Replaces a whole tag in a text.
fn replace_tag(s: &str, old: &str, new: &str) -> String {
    if old.is_empty() || !s.contains(old) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut at = 0;
    while let Some(k) = s[at..].find(old) {
        let i = at + k;
        out.push_str(&s[at..i]);
        out.push_str(if whole(s, i, old.len()) { new } else { old });
        at = i + old.len();
    }
    out.push_str(&s[at..]);
    out
}

/// Renamed tags (old, new) followed in the study's texts.
pub fn rename(s: &mut Study, renames: &[(String, String)]) {
    for n in &mut s.nodes {
        for (o, w) in renames {
            n.name = replace_tag(&n.name, o, w);
            n.intent = replace_tag(&n.intent, o, w);
            for r in &mut n.rows {
                for f in [&mut r.causes, &mut r.consequences, &mut r.safeguards, &mut r.rec] {
                    *f = replace_tag(f, o, w);
                }
            }
        }
    }
}

/// What the drawing shows in a node.
struct Seen {
    pumps: Vec<String>,
    standby: Vec<String>,
    vessels: Vec<String>,
    heat: Vec<String>,
    valves: Vec<String>,
    checks: Vec<String>,
    strainers: Vec<String>,
    /// Control valves with their fail position.
    cvs: Vec<(String, String)>,
    relief: Vec<String>,
    /// Instruments on the node's lines and equipment, with their letters and setpoints.
    instruments: Vec<(String, String, String)>,
    relief_paths: Vec<String>,
}

fn seen(p: &Project, net: &Net, node: &Node) -> Seen {
    let mut s = Seen { pumps: vec![], standby: vec![], vessels: vec![], heat: vec![], valves: vec![], checks: vec![], strainers: vec![], cvs: vec![], relief: vec![], instruments: vec![], relief_paths: vec![] };
    let mut objs: HashSet<u32> = HashSet::new();
    let mut tagged = BTreeSet::new();
    let push = |v: &mut Vec<String>, t: &str| {
        if !t.is_empty() && !v.iter().any(|x| x == t) {
            v.push(t.to_string());
        }
    };
    for &id in &node.ids {
        if let Some(l) = p.line(id) {
            objs.insert(l.obj);
            continue;
        }
        let Some(it) = p.item(id) else { continue };
        objs.insert(it.obj);
        let Some(sym) = symbols::get(&it.sym) else { continue };
        let tag = p.tag_of(it).to_string();
        if !tagged.insert(it.obj) && it.obj != 0 {
            continue;
        }
        let d = p.data_of(it.obj);
        let base = it.base_key();
        match sym.cat {
            Cat::Rotating if matches!(base.as_str(), "pump_cent" | "pump_pd" | "pump_vil" | "comp_cent" | "comp_recip" | "comp_screw" | "fan") => {
                push(&mut s.pumps, &tag);
                if d.get("operation").is_some_and(|v| v.text().eq_ignore_ascii_case("standby")) {
                    push(&mut s.standby, &tag);
                }
            }
            Cat::Vessel => push(&mut s.vessels, &tag),
            Cat::Heat => push(&mut s.heat, &tag),
            _ if matches!(base.as_str(), "check" | "nonslam") => push(&mut s.checks, &tag),
            _ if matches!(base.as_str(), "psv" | "rupture" | "breather") => push(&mut s.relief, &tag),
            Cat::Valve => {
                let pos = d.get("position").map(|v| v.text().to_uppercase()).unwrap_or_default();
                if pos != "NC" && pos != "LC" {
                    push(&mut s.valves, &tag);
                }
            }
            Cat::Control => {
                if !tag.is_empty() && !s.cvs.iter().any(|x| x.0 == tag) {
                    s.cvs.push((tag.clone(), d.get("fail").map(|v| v.text().trim().to_uppercase()).unwrap_or_default()));
                }
            }
            Cat::Fitting if base.starts_with("strainer") || base == "filter" => push(&mut s.strainers, &tag),
            _ => {}
        }
        if sym.cat.equipment() {
            if let Some(r) = d.get("relief").map(|v| v.text()).filter(|t| !t.trim().is_empty()) {
                s.relief_paths.push(format!("{tag}: relief path {r}"));
            }
        }
    }
    objs.remove(&0);
    // Instruments and relief devices whose process connection is on the node (drawn anywhere).
    let mut done = HashSet::new();
    for it in &p.items {
        let Some(sym) = symbols::get(&it.sym) else { continue };
        let relief = matches!(it.base_key().as_str(), "psv" | "rupture" | "breather");
        if !(sym.cat.instrument() || relief) || it.obj == 0 || !done.insert(it.obj) {
            continue;
        }
        let tag = p.tag_of(it).to_string();
        let on = node.ids.contains(&it.id) || crate::datasheets::process_source(p, net, it).is_some_and(|(src, _)| objs.contains(&src));
        if !on || tag.is_empty() {
            continue;
        }
        if relief {
            push(&mut s.relief, &tag);
        } else {
            let (letters, _) = crate::tags::loop_of(&tag);
            let sp = p.data_of(it.obj).get("setpoints").map(|v| v.text().to_uppercase()).unwrap_or_default();
            s.instruments.push((tag, letters, sp));
        }
    }
    s
}

impl Seen {
    /// Instruments that alarm or trip on a variable in one direction: their letters (PAH, PSHH…),
    /// or a transmitter or controller of that variable whose setpoints name it.
    fn guards(&self, var: char, high: bool) -> Vec<String> {
        let dir = if high { 'H' } else { 'L' };
        self.instruments
            .iter()
            .filter(|(_, l, sp)| {
                let mine = l.starts_with(var) && l.len() >= 3 && ["S", "A", "Z"].iter().any(|k| l[1..].starts_with(k)) && l.ends_with(dir);
                let words = sp.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w.len() >= 3 && w.starts_with(var) && (w.starts_with(&format!("{var}A{dir}")) || w.starts_with(&format!("{var}S{dir}")) || w.starts_with(&format!("{var}Z{dir}"))));
                mine || (l.starts_with(var) && words)
            })
            .map(|(t, l, _)| if l.len() >= 3 { format!("{t} ({})", describe(l)) } else { format!("{t} alarm") })
            .collect()
    }
}

fn describe(letters: &str) -> &'static str {
    let a = letters.contains('A');
    let z = letters.contains('Z') || letters.ends_with("HH") || letters.ends_with("LL");
    match (a, z) {
        (_, true) => "trip",
        (true, _) => "alarm",
        _ => "switch",
    }
}

fn join(v: &[String]) -> String {
    v.join(", ")
}

/// The usual deviations for a node, with what the drawing shows as causes and safeguards. Rows the
/// node's worksheet already has (the same parameter and guideword) are left out.
pub fn suggest(p: &Project, node: &Node) -> Vec<Row> {
    let net = Net::new(p);
    let s = seen(p, &net, node);
    let mut rows = vec![];
    let mut row = |param: &str, guide: &str, dev: &str, causes: Vec<String>, safeguards: Vec<String>| {
        rows.push(Row { param: param.into(), guide: guide.into(), deviation: dev.into(), causes: causes.join("; "), safeguards: if safeguards.is_empty() { "None shown on the drawing".into() } else { safeguards.join("; ") }, suggested: true, ..Row::default() });
    };
    let fc: Vec<String> = s.cvs.iter().filter(|x| x.1 == "FC").map(|x| x.0.clone()).collect();
    let fo: Vec<String> = s.cvs.iter().filter(|x| x.1 == "FO").map(|x| x.0.clone()).collect();
    let cvs: Vec<String> = s.cvs.iter().map(|x| x.0.clone()).collect();
    let mut relief: Vec<String> = s.relief.iter().map(|t| format!("{t} relieves")).collect();
    relief.extend(s.relief_paths.iter().cloned());

    // Flow.
    let mut c = vec![];
    if !s.pumps.is_empty() {
        c.push(format!("{} stops or trips", join(&s.pumps)));
    }
    if !s.valves.is_empty() {
        c.push(format!("{} closed in error", join(&s.valves)));
    }
    if !fc.is_empty() {
        c.push(format!("{} fails closed", join(&fc)));
    }
    if !s.strainers.is_empty() {
        c.push(format!("{} blocked", join(&s.strainers)));
    }
    c.push("Loss of supply upstream".into());
    let mut g = s.guards('F', false);
    g.extend(s.guards('P', false));
    if !s.standby.is_empty() {
        g.push(format!("standby {}", join(&s.standby)));
    }
    row("Flow", "No", "No flow", c, g.clone());
    let mut c = vec![];
    if !s.valves.is_empty() {
        c.push(format!("{} partly closed", join(&s.valves)));
    }
    if !s.strainers.is_empty() {
        c.push(format!("{} partly blocked", join(&s.strainers)));
    }
    if !cvs.is_empty() {
        c.push(format!("{} throttles too far", join(&cvs)));
    }
    if !s.pumps.is_empty() {
        c.push(format!("{} worn or cavitating", join(&s.pumps)));
    }
    if c.is_empty() {
        c.push("Restriction or leak".into());
    }
    row("Flow", "Less", "Less flow", c, g);
    let mut c = vec![];
    if !fo.is_empty() {
        c.push(format!("{} fails open", join(&fo)));
    }
    if s.pumps.len() >= 2 {
        c.push(format!("{} running together", join(&s.pumps)));
    }
    if !cvs.is_empty() && fo.is_empty() {
        c.push(format!("{} opens too far", join(&cvs)));
    }
    if c.is_empty() {
        c.push("Higher supply pressure upstream".into());
    }
    row("Flow", "More", "More flow", c, s.guards('F', true));
    if !s.pumps.is_empty() || !s.checks.is_empty() {
        let c = if s.pumps.is_empty() { vec!["Higher pressure downstream".to_string()] } else { vec![format!("{} stops with the delivery under pressure", join(&s.pumps))] };
        let g: Vec<String> = s.checks.iter().map(|t| format!("check valve {t}")).collect();
        row("Flow", "Reverse", "Reverse flow", c, g);
    }
    // Pressure.
    let mut c = vec![];
    if !s.valves.is_empty() {
        c.push(format!("Outlet blocked: {} closed", join(&s.valves)));
    }
    if !s.pumps.is_empty() {
        c.push(format!("{} against a closed outlet", join(&s.pumps)));
    }
    if !fc.is_empty() {
        c.push(format!("{} fails closed", join(&fc)));
    }
    c.push("External fire".into());
    if !s.vessels.is_empty() || !s.heat.is_empty() {
        c.push("Blocked in and heated".into());
    }
    let mut g = relief.clone();
    g.extend(s.guards('P', true));
    row("Pressure", "More", "More pressure", c, g);
    let mut c = vec!["Draining or pumping out while closed in".to_string()];
    if !s.pumps.is_empty() {
        c.push(format!("Suction of {} starved", join(&s.pumps)));
    }
    let mut g = s.guards('P', false);
    g.extend(s.relief.iter().filter(|t| t.starts_with("PVSV")).map(|t| format!("{t} admits air")));
    row("Pressure", "Less", "Less pressure (vacuum)", c, g);
    // Temperature, where the node heats or cools.
    if !s.heat.is_empty() || s.instruments.iter().any(|x| x.1.starts_with('T')) {
        let mut c: Vec<String> = s.heat.iter().map(|t| format!("{t} overheats (heating medium control fails)")).collect();
        c.push("External fire".into());
        row("Temperature", "More", "More temperature", c, s.guards('T', true));
        let mut c: Vec<String> = s.heat.iter().map(|t| format!("{t} loses its heating")).collect();
        c.push("Low ambient temperature".into());
        row("Temperature", "Less", "Less temperature", c, s.guards('T', false));
    }
    // Level, where the node has a vessel.
    if !s.vessels.is_empty() {
        let lv: Vec<String> = s.cvs.iter().filter(|x| x.0.starts_with('L')).map(|x| format!("{} fails {}", x.0, if x.1 == "FO" { "closed" } else { "closed or sticks" })).collect();
        let mut c = vec![format!("Inflow to {} above its outflow", join(&s.vessels))];
        c.extend(lv);
        if !s.valves.is_empty() {
            c.push(format!("Outlet blocked: {} closed", join(&s.valves)));
        }
        row("Level", "More", "High level", c, s.guards('L', true));
        let mut c = vec![format!("Outflow from {} above its inflow", join(&s.vessels))];
        c.extend(s.cvs.iter().filter(|x| x.0.starts_with('L')).map(|x| format!("{} fails open", x.0)));
        c.push("Drain left open".into());
        let mut g = s.guards('L', false);
        if !s.pumps.is_empty() {
            g.push(format!("(pump protection for {}: low level trip?)", join(&s.pumps)));
        }
        row("Level", "Less", "Low level", c, g);
    }
    // Composition.
    row("Composition", "Other than", "Contamination or wrong material", vec!["Wrong material supplied".into(), "Leak from a connected utility".into(), "Water or debris after maintenance".into()], vec![]);
    // Maintenance: the isolation the drawing allows.
    let ids: Vec<u32> = node.ids.iter().copied().filter(|i| p.item(*i).is_some_and(|it| symbols::get(&it.sym).is_some_and(|s| s.cat.equipment()))).collect();
    let target = if ids.is_empty() { node.ids.clone() } else { ids };
    let mut g = vec![];
    if let Ok(plan) = crate::isolation::plan(p, &target) {
        let close: Vec<String> = plan.points.iter().filter(|x| x.role == "boundary").map(|x| x.tag.clone()).collect();
        let vents: Vec<String> = plan.points.iter().filter(|x| x.role == "vent").map(|x| x.tag.clone()).collect();
        if !close.is_empty() {
            g.push(format!("isolated by {}", join(&close)));
        }
        if !vents.is_empty() {
            g.push(format!("drained or vented by {}", join(&vents)));
        }
        g.extend(plan.problems.iter().map(|x| format!("(isolation: {x})")));
    }
    row("Maintenance", "Other than", "Isolation and draining for maintenance", vec!["Work on the node's equipment or valves".into()], g);
    // Not the deviations the worksheet already has.
    rows.retain(|r| !node.rows.iter().any(|x| x.param.eq_ignore_ascii_case(&r.param) && x.guide.eq_ignore_ascii_case(&r.guide)));
    rows
}

/// Adds the suggestions to a node's worksheet; how many were added.
pub fn add_suggestions(p: &mut Project, node: u32) -> Result<usize, String> {
    let n = p.settings.hazop.nodes.iter().find(|n| n.id == node).cloned().ok_or("That node no longer exists.")?;
    let rows = suggest(p, &n);
    let k = rows.len();
    let n = p.settings.hazop.nodes.iter_mut().find(|n| n.id == node).ok_or("lost")?;
    n.rows.extend(rows);
    n.rows.truncate(MAX_ROWS);
    Ok(k)
}

/// The nodes drawn on a sheet: a dashed outline round each node's symbols and lines there, and
/// its number and name above.
pub fn prims(p: &Project, sheet: u32) -> Vec<symbols::Prim> {
    use symbols::Prim;
    let s = &p.settings.hazop;
    if !s.show {
        return vec![];
    }
    let routes = crate::view::routes(p, sheet, None);
    let mut out = vec![];
    for n in &s.nodes {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        let mut grow = |x: f64, y: f64| {
            b[0] = b[0].min(x);
            b[1] = b[1].min(y);
            b[2] = b[2].max(x);
            b[3] = b[3].max(y);
        };
        for &id in &n.ids {
            if let Some(it) = p.item(id).filter(|i| i.sheet == sheet) {
                let bb = crate::view::bbox(it);
                grow(bb[0], bb[1]);
                grow(bb[2], bb[3]);
            } else if let Some(r) = p.line(id).filter(|l| l.sheet == sheet).and_then(|l| routes.get(&l.id)) {
                r.iter().for_each(|q| grow(q[0], q[1]));
            }
        }
        if b[0] > b[2] {
            continue;
        }
        let m = 4.0;
        let (x0, y0, x1, y1) = (b[0] - m, b[1] - m, b[2] + m, b[3] + m);
        out.push(Prim::Poly { pts: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], closed: true, fill: false, dash: true, w: 0.35 });
        out.push(Prim::Text { p: [x0, y0 - 2.5], h: 3.0, s: format!("N{} {}", n.id, n.name), anchor: 0, bold: true });
    }
    out
}

/// The worksheet list (every node) and the recommendations register.
pub fn list(p: &Project, kind: &str) -> crate::lists::List {
    use crate::lists::{Col, List, Row as LRow};
    let col = |k: &str, l: &str| Col { key: k.into(), label: l.into(), q: "", edit: false, num: false };
    let s = &p.settings.hazop;
    let first = |n: &Node| n.ids.iter().find_map(|i| p.item(*i).map(|it| (it.id, it.sheet)).or_else(|| p.line(*i).map(|l| (l.id, l.sheet)))).unwrap_or((0, 0));
    if kind == "hazop_recs" {
        let cols = vec![col("no", "No."), col("node", "Node"), col("deviation", "Deviation"), col("rec", "Recommendation"), col("by", "By"), col("status", "Status")];
        let mut rows = vec![];
        for n in &s.nodes {
            let (id, sheet) = first(n);
            for r in n.rows.iter().filter(|r| !r.rec.trim().is_empty()) {
                let no = rows.len() + 1;
                rows.push(LRow { ids: if id != 0 { vec![id] } else { vec![] }, sheet, cells: vec![no.to_string(), format!("N{} {}", n.id, n.name), r.deviation.clone(), r.rec.clone(), r.by.clone(), r.status.clone()], status: (r.status == "Open").then_some(1) });
            }
        }
        return List { kind: kind.into(), title: "HAZOP recommendations".into(), cols, rows, heads: vec![] };
    }
    let cols = vec![col("node", "Node"), col("param", "Parameter"), col("guide", "Guideword"), col("deviation", "Deviation"), col("causes", "Causes"), col("consequences", "Consequences"), col("safeguards", "Safeguards"), col("severity", "S"), col("likelihood", "L"), col("rec", "Recommendation"), col("by", "By"), col("status", "Status")];
    let mut rows = vec![];
    let mut heads = vec![];
    for n in &s.nodes {
        let (id, sheet) = first(n);
        heads.push(rows.len());
        let intent = if n.intent.trim().is_empty() { String::new() } else { format!("Intent: {}", n.intent) };
        let mut head = vec![format!("N{} {}", n.id, n.name), intent];
        head.resize(cols.len(), String::new());
        rows.push(LRow { ids: if id != 0 { vec![id] } else { vec![] }, sheet, cells: head, status: None });
        for r in &n.rows {
            let mark = |t: &str| if r.suggested && !t.is_empty() { format!("{t} (suggested)") } else { t.to_string() };
            rows.push(LRow { ids: if id != 0 { vec![id] } else { vec![] }, sheet, cells: vec![format!("N{}", n.id), r.param.clone(), r.guide.clone(), r.deviation.clone(), mark(&r.causes), r.consequences.clone(), mark(&r.safeguards), r.severity.clone(), r.likelihood.clone(), r.rec.clone(), r.by.clone(), r.status.clone()], status: (r.status == "Open").then_some(1) });
        }
    }
    List { kind: kind.into(), title: "HAZOP worksheet".into(), cols, rows, heads }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    fn tag(p: &Project, t: &str) -> u32 {
        p.items.iter().find(|i| p.tag_of(i) == t).unwrap_or_else(|| panic!("{t}")).id
    }

    #[test]
    fn a_node_gets_its_deviations_from_the_drawing() {
        let mut d = Doc::new(crate::examples::get("worked").unwrap());
        // Node 1: pump P-101A with its suction and discharge as far as the vessel.
        let ids: Vec<u32> = ["P-101A", "HV-101", "S-101A", "HV-103", "HV-104", "V-101", "LV-101", "PSV-101"].iter().map(|t| tag(&d.p, t)).collect();
        let lines: Vec<u32> = d.p.lines.iter().filter(|l| l.kind.process() && [&l.a, &l.b].iter().any(|e| e.item().is_some_and(|x| ids.contains(&x.0)))).map(|l| l.id).collect();
        let all: Vec<u32> = ids.iter().chain(&lines).copied().collect();
        let n = d.apply(Op::HazopNode { ids: all, name: "Transfer to V-101".into() }).unwrap()[0];
        assert_eq!(n, 1);
        d.apply(Op::HazopSuggest { node: 1 }).unwrap();
        let node = d.p.settings.hazop.nodes[0].clone();
        let get = |dev: &str| node.rows.iter().find(|r| r.deviation == dev).unwrap_or_else(|| panic!("{dev}: {:#?}", node.rows));
        assert!(get("No flow").causes.contains("P-101A stops") && get("No flow").causes.contains("HV-101, HV-104 closed in error"), "{:?}", get("No flow"));
        assert!(get("Reverse flow").safeguards.contains("HV-103"), "{:?}", get("Reverse flow"));
        assert!(get("More pressure").safeguards.contains("PSV-101"), "{:?}", get("More pressure"));
        assert!(get("High level").safeguards.contains("LIC-101") || get("High level").safeguards.contains("LT-101"), "{:?}", get("High level"));
        assert!(get("Isolation and draining for maintenance").safeguards.contains("drained or vented by"), "{:?}", get("Isolation and draining for maintenance"));
        assert!(node.rows.iter().all(|r| r.suggested));
        // Suggesting again adds nothing new; the lists show it.
        assert_eq!(add_suggestions(&mut d.p.clone(), 1).unwrap(), 0);
        let l = crate::lists::build(&d.p, "hazop").unwrap();
        assert_eq!(l.rows.len(), node.rows.len() + 1);
        // A recommendation, then a renamed tag followed in the worksheet.
        let mut st = d.p.settings.hazop.clone();
        st.nodes[0].rows[0].rec = "Add a low-flow alarm on P-101A discharge".into();
        let mut s = d.p.settings.clone();
        s.hazop = st;
        d.apply(Op::SetSettings { settings: Box::new(s) }).unwrap();
        assert_eq!(d.p.settings.hazop.nodes[0].rows[0].status, "Open");
        assert_eq!(crate::lists::build(&d.p, "hazop_recs").unwrap().rows.len(), 1);
        d.apply(Op::SetTag { id: tag(&d.p, "P-101A"), tag: "P-111A".into() }).unwrap();
        let r = &d.p.settings.hazop.nodes[0].rows[0];
        assert!(r.causes.contains("P-111A") && !r.causes.contains("P-101A") && r.rec.contains("P-111A"), "{r:?}");
        // P-101B is not renamed by P-101 → something (whole tags only).
        assert_eq!(replace_tag("P-101B and P-101", "P-101", "P-9"), "P-101B and P-9");
        // Drawn on the sheet and kept through save and open.
        assert!(prims(&d.p, d.p.sheets[0].id).iter().any(|x| matches!(x, symbols::Prim::Text { s, .. } if s.starts_with("N1 "))));
        let path = std::env::temp_dir().join(format!("hz-{}.dschema", std::process::id()));
        crate::file::write(&path, &d.p, &[]).unwrap();
        let (back, _, w) = crate::file::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(back.settings.hazop, d.p.settings.hazop);
        // A deleted symbol leaves the node.
        let pid = tag(&d.p, "PSV-101");
        d.apply(Op::Delete { ids: vec![pid], keep: false }).unwrap();
        assert!(!d.p.settings.hazop.nodes[0].ids.contains(&pid));
        assert!(d.apply(Op::HazopNode { ids: vec![], name: String::new() }).is_err());
    }

    #[test]
    fn every_example_suggests_for_every_symbol_without_panic() {
        for ex in crate::examples::list().iter().map(|e| e.id) {
            let p = crate::examples::get(ex).unwrap();
            for it in &p.items {
                let n = Node { id: 1, name: "x".into(), ids: vec![it.id], ..Node::default() };
                let rows = suggest(&p, &n);
                assert!(rows.len() >= 5, "{ex}");
            }
        }
    }
}
