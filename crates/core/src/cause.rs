//! The cause-and-effect matrix, read from the P&ID: every interlock (the interlock logic symbol)
//! with the instruments signalled into it (the causes: switches and transmitters with their trip
//! settings) and the equipment and valves it acts on (the effects: stop a drive, close or open a
//! valve). A switch wired straight to a final element is an interlock of its own. Rows are causes,
//! columns are effects, and a cell names the interlock that links them.

use crate::graph::Net;
use crate::model::{End, Item, Project};
use crate::symbols::{self, Cat};
use crate::tags;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Cause {
    pub obj: u32,
    pub item: u32,
    pub sheet: u32,
    pub tag: String,
    /// What trips: the instrument's function, or its letters in words ("Level high-high").
    pub what: String,
    /// The trip setting (the instrument's set points).
    pub setting: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Effect {
    pub obj: u32,
    pub item: u32,
    pub sheet: u32,
    pub tag: String,
    pub what: String,
    /// Stop, Close, Open, Hold or Trip.
    pub action: String,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Matrix {
    pub causes: Vec<Cause>,
    pub effects: Vec<Effect>,
    /// cells[cause][effect]: the interlocks that link them ("UY-202"); empty: none.
    pub cells: Vec<Vec<String>>,
    /// Interlocks with no cause or no effect drawn, in words.
    pub open: Vec<String>,
}

/// What a final element does on a trip: a drive stops; a valve goes to its fail position.
fn action(p: &Project, it: &Item) -> String {
    let d = p.data_of(it.obj);
    if let Some(a) = d.get("trip_action").map(|v| v.text()).filter(|s| !s.trim().is_empty()) {
        return a;
    }
    match symbols::get(&it.sym).map(|s| s.cat) {
        Some(Cat::Rotating) => "Stop".into(),
        Some(c) if c.valve() || c == Cat::Control => match d.get("fail").map(|v| v.text()).as_deref() {
            Some("FC") => "Close".into(),
            Some("FO") => "Open".into(),
            Some("FL") => "Hold".into(),
            _ => "Close".into(),
        },
        _ => "Trip".into(),
    }
}

/// An effect: something an interlock acts on (valves, drives, equipment, final elements).
fn is_effect(p: &Project, it: &Item) -> bool {
    let Some(s) = symbols::get(&it.sym) else { return false };
    let letters = tags::loop_of(p.tag_of(it)).0;
    s.cat.valve() || s.cat == Cat::Control && !it.base_key().starts_with("psv") || s.cat.equipment() || matches!(s.cat, Cat::Rotating) || tags::is_final(&letters)
}

/// An instrument's trip in words, from its letters: LSHH is "Level high-high".
fn words(letters: &str) -> String {
    let var = match letters.chars().next() {
        Some('A') => "Analysis",
        Some('F') => "Flow",
        Some('L') => "Level",
        Some('P') => "Pressure",
        Some('T') => "Temperature",
        Some('S') => "Speed",
        Some('V') => "Vibration",
        Some('Z') => "Position",
        Some('B') => "Flame",
        Some('H') => "Hand",
        _ => "Trip",
    };
    let tail: String = letters.chars().skip(1).collect();
    let lim = if tail.ends_with("HH") { " high-high" } else if tail.ends_with("LL") { " low-low" } else if tail.ends_with('H') { " high" } else if tail.ends_with('L') { " low" } else { "" };
    format!("{var}{lim}")
}

fn is_interlock(it: &Item) -> bool {
    it.base_key() == "inst_interlock"
}

/// A switch: an instrument whose letters after the first hold an S (LSHH, PSL…), not a safety valve.
fn is_switch(p: &Project, it: &Item) -> bool {
    let l = tags::loop_of(p.tag_of(it)).0;
    symbols::get(&it.sym).is_some_and(|s| s.cat == Cat::Instrument) && l.chars().count() >= 2 && l.chars().skip(1).any(|c| c == 'S') && !l.ends_with('V')
}

/// The items joined to an item by signal lines (any port, either end).
fn signalled<'a>(net: &'a Net, it: &Item) -> Vec<&'a Item> {
    let Some(s) = symbols::get(&it.sym) else { return vec![] };
    let mut v: Vec<&Item> = vec![];
    for k in 0..s.ports.len() {
        for &(l, b) in net.on_port(it.id, k as u8) {
            let Some(line) = net.lines.get(&l) else { continue };
            if line.kind.process() {
                continue;
            }
            if let End::Port { item, .. } = if b { &line.a } else { &line.b } {
                if let Some(o) = net.items.get(item) {
                    if o.id != it.id && !v.iter().any(|x| x.id == o.id) {
                        v.push(o);
                    }
                }
            }
        }
    }
    v
}

pub fn matrix(p: &Project) -> Matrix {
    let net = Net::new(p);
    // Each interlock: (its tag, causes, effects).
    let mut groups: Vec<(String, Vec<&Item>, Vec<&Item>)> = vec![];
    let mut seen = std::collections::HashSet::new();
    for it in p.items.iter().filter(|i| is_interlock(i) && seen.insert(i.obj)) {
        let (mut c, mut e) = (vec![], vec![]);
        for o in signalled(&net, it) {
            if is_interlock(o) {
                continue;
            }
            if is_effect(p, o) { e.push(o) } else { c.push(o) }
        }
        groups.push((p.tag_of(it).to_string(), c, e));
    }
    // A switch wired straight to a final element.
    for it in p.items.iter().filter(|i| is_switch(p, i)) {
        let e: Vec<&Item> = signalled(&net, it).into_iter().filter(|o| !is_interlock(o) && is_effect(p, o)).collect();
        if !e.is_empty() {
            groups.push((format!("{} (direct)", p.tag_of(it)), vec![it], e));
        }
    }
    let mut m = Matrix::default();
    let order = |t: &str| {
        let (a, n, _) = tags::split(t);
        (n, a, t.to_string())
    };
    for (name, c, e) in &groups {
        if c.is_empty() || e.is_empty() {
            m.open.push(format!("{name} has no {} drawn.", if c.is_empty() && e.is_empty() { "cause or effect" } else if c.is_empty() { "cause (an instrument signalled into it)" } else { "effect (a valve or drive it acts on)" }));
        }
        for it in c {
            if !m.causes.iter().any(|x| x.obj == it.obj) {
                let d = p.data_of(it.obj);
                let letters = tags::loop_of(p.tag_of(it)).0;
                let what = d.get("function").map(|v| v.text()).filter(|s| !s.is_empty()).unwrap_or_else(|| words(&letters));
                m.causes.push(Cause { obj: it.obj, item: it.id, sheet: it.sheet, tag: p.tag_of(it).to_string(), what, setting: d.get("setpoints").map(|v| v.text()).unwrap_or_default() });
            }
        }
        for it in e {
            if !m.effects.iter().any(|x| x.obj == it.obj) {
                let what = p.data_of(it.obj).get("service").map(|v| v.text()).filter(|s| !s.is_empty()).unwrap_or_else(|| symbols::get(&it.sym).map_or(String::new(), |s| s.name.to_string()));
                m.effects.push(Effect { obj: it.obj, item: it.id, sheet: it.sheet, tag: p.tag_of(it).to_string(), what, action: action(p, it) });
            }
        }
    }
    m.causes.sort_by_key(|c| order(&c.tag));
    m.effects.sort_by_key(|e| order(&e.tag));
    m.cells = m.causes.iter().map(|_| vec![String::new(); m.effects.len()]).collect();
    for (name, c, e) in &groups {
        for ci in c {
            for ei in e {
                let (Some(r), Some(k)) = (m.causes.iter().position(|x| x.obj == ci.obj), m.effects.iter().position(|x| x.obj == ei.obj)) else { continue };
                let cell = &mut m.cells[r][k];
                let label = name.trim_end_matches(" (direct)").to_string();
                let label = if name.ends_with("(direct)") { "X".to_string() } else { label };
                if !cell.split(", ").any(|x| x == label) {
                    if !cell.is_empty() {
                        cell.push_str(", ");
                    }
                    cell.push_str(&label);
                }
            }
        }
    }
    m
}

/// The matrix as an Excel workbook: causes down, effects across, the action under each effect.
pub fn xlsx(p: &Project) -> Result<Vec<u8>, String> {
    use rust_xlsxwriter::{Format, FormatAlign, Workbook};
    let m = matrix(p);
    if m.causes.is_empty() || m.effects.is_empty() {
        return Err("There is no interlock with a cause and an effect to put in a matrix yet: draw an interlock symbol and join it with signal lines to its instruments and to the valves or drives it acts on.".into());
    }
    let e = |x: rust_xlsxwriter::XlsxError| x.to_string();
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("Cause and effect").map_err(e)?;
    let bold = Format::new().set_bold();
    let head = Format::new().set_bold().set_background_color(0xF4DCEE).set_border(rust_xlsxwriter::FormatBorder::Thin);
    let vert = Format::new().set_bold().set_rotation(90).set_align(FormatAlign::Center).set_background_color(0xF4DCEE).set_border(rust_xlsxwriter::FormatBorder::Thin);
    let cell = Format::new().set_align(FormatAlign::Center).set_border(rust_xlsxwriter::FormatBorder::Thin);
    let plain = Format::new().set_border(rust_xlsxwriter::FormatBorder::Thin);
    ws.write_string_with_format(0, 0, format!("Cause and effect · {}", if p.title.project.is_empty() { &p.name } else { &p.title.project }), &bold).map_err(e)?;
    for (c, h) in ["Cause", "Trip", "Setting"].iter().enumerate() {
        ws.write_string_with_format(4, c as u16, *h, &head).map_err(e)?;
    }
    for (k, ef) in m.effects.iter().enumerate() {
        let col = 3 + k as u16;
        ws.write_string_with_format(2, col, &ef.tag, &vert).map_err(e)?;
        ws.write_string_with_format(3, col, &ef.what, &vert).map_err(e)?;
        ws.write_string_with_format(4, col, &ef.action, &head).map_err(e)?;
        ws.set_column_width(col, 7).map_err(e)?;
    }
    for (r, c) in m.causes.iter().enumerate() {
        let row = 5 + r as u32;
        ws.write_string_with_format(row, 0, &c.tag, &plain).map_err(e)?;
        ws.write_string_with_format(row, 1, &c.what, &plain).map_err(e)?;
        ws.write_string_with_format(row, 2, &c.setting, &plain).map_err(e)?;
        for (k, v) in m.cells[r].iter().enumerate() {
            ws.write_string_with_format(row, 3 + k as u16, v, &cell).map_err(e)?;
        }
    }
    ws.set_row_height(2, 60).map_err(e)?;
    ws.set_row_height(3, 110).map_err(e)?;
    ws.set_column_width(0, 14).map_err(e)?;
    ws.set_column_width(1, 30).map_err(e)?;
    ws.set_column_width(2, 14).map_err(e)?;
    wb.save_to_buffer().map_err(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compressor_trip_is_read_from_the_drawing() {
        let p = crate::examples::get("compressor").unwrap();
        let m = matrix(&p);
        assert_eq!(m.causes.iter().map(|c| c.tag.as_str()).collect::<Vec<_>>(), ["LSHH-202", "TSHH-203"]);
        assert_eq!(m.effects.iter().map(|e| (e.tag.as_str(), e.action.as_str())).collect::<Vec<_>>(), [("M-201", "Stop"), ("UV-201", "Open")]);
        assert!(m.cells.iter().flatten().all(|c| c == "UY-202"), "{:?}", m.cells);
        assert_eq!(m.causes[0].setting, "HH 80 %");
        assert!(m.open.is_empty(), "{:?}", m.open);
        assert_eq!(&xlsx(&p).unwrap()[..2], b"PK");
        // No interlocks, no matrix: said in words.
        let q = crate::examples::get("pump").unwrap();
        assert!(matrix(&q).causes.is_empty());
        assert!(xlsx(&q).unwrap_err().contains("interlock"));
    }

    #[test]
    fn a_switch_wired_straight_to_a_valve_and_an_interlock_with_no_effect() {
        use crate::doc::{Doc, Op};
        use crate::model::{End, Item, Kind, Line};
        let mut d = Doc::new(crate::model::Project::new());
        let sheet = d.p.sheets[0].id;
        let add = |d: &mut Doc, sym: &str, x: f64, tag: &str| d.apply(Op::AddItem { item: Item { sheet, sym: sym.into(), x, y: 100.0, scale: 1.0, ..Item::default() }, on: None, tag: tag.into(), obj: None }).unwrap()[0];
        let ps = add(&mut d, "inst_field", 50.0, "PSL-301");
        let xv = add(&mut d, "cv_solenoid", 150.0, "XV-301");
        let ux = add(&mut d, "inst_interlock", 250.0, "UY-302");
        let line = |a: u32, pa: u8, b: u32, pb: u8| Op::AddLine { line: Line { sheet, kind: Kind::Electric, a: End::Port { item: a, port: pa }, b: End::Port { item: b, port: pb }, ..Line::default() } };
        d.apply(line(ps, 3, xv, 2)).unwrap();
        d.apply(line(ux, 0, xv, 2)).unwrap();
        let m = matrix(&d.p);
        assert_eq!((m.causes.len(), m.effects.len()), (1, 1));
        assert_eq!(m.cells[0][0], "X");
        assert_eq!(m.causes[0].what, "Pressure low");
        assert_eq!(m.effects[0].action, "Close");
        assert!(m.open.iter().any(|o| o.starts_with("UY-302 has no cause")), "{:?}", m.open);
    }
}
