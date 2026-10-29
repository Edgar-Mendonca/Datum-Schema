//! The four lists, always generated from the drawing: the line list (one row per line number,
//! with from/to and hydraulics), the equipment list, the instrument index and the valve list.
//! Rows are text in the project's display units; columns say which data field they edit and in
//! which quantity, so the window can send edits back. CSV and Excel export.

use crate::graph::Net;
use crate::hydraulics::{self, Hyd};
use crate::model::{End, Item, Project};
use crate::symbols::{self, Cat};
use crate::units;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Col {
    pub key: &'static str,
    pub label: String,
    pub q: &'static str,
    pub edit: bool,
    pub num: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Row {
    /// The items or lines the row is about (a line number's every drawn piece).
    pub ids: Vec<u32>,
    pub sheet: u32,
    pub cells: Vec<String>,
    /// Hydraulic status (line list): 0 ✓, 1 !, 2 ✗.
    pub status: Option<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub struct List {
    pub kind: String,
    pub title: String,
    pub cols: Vec<Col>,
    pub rows: Vec<Row>,
}

pub const KINDS: [(&str, &str); 4] = [("lines", "Line list"), ("equipment", "Equipment list"), ("instruments", "Instrument index"), ("valves", "Valve list")];

/// (key, label, quantity, editable)
type Spec = (&'static str, &'static str, &'static str, bool);

fn cols(specs: &[Spec], us: bool) -> Vec<Col> {
    specs
        .iter()
        .map(|&(key, label, q, edit)| {
            let u = if q.is_empty() { String::new() } else { units::shown(us).into_iter().find(|s| s.q == q).map(|s| s.label).unwrap_or_default() };
            Col { key, label: if u.is_empty() { label.into() } else { format!("{label} ({u})") }, q, edit, num: !q.is_empty() }
        })
        .collect()
}

fn val(data: &crate::model::Data, k: &str, q: &str, us: bool) -> String {
    match data.get(k) {
        None => String::new(),
        Some(v) if !q.is_empty() => v.num().map(|x| crate::fmt_sig(units::to_display(x, q, us), 4)).unwrap_or_else(|| v.text()),
        Some(v) => v.text(),
    }
}

fn sheet_no(p: &Project, id: u32) -> String {
    p.sheet(id).map(|s| s.number.clone()).unwrap_or_default()
}

pub fn build(p: &Project, kind: &str) -> Result<List, String> {
    let net = Net::new(p);
    let us = p.units == "US";
    let title = KINDS.iter().find(|k| k.0 == kind).ok_or_else(|| format!("There is no list called {kind}."))?.1.to_string();
    let items = |f: &dyn Fn(Cat) -> bool| -> Vec<&Item> {
        let mut v: Vec<&Item> = p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| f(s.cat))).collect();
        v.sort_by(|a, b| crate::tags::split(&a.tag).0.cmp(&crate::tags::split(&b.tag).0).then(crate::tags::split(&a.tag).1.cmp(&crate::tags::split(&b.tag).1)).then(a.tag.cmp(&b.tag)));
        v
    };
    let sym_name = |it: &Item| symbols::get(&it.sym).map_or(String::new(), |s| s.name.to_string());
    let (cols_, rows): (Vec<Col>, Vec<Row>) = match kind {
        "lines" => {
            let specs: Vec<Spec> = vec![
                ("number", "Line number", "", false), ("size", "Size", "", true), ("sched", "Sched", "", true), ("fluid", "Fluid", "", true), ("seq", "Seq", "", true), ("class", "Class", "", true),
                ("service", "Service", "", true), ("from", "From", "", false), ("to", "To", "", false),
                ("flow", "Flow", "flow", true), ("v", "Velocity", "velocity", false), ("dp100", "ΔP", "dp100", false), ("dp", "ΔP total", "dp", false), ("status", "Hydraulics", "", false),
                ("design_p", "Design P", "press", true), ("design_t", "Design T", "temp", true), ("oper_p", "Oper P", "press", true), ("oper_t", "Oper T", "temp", true),
                ("density", "Density", "density", true), ("viscosity", "Viscosity", "visc", true), ("length", "Length", "length", true),
                ("ins_type", "Insulation", "", true), ("ins_thk", "Ins. thk", "thk", true), ("tracing", "Tracing", "", true), ("test_p", "Test P", "press", true), ("sheet", "Sheet", "", false),
            ];
            let hyd: Vec<Option<Hyd>> = hydraulics::all(p, &net);
            let mut rows: Vec<Row> = net
                .runs
                .iter()
                .enumerate()
                .map(|(r, lines)| {
                    let l = net.lines[&lines[0]];
                    let (from, to) = net.run_ends(r);
                    let h = hyd[r].as_ref();
                    let sheets: Vec<String> = { let mut s: Vec<u32> = lines.iter().map(|x| net.lines[x].sheet).collect(); s.sort_by_key(|x| p.sheet_no(*x)); s.dedup(); s.iter().map(|x| sheet_no(p, *x)).collect() };
                    let cells = specs
                        .iter()
                        .map(|&(k, _, q, _)| match k {
                            "number" => net.numbers[&lines[0]].clone(),
                            "from" => from.as_ref().map(|e| net.describe(e)).unwrap_or_default(),
                            "to" => to.as_ref().map(|e| net.describe(e)).unwrap_or_default(),
                            "v" => h.map(|h| crate::fmt_sig(units::to_display(h.v, q, us), 3)).unwrap_or_default(),
                            "dp100" => h.map(|h| crate::fmt_sig(units::to_display(h.dp100, q, us), 3)).unwrap_or_default(),
                            "dp" => h.filter(|h| h.length > 0.0).map(|h| crate::fmt_sig(units::to_display(h.dp, q, us), 3)).unwrap_or_default(),
                            "status" => h.map(|h| ["✓", "!", "✗"][h.status.min(2) as usize].to_string()).unwrap_or_default(),
                            "sheet" => sheets.join(", "),
                            _ => val(&l.data, k, q, us),
                        })
                        .collect();
                    Row { ids: lines.clone(), sheet: l.sheet, cells, status: h.map(|h| h.status) }
                })
                .collect();
            rows.sort_by(|a, b| a.cells[0].cmp(&b.cells[0]));
            (cols(&specs, us), rows)
        }
        "equipment" => {
            let specs: Vec<Spec> = vec![("tag", "Tag", "", true), ("kind", "Type", "", false), ("service", "Service", "", true), ("design_p", "Design P", "press", true), ("design_t", "Design T", "temp", true), ("oper_p", "Oper P", "press", true), ("oper_t", "Oper T", "temp", true), ("material", "Material", "", true), ("capacity", "Size / capacity", "", false), ("power", "Power", "power", true), ("sheet", "Sheet", "", false)];
            let rows = items(&|c| c.equipment())
                .into_iter()
                .map(|it| {
                    let cap = [("flow", "flow"), ("head", "head"), ("duty", "power"), ("volume", "volume"), ("area", "area")].iter().filter_map(|(k, q)| it.data.get(*k).and_then(|v| v.num()).map(|x| units::show(x, q, us))).chain(it.data.get("size").map(|v| v.text())).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
                    let cells = specs.iter().map(|&(k, _, q, _)| match k { "tag" => it.tag.clone(), "kind" => sym_name(it), "capacity" => cap.clone(), "sheet" => sheet_no(p, it.sheet), _ => val(&it.data, k, q, us) }).collect();
                    Row { ids: vec![it.id], sheet: it.sheet, cells, status: None }
                })
                .collect();
            (cols(&specs, us), rows)
        }
        "instruments" => {
            let specs: Vec<Spec> = vec![("tag", "Tag", "", true), ("function", "Function", "", true), ("loop", "Loop", "", false), ("kind", "Type", "", false), ("location", "Location", "", true), ("range", "Range", "", true), ("setpoints", "Set points", "", true), ("signal", "Signal", "", true), ("on", "On", "", false), ("sheet", "Sheet", "", false)];
            let rows = items(&|c| c.instrument())
                .into_iter()
                .map(|it| {
                    let cells = specs.iter().map(|&(k, _, q, _)| match k { "tag" => it.tag.clone(), "loop" => crate::tags::loop_of(&it.tag).1, "kind" => sym_name(it), "on" => attached(&net, it), "sheet" => sheet_no(p, it.sheet), _ => val(&it.data, k, q, us) }).collect();
                    Row { ids: vec![it.id], sheet: it.sheet, cells, status: None }
                })
                .collect();
            (cols(&specs, us), rows)
        }
        "valves" => {
            let specs: Vec<Spec> = vec![("tag", "Tag", "", true), ("kind", "Type", "", false), ("size", "Size", "", true), ("class", "Class", "", true), ("operator", "Operator", "", true), ("position", "Position", "", true), ("fail", "Fail", "", true), ("on", "Line", "", false), ("service", "Service", "", true), ("sheet", "Sheet", "", false)];
            let rows = items(&|c| c.valve())
                .into_iter()
                .map(|it| {
                    let cells = specs.iter().map(|&(k, _, q, _)| match k { "tag" => it.tag.clone(), "kind" => sym_name(it), "on" => attached(&net, it), "sheet" => sheet_no(p, it.sheet), _ => val(&it.data, k, q, us) }).collect();
                    Row { ids: vec![it.id], sheet: it.sheet, cells, status: None }
                })
                .collect();
            (cols(&specs, us), rows)
        }
        _ => unreachable!(),
    };
    Ok(List { kind: kind.into(), title, cols: cols_, rows })
}

/// What an instrument or valve is on: the line numbers and equipment at its ports' other ends.
fn attached(net: &Net, it: &Item) -> String {
    let Some(s) = symbols::get(&it.sym) else { return String::new() };
    let mut out: Vec<String> = vec![];
    for k in 0..s.ports.len() {
        for &(l, b) in net.on_port(it.id, k as u8) {
            let line = net.lines[&l];
            let n = net.numbers.get(&l).cloned().unwrap_or_default();
            let what = if line.kind.process() {
                n
            } else {
                match if b { &line.a } else { &line.b } {
                    End::Line { line, .. } => net.numbers.get(line).cloned().unwrap_or_default(),
                    End::Port { item, .. } => net.items.get(item).filter(|o| symbols::get(&o.sym).is_some_and(|s| s.cat.equipment())).map(|o| o.tag.clone()).unwrap_or_default(),
                    End::Free { .. } => String::new(),
                }
            };
            if !what.is_empty() && !out.contains(&what) {
                out.push(what);
            }
        }
    }
    out.join(", ")
}

/// A cell made safe for spreadsheets: text starting with = + - @ gets a leading ' (unless it is a
/// plain number), so a spreadsheet never runs it as a formula.
pub fn safe(s: &str) -> String {
    let plain = s.trim().parse::<f64>().is_ok();
    if !plain && s.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{s}") } else { s.to_string() }
}

pub fn csv(l: &List) -> String {
    let q = |s: &str| {
        let s = safe(s);
        if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
    };
    let mut out = l.cols.iter().map(|c| q(&c.label)).collect::<Vec<_>>().join(",");
    out.push_str("\r\n");
    for r in &l.rows {
        out.push_str(&r.cells.iter().map(|c| q(c)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    out
}

/// An Excel workbook with the list on one sheet: a title, the column headers, the rows (numbers as
/// numbers), frozen headers.
pub fn xlsx(p: &Project, kind: &str) -> Result<Vec<u8>, String> {
    use rust_xlsxwriter::{Format, Workbook};
    let l = build(p, kind)?;
    let e = |x: rust_xlsxwriter::XlsxError| x.to_string();
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name(crate::clip(&l.title, 31)).map_err(e)?;
    let bold = Format::new().set_bold();
    let head = Format::new().set_bold().set_background_color(0xF4DCEE).set_border_bottom(rust_xlsxwriter::FormatBorder::Thin);
    ws.write_string_with_format(0, 0, format!("{} · {}", l.title, if p.title.project.is_empty() { &p.name } else { &p.title.project }), &bold).map_err(e)?;
    for (c, col) in l.cols.iter().enumerate() {
        ws.write_string_with_format(2, c as u16, &col.label, &head).map_err(e)?;
        ws.set_column_width(c as u16, (col.label.chars().count().max(8) + 2) as f64).map_err(e)?;
    }
    for (r, row) in l.rows.iter().enumerate() {
        for (c, cell) in row.cells.iter().enumerate() {
            let (rr, cc) = (r as u32 + 3, c as u16);
            match cell.parse::<f64>() {
                Ok(v) if l.cols[c].num && v.is_finite() => ws.write_number(rr, cc, v).map_err(e)?,
                _ => ws.write_string(rr, cc, safe(cell)).map_err(e)?,
            };
        }
    }
    ws.set_freeze_panes(3, 0).map_err(e)?;
    wb.save_to_buffer().map_err(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::tests::B;
    use crate::model::Val;

    #[test]
    fn lists_follow_the_drawing() {
        let mut b = B::new();
        let t = b.item("tank_cone", 50.0, 100.0);
        let pm = b.item("pump_cent", 150.0, 120.0);
        let l = b.line((t, 2), (pm, 0));
        b.spec(&[l], "4", "A1A");
        b.set(&[l], "flow", Val::Num(30.0 / 3600.0));
        b.set(&[l], "density", Val::Num(1000.0));
        b.set(&[l], "viscosity", Val::Num(1e-3));
        b.set(&[l], "design_p", Val::Num(10e5));
        let i = b.item("inst_field", 100.0, 60.0);
        b.tag(i, "PT-101");
        let sheet = b.sheet();
        b.d.apply(crate::doc::Op::AddLine { line: crate::model::Line { sheet, kind: crate::model::Kind::Electric, a: End::Port { item: i, port: 0 }, b: End::Line { line: l, x: 100.0, y: 110.0 }, ..Default::default() } }).unwrap();
        let ll = build(&b.d.p, "lines").unwrap();
        assert_eq!(ll.rows.len(), 1);
        let row = &ll.rows[0].cells;
        let col = |k: &str| ll.cols.iter().position(|c| c.key == k).unwrap();
        assert_eq!(row[col("number")], "4\"-P-1001-A1A");
        assert_eq!(row[col("from")], "T-101 right lower");
        assert_eq!(row[col("to")], "P-101 suction");
        assert_eq!(row[col("design_p")], "10", "bar");
        assert_eq!(ll.cols[col("design_p")].label, "Design P (bar)");
        assert_eq!(row[col("status")], "✓");
        let eq = build(&b.d.p, "equipment").unwrap();
        assert_eq!(eq.rows.iter().map(|r| r.cells[0].clone()).collect::<Vec<_>>(), vec!["P-101", "T-101"]);
        let inst = build(&b.d.p, "instruments").unwrap();
        assert_eq!(inst.rows[0].cells[0], "PT-101");
        assert_eq!(inst.rows[0].cells[8], "4\"-P-1001-A1A", "on its line");
        // US units change the numbers, not the file.
        b.d.apply(crate::doc::Op::SetProject { name: "x".into(), units: "US".into() }).unwrap();
        let us = build(&b.d.p, "lines").unwrap();
        assert_eq!(us.rows[0].cells[col("design_p")], "145");
        assert!(build(&b.d.p, "nope").is_err());
    }

    #[test]
    fn csv_and_excel() {
        let mut b = B::new();
        let v = b.item("vvessel", 50.0, 100.0);
        b.set(&[v], "service", Val::Text("=HYPERLINK(\"x\")".into()));
        let l = build(&b.d.p, "equipment").unwrap();
        let c = csv(&l);
        assert!(c.contains("\"'=HYPERLINK(\"\"x\"\")\""), "{c}");
        assert_eq!(safe("-3.5"), "-3.5");
        assert_eq!(safe("-x"), "'-x");
        let x = xlsx(&b.d.p, "equipment").unwrap();
        assert_eq!(&x[..2], b"PK");
    }
}
