//! Connectivity: which items each line joins, lines branching off lines, line runs (a pipeline
//! with one line number, followed through in-line items and across sheets through off-page
//! connectors), from/to for the lists, and walks along the piping for the checks.

use crate::model::{End, Item, Line, Project};
use crate::symbols::{self, dir_vec, Cat, Place, Sym};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

/// A port's position and outward direction on the sheet.
pub fn port_pos(it: &Item, k: u8) -> Option<([f64; 2], u8)> {
    let s = symbols::get(&it.sym)?;
    let p = s.ports.get(k as usize)?;
    let pl = Place::of(it);
    Some((pl.pt([p.x, p.y]), pl.dir(p.dir)))
}

/// Where a line end is on the sheet.
pub fn end_pt(p: &Project, e: &End) -> [f64; 2] {
    match e {
        End::Port { item, port } => p.item(*item).and_then(|it| port_pos(it, *port)).map_or([0.0, 0.0], |x| x.0),
        End::Line { x, y, .. } | End::Free { x, y } => [*x, *y],
    }
}

/// The direction a line leaves an end (a port's outward direction), if it has one.
pub fn end_dir(p: &Project, e: &End) -> Option<[f64; 2]> {
    match e {
        End::Port { item, port } => p.item(*item).and_then(|it| port_pos(it, *port)).map(|x| dir_vec(x.1)),
        _ => None,
    }
}

/// A process-type line with an end on an instrument bubble (or a thermowell) is the instrument's connection to the
/// process (an impulse line), not a pipeline: it has no line number and is not in the line list.
pub fn instrument_connection(p: &Project, l: &Line) -> bool {
    [&l.a, &l.b].iter().any(|e| e.item().and_then(|(i, _)| p.item(i)).and_then(|i| symbols::get(&i.sym)).is_some_and(|s| s.cat == Cat::Instrument || (s.cat == Cat::Primary && !s.inline)))
}

/// The formatted line number of a line ("" for signal lines and lines with nothing filled in).
pub fn number(p: &Project, l: &Line) -> String {
    if !l.kind.process() || instrument_connection(p, l) {
        return String::new();
    }
    let get = |k: &str| l.data.get(k).map(|v| v.text().trim().to_string()).unwrap_or_default();
    let typed = get("number");
    if !typed.is_empty() {
        return typed;
    }
    let (size, fluid, seq, class) = (get("size"), get("fluid"), get("seq"), get("class"));
    if size.is_empty() && fluid.is_empty() && seq.is_empty() && class.is_empty() {
        return String::new();
    }
    let size = if size.is_empty() { "?".into() } else { format!("{}\"", size.trim_end_matches('"')) };
    let or = |s: String| if s.is_empty() { "?".to_string() } else { s };
    p.settings.line_format.replace("<size>", &size).replace("<fluid>", &or(fluid)).replace("<seq>", &or(seq)).replace("<class>", &or(class)).replace("<unit>", &p.title.unit)
}

/// The piping network of a project.
pub struct Net<'a> {
    pub p: &'a Project,
    pub items: HashMap<u32, &'a Item>,
    pub lines: HashMap<u32, &'a Line>,
    /// Lines on each item port: (line, which end: false = a, true = b).
    pub at_port: HashMap<(u32, u8), Vec<(u32, bool)>>,
    /// Lines that end on each line (branches), with which end.
    pub branches: HashMap<u32, Vec<(u32, bool)>>,
    /// Each process line's run, and the runs (lines in order of id).
    pub run_of: HashMap<u32, usize>,
    pub runs: Vec<Vec<u32>>,
    pub numbers: HashMap<u32, String>,
}

struct Dsu(Vec<usize>);
impl Dsu {
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.0[r] != r {
            r = self.0[r];
        }
        let mut y = x;
        while self.0[y] != r {
            let n = self.0[y];
            self.0[y] = r;
            y = n;
        }
        r
    }
    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a.max(b)] = a.min(b);
        }
    }
}

impl<'a> Net<'a> {
    pub fn new(p: &'a Project) -> Self {
        let items: HashMap<u32, &Item> = p.items.iter().map(|i| (i.id, i)).collect();
        let lines: HashMap<u32, &Line> = p.lines.iter().map(|l| (l.id, l)).collect();
        let mut at_port: HashMap<(u32, u8), Vec<(u32, bool)>> = HashMap::new();
        let mut branches: HashMap<u32, Vec<(u32, bool)>> = HashMap::new();
        for l in &p.lines {
            for (e, b) in [(&l.a, false), (&l.b, true)] {
                match e {
                    End::Port { item, port } => at_port.entry((*item, *port)).or_default().push((l.id, b)),
                    End::Line { line, .. } => branches.entry(*line).or_default().push((l.id, b)),
                    End::Free { .. } => {}
                }
            }
        }
        let numbers: HashMap<u32, String> = p.lines.iter().map(|l| (l.id, number(p, l))).collect();
        // Runs: process lines joined through in-line items and off-page partners when their numbers agree.
        let proc: Vec<&Line> = p.lines.iter().filter(|l| l.kind.process() && !instrument_connection(p, l)).collect();
        let idx: HashMap<u32, usize> = proc.iter().enumerate().map(|(i, l)| (l.id, i)).collect();
        let mut d = Dsu((0..proc.len()).collect());
        let process_lines = |item: u32, port: u8| -> Vec<u32> { at_port.get(&(item, port)).map(|v| v.iter().map(|x| x.0).filter(|l| idx.contains_key(l)).collect()).unwrap_or_default() };
        for it in &p.items {
            let Some(s) = symbols::get(&it.sym) else { continue };
            let mut joined: Vec<u32> = vec![];
            if s.inline {
                for (k, port) in s.ports.iter().enumerate() {
                    if !port.signal {
                        joined.extend(process_lines(it.id, k as u8));
                    }
                }
            } else if s.cat == Cat::Connector && it.link != 0 && it.id < it.link {
                joined.extend(process_lines(it.id, 0));
                joined.extend(process_lines(it.link, 0));
            }
            for w in joined.windows(2) {
                if numbers[&w[0]] == numbers[&w[1]] {
                    d.join(idx[&w[0]], idx[&w[1]]);
                }
            }
        }
        let mut groups: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
        for (i, l) in proc.iter().enumerate() {
            groups.entry(d.find(i)).or_default().push(l.id);
        }
        let mut runs: Vec<Vec<u32>> = groups.into_values().collect();
        runs.iter_mut().for_each(|r| r.sort_unstable());
        runs.sort_by_key(|r| r[0]);
        let run_of = runs.iter().enumerate().flat_map(|(i, r)| r.iter().map(move |l| (*l, i))).collect();
        Self { p, items, lines, at_port, branches, run_of, runs, numbers }
    }

    pub fn sym(&self, item: u32) -> Option<&'static Sym> {
        self.items.get(&item).and_then(|i| symbols::get(&i.sym))
    }

    /// Lines on an item's port.
    pub fn on_port(&self, item: u32, port: u8) -> &[(u32, bool)] {
        self.at_port.get(&(item, port)).map_or(&[], |v| v.as_slice())
    }

    /// The other end of a line, seen from one end.
    pub fn other(&self, line: u32, from_b: bool) -> Option<&End> {
        self.lines.get(&line).map(|l| if from_b { &l.a } else { &l.b })
    }

    /// Does fluid flow out of this end of the line (towards the other end)?
    pub fn flows_from(&self, line: u32, at_b: bool) -> bool {
        self.lines.get(&line).is_some_and(|l| at_b == l.flip)
    }

    /// The far side of an in-line item (the other process port), or an off-page partner's port.
    fn through(&self, item: u32, port: u8) -> Option<(u32, u8)> {
        let it = self.items.get(&item)?;
        let s = symbols::get(&it.sym)?;
        if s.inline {
            let k = s.ports.iter().enumerate().find(|(k, p)| !p.signal && *k as u8 != port)?.0;
            return Some((item, k as u8));
        }
        if s.cat == Cat::Connector && it.link != 0 {
            return Some((it.link, 0));
        }
        None
    }

    /// Items reached from an item's port along process lines, passing through in-line items and
    /// off-page pairs (and, with `branch`, onto and off other lines), until `stop` says to stop at
    /// an item. Returns each item reached with the port it was reached on and how many items were
    /// passed on the way, nearest first.
    pub fn reach(&self, item: u32, port: u8, branch: bool, max: usize, stop: impl Fn(&Item, &Sym) -> bool) -> Vec<(u32, u8, usize)> {
        let mut out = vec![];
        let mut seen_lines = HashSet::new();
        let mut seen_items = HashSet::from([item]);
        let mut q: VecDeque<(u32, u8, usize)> = VecDeque::from([(item, port, 0)]);
        // From a port: every line on it, to its far end; with `branch`, also every line joined to
        // it at a branch point (entered there, so walked to its far end, or entered mid-line and
        // walked both ways).
        while let Some((it, pt, depth)) = q.pop_front() {
            let mut lines: Vec<(u32, Option<bool>)> = self.on_port(it, pt).iter().map(|&(l, b)| (l, Some(b))).collect();
            let mut k = 0;
            while k < lines.len() {
                let (l, entered) = lines[k];
                k += 1;
                let Some(line) = self.lines.get(&l) else { continue };
                if !line.kind.process() || !seen_lines.insert(l) {
                    continue;
                }
                if branch {
                    lines.extend(self.branches.get(&l).into_iter().flatten().map(|&(bl, bb)| (bl, Some(bb))));
                    for e in [&line.a, &line.b] {
                        if let End::Line { line: m, .. } = e {
                            lines.push((*m, None));
                        }
                    }
                }
                let ends: Vec<&End> = match entered { Some(at_b) => vec![if at_b { &line.a } else { &line.b }], None => vec![&line.a, &line.b] };
                for e in ends {
                    if let End::Port { item: n, port: np } = *e {
                        if !seen_items.insert(n) {
                            continue;
                        }
                        let (Some(ni), Some(ns)) = (self.items.get(&n), self.sym(n)) else { continue };
                        out.push((n, np, depth));
                        if stop(ni, ns) || depth + 1 >= max {
                            continue;
                        }
                        if let Some((ti, tp)) = self.through(n, np) {
                            seen_items.insert(ti);
                            q.push_back((ti, tp, depth + 1));
                        }
                    }
                }
            }
        }
        out
    }

    /// What one end of a run connects to, in words ("P-101A discharge", "line 3\"-P-1002-A1A",
    /// "open end").
    pub fn describe(&self, e: &End) -> String {
        match e {
            End::Port { item, port } => {
                let Some(it) = self.items.get(item) else { return String::new() };
                let s = symbols::get(&it.sym);
                let what = if it.tag.is_empty() { s.map_or("", |s| s.name).to_string() } else { it.tag.clone() };
                if let (Some(s), true) = (s, s.is_some_and(|s| s.cat == Cat::Connector)) {
                    let _ = s;
                    let partner = self.items.get(&it.link).map(|q| format!("sheet {}", self.p.sheet(q.sheet).map_or(String::new(), |sh| sh.number.clone())));
                    return partner.map_or_else(|| "off-page connector".into(), |x| format!("from/to {x}"));
                }
                let pn = s.and_then(|s| s.ports.get(*port as usize)).map_or("", |p| p.name);
                if s.is_some_and(|s| s.ports.len() > 1 && !s.inline) && !pn.is_empty() { format!("{what} {pn}") } else { what }
            }
            End::Line { line, .. } => {
                let n = self.numbers.get(line).cloned().unwrap_or_default();
                if n.is_empty() { "a branch point".into() } else { format!("line {n}") }
            }
            End::Free { .. } => "open end".into(),
        }
    }

    /// The two ends of a run in flow order: (from, to). The ends are the line ends that are not
    /// joined to another line of the same run.
    pub fn run_ends(&self, run: usize) -> (Option<End>, Option<End>) {
        let lines = &self.runs[run];
        let set: HashSet<u32> = lines.iter().copied().collect();
        let mut ends = vec![];
        for &l in lines {
            let line = self.lines[&l];
            for (e, at_b) in [(&line.a, false), (&line.b, true)] {
                let inner = match e {
                    End::Port { item, port } => self.through(*item, *port).is_some_and(|(ti, tp)| self.on_port(ti, tp).iter().any(|x| set.contains(&x.0))),
                    _ => false,
                };
                if !inner {
                    ends.push((e.clone(), self.flows_from(l, at_b)));
                }
            }
        }
        let from = ends.iter().find(|x| x.1).or(ends.first()).map(|x| x.0.clone());
        let to = ends.iter().find(|x| !x.1 && Some(&x.0) != from.as_ref()).or(ends.iter().find(|x| Some(&x.0) != from.as_ref())).map(|x| x.0.clone());
        (from, to)
    }

    /// The run's hydraulic service from its connections: pump suction or discharge, if any.
    pub fn pump_side(&self, run: usize) -> Option<&'static str> {
        for &l in &self.runs[run] {
            let line = self.lines[&l];
            for e in [&line.a, &line.b] {
                if let End::Port { item, port } = e {
                    let Some(s) = self.sym(*item) else { continue };
                    if s.key.starts_with("pump") {
                        match s.ports.get(*port as usize).map(|p| p.name) {
                            Some("suction") => return Some("suction"),
                            Some("discharge") => return Some("discharge"),
                            _ => {}
                        }
                    }
                }
            }
        }
        None
    }
}

/// A line's route on the sheet is worked out by `view`; for the graph a straight segment between
/// the ends is enough to find the nearest point.
pub fn nearest_on(path: &[[f64; 2]], q: [f64; 2]) -> ([f64; 2], usize, f64) {
    let mut best = (path.first().copied().unwrap_or(q), 0, f64::INFINITY);
    for (i, w) in path.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx * dx + dy * dy;
        let t = if l > 0.0 { (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / l).clamp(0.0, 1.0) } else { 0.0 };
        let p = [a[0] + t * dx, a[1] + t * dy];
        let d = (p[0] - q[0]).hypot(p[1] - q[1]);
        if d < best.2 {
            best = (p, i, d);
        }
    }
    best
}

/// The off-page connectors of a sheet that have no partner, or whose partner is on the same sheet.
pub fn lonely_connectors(p: &Project) -> Vec<u32> {
    p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| s.cat == Cat::Connector) && (i.link == 0 || p.item(i.link).is_none_or(|q| q.sheet == i.sheet))).map(|i| i.id).collect()
}

/// The direction vector of a port on the sheet.
pub fn port_dir(it: &Item, k: u8) -> Option<[f64; 2]> {
    port_pos(it, k).map(|x| dir_vec(x.1))
}
