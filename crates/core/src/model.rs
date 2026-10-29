//! The project: sheets, items (placed symbols), lines (drawn port to port), notes and settings,
//! and the `.dschema` file (JSON, versioned). Positions are millimetres on the sheet, y down, so a
//! drawing prints at its true size. Engineering values are SI base units (Pa, K, m³/s, kg/s,
//! kg/m³, Pa·s, m, W); the window shows them in SI or US units.
//!
//! Damaged files open as far as possible: every sheet, item, line and note is read on its own, a
//! broken one is left out with a warning, and `normalize` repairs what remains (ids, references,
//! impossible numbers).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// The file format written by this version. Older formats are migrated when read.
pub const FORMAT: u32 = 1;
/// Coordinates are kept inside ±this many millimetres (a few times the largest sheet).
pub const MAX_COORD: f64 = 100_000.0;
/// The most of any one kind of thing a file may hold (far more than any real drawing).
pub const MAX_COUNT: usize = 200_000;

/// A value in an item's or line's data: a number (SI base units) or text.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Val {
    Num(f64),
    Text(String),
}

impl Val {
    pub fn num(&self) -> Option<f64> {
        match self {
            Val::Num(v) if v.is_finite() => Some(*v),
            Val::Text(t) => t.trim().parse().ok().filter(|v: &f64| v.is_finite()),
            _ => None,
        }
    }
    pub fn text(&self) -> String {
        match self {
            Val::Num(v) => crate::fmt_num(*v),
            Val::Text(t) => t.clone(),
        }
    }
}

pub type Data = BTreeMap<String, Val>;

/// The title block fields shared by every sheet.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Title {
    pub company: String,
    pub project: String,
    pub client: String,
    pub unit: String,
    pub by: String,
    pub checked: String,
    pub approved: String,
}

/// One row of a sheet's revision table.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Rev {
    pub rev: String,
    pub date: String,
    pub desc: String,
    pub by: String,
    pub checked: String,
    pub approved: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Sheet {
    pub id: u32,
    /// Drawing number, e.g. "PID-001".
    pub number: String,
    pub title: String,
    /// A3, A1, ANSI B or ANSI D (landscape).
    pub size: String,
    pub rev: String,
    pub date: String,
    pub scale: String,
    pub revs: Vec<Rev>,
    /// The legend sheet: drawn automatically with every symbol the project uses.
    pub legend: bool,
}

impl Default for Sheet {
    fn default() -> Self {
        Self { id: 0, number: "PID-001".into(), title: "Piping and instrument diagram".into(), size: "A3".into(), rev: "A".into(), date: String::new(), scale: "NTS".into(), revs: vec![], legend: false }
    }
}

/// Paper sizes (landscape), millimetres.
pub const SIZES: &[(&str, f64, f64)] = &[("A3", 420.0, 297.0), ("A1", 841.0, 594.0), ("ANSI B", 431.8, 279.4), ("ANSI D", 863.6, 558.8)];

impl Sheet {
    pub fn dims(&self) -> (f64, f64) {
        SIZES.iter().find(|s| s.0 == self.size).map_or((420.0, 297.0), |s| (s.1, s.2))
    }
}

/// A placed symbol: equipment, a valve, an instrument, a fitting, an off-page connector or a mark.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Item {
    pub id: u32,
    pub sheet: u32,
    /// The symbol's key in the library (`symbols::get`).
    pub sym: String,
    pub x: f64,
    pub y: f64,
    /// Quarter turns clockwise, 0–3.
    pub rot: u8,
    /// Mirrored left to right (before the rotation).
    pub flip: bool,
    pub scale: f64,
    pub tag: String,
    pub data: Data,
    /// Off-page connectors: the partner connector's id (0: none yet).
    pub link: u32,
}

impl Default for Item {
    fn default() -> Self {
        Self { id: 0, sheet: 0, sym: String::new(), x: 0.0, y: 0.0, rot: 0, flip: false, scale: 1.0, tag: String::new(), data: Data::new(), link: 0 }
    }
}

/// Line types (ISO 10628-2 for process lines, ISA-5.1 for signals).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Major,
    Minor,
    Utility,
    Pneumatic,
    Electric,
    Data,
    Capillary,
}

impl Kind {
    pub const ALL: [Kind; 7] = [Kind::Major, Kind::Minor, Kind::Utility, Kind::Pneumatic, Kind::Electric, Kind::Data, Kind::Capillary];
    /// Process lines carry fluid (and a line number); the others carry signals.
    pub fn process(self) -> bool {
        matches!(self, Kind::Major | Kind::Minor | Kind::Utility)
    }
    pub fn name(self) -> &'static str {
        match self {
            Kind::Major => "Major process",
            Kind::Minor => "Minor process",
            Kind::Utility => "Utility",
            Kind::Pneumatic => "Pneumatic signal",
            Kind::Electric => "Electrical signal",
            Kind::Data => "Software or data link",
            Kind::Capillary => "Capillary",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Kind::Major => "major",
            Kind::Minor => "minor",
            Kind::Utility => "utility",
            Kind::Pneumatic => "pneumatic",
            Kind::Electric => "electric",
            Kind::Data => "data",
            Kind::Capillary => "capillary",
        }
    }
}

/// Where a line ends: on a symbol's port, on another line (a branch), or nowhere (an open end).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "at", rename_all = "lowercase")]
pub enum End {
    Port { item: u32, port: u8 },
    Line { line: u32, x: f64, y: f64 },
    Free { x: f64, y: f64 },
}

impl Default for End {
    fn default() -> Self {
        End::Free { x: 0.0, y: 0.0 }
    }
}

impl End {
    pub fn item(&self) -> Option<(u32, u8)> {
        match self {
            End::Port { item, port } => Some((*item, *port)),
            _ => None,
        }
    }
}

/// A drawn line from end `a` to end `b`. Flow runs a → b unless `flip`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Line {
    pub id: u32,
    pub sheet: u32,
    pub kind: Kind,
    pub a: End,
    pub b: End,
    /// Bend points the user placed; the route passes through them. Empty: routed automatically.
    pub pts: Vec<[f64; 2]>,
    pub flip: bool,
    pub data: Data,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    #[default]
    Note,
    Leader,
    Hold,
}

/// Free text, a note with a leader to a point, or a "Hold" cloud round an area that is not final.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Note {
    pub id: u32,
    pub sheet: u32,
    pub kind: NoteKind,
    pub x: f64,
    pub y: f64,
    /// The cloud's size (Hold) or the text's width (0: as long as the text).
    pub w: f64,
    pub h: f64,
    pub text: String,
    /// The leader's arrow point (Leader).
    pub to: [f64; 2],
}

/// Velocity and pressure-drop limits for one kind of service (SI: m/s, Pa per 100 m).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Limit {
    pub service: String,
    pub name: String,
    pub vmin: f64,
    pub vmax: f64,
    pub dp: f64,
}

impl Default for Limit {
    fn default() -> Self {
        Self { service: String::new(), name: String::new(), vmin: 0.0, vmax: 3.0, dp: 40_000.0 }
    }
}

/// Typical limits by service (editable in Options): velocity range and pressure drop per 100 m.
pub fn default_limits() -> Vec<Limit> {
    let l = |s: &str, n: &str, vmin: f64, vmax: f64, dp_kpa: f64| Limit { service: s.into(), name: n.into(), vmin, vmax, dp: dp_kpa * 1000.0 };
    vec![
        l("liquid", "Liquid, general", 0.5, 3.0, 40.0),
        l("suction", "Pump suction", 0.3, 1.5, 25.0),
        l("discharge", "Pump discharge", 1.0, 3.5, 45.0),
        l("drain", "Gravity drain", 0.3, 1.0, 5.0),
        l("gas", "Gas or vapour", 3.0, 20.0, 20.0),
        l("steam", "Steam", 10.0, 35.0, 25.0),
        l("twophase", "Two-phase", 3.0, 15.0, 50.0),
    ]
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// The line number format; tokens <size>, <fluid>, <seq>, <class>, <unit>.
    pub line_format: String,
    /// Check rules switched off (numbers 1–13).
    pub off: Vec<u8>,
    pub limits: Vec<Limit>,
    /// Pipe roughness for the friction factor (m); 0.045 mm is commercial steel.
    pub roughness: f64,
    /// Snap step on the sheet (mm).
    pub grid: f64,
    /// Hidden layers: process, utility, instrument, notes.
    pub hidden: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { line_format: "<size>-<fluid>-<seq>-<class>".into(), off: vec![], limits: default_limits(), roughness: 4.5e-5, grid: 2.5, hidden: vec![] }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Project {
    pub format: u32,
    pub name: String,
    /// "SI" or "US": how values are shown and typed. The file always keeps SI units.
    pub units: String,
    pub title: Title,
    pub sheets: Vec<Sheet>,
    pub items: Vec<Item>,
    pub lines: Vec<Line>,
    pub notes: Vec<Note>,
    pub settings: Settings,
    /// The next id for anything (sheets, items, lines and notes share one sequence).
    pub next: u32,
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

impl Project {
    /// A new project with one blank A3 sheet.
    pub fn new() -> Self {
        let mut p = Self { format: FORMAT, name: "Project1".into(), units: "SI".into(), title: Title::default(), sheets: vec![], items: vec![], lines: vec![], notes: vec![], settings: Settings::default(), next: 1 };
        let id = p.id();
        p.sheets.push(Sheet { id, ..Sheet::default() });
        p
    }

    /// A fresh id.
    pub fn id(&mut self) -> u32 {
        let id = self.next.max(1);
        self.next = id.saturating_add(1);
        id
    }

    pub fn item(&self, id: u32) -> Option<&Item> {
        self.items.iter().find(|x| x.id == id)
    }
    pub fn line(&self, id: u32) -> Option<&Line> {
        self.lines.iter().find(|x| x.id == id)
    }
    pub fn sheet(&self, id: u32) -> Option<&Sheet> {
        self.sheets.iter().find(|x| x.id == id)
    }
    /// The sheet's position, 1 for the first.
    pub fn sheet_no(&self, id: u32) -> usize {
        self.sheets.iter().position(|s| s.id == id).map_or(0, |i| i + 1)
    }
    pub fn rule_on(&self, n: u8) -> bool {
        !self.settings.off.contains(&n)
    }

    /// Repairs what a file, an import or a random edit may have broken: ids unique and non-zero,
    /// references pointing at things that exist, numbers finite and in range, at least one sheet.
    /// Returns what it changed, in plain words.
    pub fn normalize(&mut self) -> Vec<String> {
        let mut w = vec![];
        let fix = |v: &mut f64, lo: f64, hi: f64, def: f64| {
            if !v.is_finite() {
                *v = def;
            }
            *v = v.clamp(lo, hi);
        };
        if self.units != "SI" && self.units != "US" {
            self.units = "SI".into();
        }
        for (what, n) in [("sheets", self.sheets.len()), ("items", self.items.len()), ("lines", self.lines.len()), ("notes", self.notes.len())] {
            if n > MAX_COUNT {
                w.push(format!("The file had {n} {what}; only the first {MAX_COUNT} were kept."));
            }
        }
        self.sheets.truncate(MAX_COUNT);
        self.items.truncate(MAX_COUNT);
        self.lines.truncate(MAX_COUNT);
        self.notes.truncate(MAX_COUNT);
        // Unique, non-zero ids across everything; the next id above them all.
        let mut seen = HashSet::new();
        let mut dup = 0;
        let mut max = 0u32;
        let ids: Vec<u32> = self.sheets.iter().map(|x| x.id).chain(self.items.iter().map(|x| x.id)).chain(self.lines.iter().map(|x| x.id)).chain(self.notes.iter().map(|x| x.id)).collect();
        for &i in &ids {
            max = max.max(i);
        }
        let mut next = max.saturating_add(1);
        let mut fresh = |id: &mut u32, seen: &mut HashSet<u32>| {
            if *id == 0 || !seen.insert(*id) {
                if next == u32::MAX {
                    return false;
                }
                *id = next;
                next += 1;
                seen.insert(*id);
                dup += 1;
            }
            true
        };
        self.sheets.retain_mut(|s| fresh(&mut s.id, &mut seen));
        self.items.retain_mut(|s| fresh(&mut s.id, &mut seen));
        self.lines.retain_mut(|s| fresh(&mut s.id, &mut seen));
        self.notes.retain_mut(|s| fresh(&mut s.id, &mut seen));
        if dup > 0 {
            w.push(format!("{dup} repeated or missing ids were renumbered."));
        }
        self.next = self.next.max(next);
        if self.sheets.is_empty() {
            let id = self.id();
            self.sheets.push(Sheet { id, ..Sheet::default() });
            w.push("The file had no sheets; a blank one was added.".into());
        }
        for s in &mut self.sheets {
            if !SIZES.iter().any(|z| z.0 == s.size) {
                s.size = "A3".into();
            }
            s.revs.truncate(50);
        }
        let sheets: HashSet<u32> = self.sheets.iter().map(|s| s.id).collect();
        let first = self.sheets[0].id;
        let mut moved = 0;
        for it in &mut self.items {
            if !sheets.contains(&it.sheet) {
                it.sheet = first;
                moved += 1;
            }
            fix(&mut it.x, -MAX_COORD, MAX_COORD, 0.0);
            fix(&mut it.y, -MAX_COORD, MAX_COORD, 0.0);
            fix(&mut it.scale, 0.25, 4.0, 1.0);
            it.rot %= 4;
            it.data.retain(|_, v| !matches!(v, Val::Num(x) if !x.is_finite()));
            if crate::symbols::get(&it.sym).is_none() {
                w.push(format!("Symbol \"{}\" is not in this version's library; it is shown as a box.", crate::clip(&it.sym, 40)));
                it.sym = "unknown".into();
            }
        }
        let items: BTreeMap<u32, (u32, usize)> = self.items.iter().map(|i| (i.id, (i.sheet, crate::symbols::get(&i.sym).map_or(0, |s| s.ports.len())))).collect();
        for it in &mut self.items {
            if it.link != 0 && (it.link == it.id || !items.contains_key(&it.link)) {
                it.link = 0;
            }
        }
        // Partners point at each other.
        let links: BTreeMap<u32, u32> = self.items.iter().filter(|i| i.link != 0).map(|i| (i.id, i.link)).collect();
        for it in &mut self.items {
            if it.link != 0 && links.get(&it.link) != Some(&it.id) {
                it.link = 0;
            }
        }
        for n in &mut self.notes {
            if !sheets.contains(&n.sheet) {
                n.sheet = first;
                moved += 1;
            }
            let [tx, ty] = &mut n.to;
            for v in [&mut n.x, &mut n.y, tx, ty] {
                fix(v, -MAX_COORD, MAX_COORD, 0.0);
            }
            fix(&mut n.w, 0.0, MAX_COORD, 0.0);
            fix(&mut n.h, 0.0, MAX_COORD, 0.0);
            if n.text.len() > 4000 {
                n.text = crate::clip(&n.text, 4000);
            }
        }
        let line_ids: BTreeMap<u32, u32> = self.lines.iter().map(|l| (l.id, l.sheet)).collect();
        let mut loose = 0;
        for l in &mut self.lines {
            if !sheets.contains(&l.sheet) {
                l.sheet = first;
                moved += 1;
            }
            let sheet = l.sheet;
            let id = l.id;
            for e in [&mut l.a, &mut l.b] {
                let ok = match e {
                    End::Port { item, port } => items.get(item).is_some_and(|(s, n)| *s == sheet && (*port as usize) < *n),
                    End::Line { line, x, y } => {
                        fix(x, -MAX_COORD, MAX_COORD, 0.0);
                        fix(y, -MAX_COORD, MAX_COORD, 0.0);
                        *line != id && line_ids.get(line) == Some(&sheet)
                    }
                    End::Free { x, y } => {
                        fix(x, -MAX_COORD, MAX_COORD, 0.0);
                        fix(y, -MAX_COORD, MAX_COORD, 0.0);
                        true
                    }
                };
                if !ok {
                    *e = End::Free { x: 0.0, y: 0.0 };
                    loose += 1;
                }
            }
            l.pts.truncate(200);
            for p in &mut l.pts {
                fix(&mut p[0], -MAX_COORD, MAX_COORD, 0.0);
                fix(&mut p[1], -MAX_COORD, MAX_COORD, 0.0);
            }
            l.data.retain(|_, v| !matches!(v, Val::Num(x) if !x.is_finite()));
        }
        if moved > 0 {
            w.push(format!("{moved} things on a missing sheet were moved to the first sheet."));
        }
        if loose > 0 {
            w.push(format!("{loose} line ends pointed at something missing and were left open."));
        }
        let s = &mut self.settings;
        fix(&mut s.roughness, 0.0, 0.01, 4.5e-5);
        fix(&mut s.grid, 0.5, 20.0, 2.5);
        s.off.retain(|n| (1..=13).contains(n));
        s.off.sort_unstable();
        s.off.dedup();
        if s.limits.is_empty() {
            s.limits = default_limits();
        }
        for l in &mut s.limits {
            fix(&mut l.vmin, 0.0, 1000.0, 0.0);
            fix(&mut l.vmax, 0.0, 1000.0, 3.0);
            fix(&mut l.dp, 0.0, 1e9, 40_000.0);
        }
        if s.line_format.trim().is_empty() || s.line_format.len() > 200 {
            s.line_format = Settings::default().line_format;
        }
        self.format = FORMAT;
        w
    }
}

/// Reads a `.dschema` file: each part on its own, so a damaged one is left out instead of the
/// whole file failing. Returns the project and what was repaired.
pub fn from_json(text: &str) -> Result<(Project, Vec<String>), String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("This is not a Datum Schema file (it is not valid JSON: {e})."))?;
    let o = v.as_object().ok_or("This is not a Datum Schema file.")?;
    let format = o.get("format").and_then(|f| f.as_u64()).unwrap_or(0);
    if format > FORMAT as u64 {
        return Err(format!("This file was saved by a newer Datum Schema (format {format}). Update Datum Schema to open it."));
    }
    let mut w = vec![];
    let mut p = Project { sheets: vec![], ..Project::new() };
    let part = |key: &str, w: &mut Vec<String>| -> Option<serde_json::Value> {
        let x = o.get(key)?;
        Some(x.clone()).filter(|x| !x.is_null()).or_else(|| {
            w.push(format!("The file's {key} were empty."));
            None
        })
    };
    if let Some(n) = o.get("name").and_then(|n| n.as_str()) {
        p.name = crate::clip(n, 200);
    }
    if let Some(u) = o.get("units").and_then(|n| n.as_str()) {
        p.units = u.into();
    }
    p.next = o.get("next").and_then(|n| n.as_u64()).map_or(1, |n| n.min(u32::MAX as u64) as u32);
    if let Some(t) = part("title", &mut w) {
        p.title = serde_json::from_value(t).unwrap_or_else(|_| {
            w.push("The title block could not be read; it is blank.".into());
            Title::default()
        });
    }
    if let Some(s) = part("settings", &mut w) {
        p.settings = serde_json::from_value(s).unwrap_or_else(|_| {
            w.push("The project settings could not be read; the defaults are used.".into());
            Settings::default()
        });
    }
    fn list<T: serde::de::DeserializeOwned>(v: Option<serde_json::Value>, what: &str, w: &mut Vec<String>) -> Vec<T> {
        let Some(serde_json::Value::Array(a)) = v else { return vec![] };
        let n = a.len();
        let out: Vec<T> = a.into_iter().take(MAX_COUNT).filter_map(|x| serde_json::from_value(x).ok()).collect();
        if out.len() < n.min(MAX_COUNT) {
            w.push(format!("{} {what} could not be read and were left out.", n.min(MAX_COUNT) - out.len()));
        }
        out
    }
    p.sheets = list(part("sheets", &mut w), "sheets", &mut w);
    p.items = list(part("items", &mut w), "symbols", &mut w);
    p.lines = list(part("lines", &mut w), "lines", &mut w);
    p.notes = list(part("notes", &mut w), "notes", &mut w);
    w.extend(p.normalize());
    Ok((p, w))
}

pub fn to_json(p: &Project) -> String {
    serde_json::to_string_pretty(p).unwrap_or_default()
}
