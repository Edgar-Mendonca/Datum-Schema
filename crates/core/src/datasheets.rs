//! Datasheets: one per instrument, primary element, control valve or relief device, built from
//! the object's class attributes and the process conditions of the line it measures or sits in.
//! The layout is Datum Schema's own: General · Process conditions · the class's attributes ·
//! Specification · the project's own attributes. Values the datasheet takes from elsewhere (the
//! line, the drawing) are marked with their source and are edited there; the object's own values
//! are edited on the datasheet.

use crate::classes::{self, Attr};
use crate::graph::{instrument_connection, Net};
use crate::model::{End, Item, Project, Val};
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
}

/// Objects that have a datasheet: instruments, primary elements, control valves, relief devices.
pub fn has_datasheet(cat: Cat) -> bool {
    cat.instrument() || cat == Cat::Control
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
    Row { key: String::new(), label: label.into(), value, unit, q: String::new(), choices: vec![], required: false, source: source.into() }
}

/// The datasheet of an object (by object id or the id of a symbol showing it).
pub fn build(p: &Project, id: u32) -> Result<Datasheet, String> {
    let net = Net::new(p);
    let obj = p.obj_id(id).and_then(|o| p.obj(o)).ok_or("That object no longer exists.")?;
    let it = p.items.iter().find(|i| i.obj == obj.id).ok_or("Draw the object on a sheet first: its datasheet reads its connections.")?;
    let sym = symbols::get(&it.sym).ok_or("There is no such symbol.")?;
    if !has_datasheet(sym.cat) {
        return Err(format!("A {} has no datasheet yet; instruments, control valves and relief devices do.", sym.name.to_lowercase()));
    }
    let us = p.units == "US";
    let (letters, lp) = tags::loop_of(&obj.tag);
    let class_label = classes::get(&obj.class).map_or(obj.class.clone(), |c| c.label.clone());
    let sheet = p.sheet(it.sheet).map(|s| s.number.clone()).unwrap_or_default();
    let source = process_source(p, &net, it);
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
        Row { key: a.key.clone(), label: a.label.clone(), value, unit, q: a.q.clone(), choices: a.choices.clone(), required: a.required, source: String::new() }
    };
    let own_title = match sym.cat { Cat::Control => "Valve", Cat::Primary => "Element", _ => "Instrument" };
    let user: std::collections::HashSet<String> = p.settings.user_attrs.iter().map(|u| u.attr.key.clone()).collect();
    let own: Vec<Row> = attrs.iter().filter(|a| a.group.is_empty() && !user.contains(&a.key) && a.key != "service").map(row).collect();
    let spec: Vec<Row> = attrs.iter().filter(|a| a.group == "Specification" && !user.contains(&a.key)).map(row).collect();
    let company: Vec<Row> = attrs.iter().filter(|a| user.contains(&a.key)).map(row).collect();
    let mut sections = vec![Section { title: "General".into(), rows: general }];
    sections.push(Section { title: "Process conditions".into(), rows: if process.is_empty() { vec![derived("Process line", "not connected".into(), String::new(), "drawing")] } else { process } });
    sections.push(Section { title: own_title.into(), rows: own });
    sections.push(Section { title: "Specification".into(), rows: spec });
    if !company.is_empty() {
        sections.push(Section { title: "Company attributes".into(), rows: company });
    }
    let title = format!("{} datasheet", if sym.cat == Cat::Control { "Valve" } else { "Instrument" });
    Ok(Datasheet { obj: obj.id, item: it.id, sheet: it.sheet, tag: obj.tag.clone(), title, sections })
}

/// Every datasheet in the project, in tag order.
pub fn all(p: &Project) -> Vec<Datasheet> {
    let mut seen = std::collections::HashSet::new();
    let mut v: Vec<Datasheet> = p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| has_datasheet(s.cat)) && seen.insert(i.obj)).filter_map(|i| build(p, i.obj).ok()).collect();
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
        for (r, d) in sheets.iter().enumerate() {
            ix.write_string(r as u32 + 2, 0, &d.tag).map_err(e)?;
            ix.write_string(r as u32 + 2, 1, &d.title).map_err(e)?;
        }
        ix.set_column_width(0, 18).map_err(e)?;
    }
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
        let mut r = 3u32;
        for s in &d.sections {
            ws.write_string_with_format(r, 0, &s.title, &head).map_err(e)?;
            ws.write_string_with_format(r, 1, "", &head).map_err(e)?;
            ws.write_string_with_format(r, 2, "", &head).map_err(e)?;
            ws.write_string_with_format(r, 3, "", &head).map_err(e)?;
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
                r += 1;
            }
            r += 1;
        }
        ws.set_column_width(0, 32).map_err(e)?;
        ws.set_column_width(1, 26).map_err(e)?;
        ws.set_column_width(3, 30).map_err(e)?;
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
        // The control valve has valve sections; a pump has no datasheet.
        let lv = p.objects.iter().find(|o| o.tag == "LV-101").unwrap();
        let d = build(&p, lv.id).unwrap();
        assert!(d.sections.iter().any(|s| s.title == "Valve" && s.rows.iter().any(|r| r.key == "fail" && r.value == "FC")));
        assert!(d.sections.iter().any(|s| s.title == "Specification" && s.rows.iter().any(|r| r.key == "characteristic")));
        let pump = p.objects.iter().find(|o| o.tag == "P-101A").unwrap();
        assert!(build(&p, pump.id).is_err());
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
}
