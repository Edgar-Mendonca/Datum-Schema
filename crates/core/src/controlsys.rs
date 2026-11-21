//! What the control-system team needs from the P&IDs, in one workbook: the I/O list (each input
//! and output with its tag, description, I/O type, signal, range, fail position, system, channel,
//! junction box, cable and loop), the instrument tag list (drawn and virtual), the alarms and trips
//! read from each instrument's set points, and the I/O summary with spare. Read from the drawing
//! when it is made; nothing changes.

use crate::lists::{Col, List, Row};
use crate::model::Project;
use crate::symbols::{self, Cat};

fn col(k: &str, l: &str) -> Col {
    Col { key: k.into(), label: l.into(), q: "", edit: false, num: false }
}

fn text(p: &Project, obj: u32, k: &str) -> String {
    p.data_of(obj).get(k).map(|v| v.text().trim().to_string()).unwrap_or_default()
}

fn sheet_of(p: &Project, it: &crate::model::Item) -> String {
    let n = p.sheet(it.sheet).map(|s| s.number.clone()).unwrap_or_default();
    if it.virt { format!("{n} (virtual)") } else { n }
}

/// The I/O list: one row per input or output, in tag order.
pub fn io_list(p: &Project) -> List {
    let mut seen = std::collections::HashSet::new();
    let mut rows: Vec<(String, Row)> = vec![];
    for it in &p.items {
        if it.obj == 0 || !seen.insert(it.obj) {
            continue;
        }
        let Some(kind) = crate::instruments::io_of(p, it) else { continue };
        let o = it.obj;
        let tag = p.tag_of(it).to_string();
        let desc = [text(p, o, "service"), text(p, o, "function")].into_iter().find(|s| !s.is_empty()).unwrap_or_default();
        let output = matches!(kind, "AO" | "DO");
        let cells = vec![
            tag.clone(),
            desc,
            kind.to_string(),
            text(p, o, "signal"),
            text(p, o, "range"),
            if output { text(p, o, "fail") } else { String::new() },
            text(p, o, "system"),
            text(p, o, "io"),
            text(p, o, "jb"),
            text(p, o, "cable"),
            crate::tags::loop_of(&tag).1,
            sheet_of(p, it),
        ];
        rows.push((tag, Row { ids: vec![it.id], sheet: it.sheet, cells, status: None }));
    }
    rows.sort_by(|a, b| {
        let (x, y) = (crate::tags::split(&a.0), crate::tags::split(&b.0));
        (x.1, x.0, &a.0).cmp(&(y.1, y.0, &b.0))
    });
    let cols = ["Tag", "Description", "I/O", "Signal", "Range", "Fail", "System", "Channel", "Junction box", "Cable", "Loop", "P&ID"].iter().enumerate().map(|(k, l)| col(&format!("c{k}"), l)).collect();
    List { kind: "io_list".into(), title: "I/O list".into(), cols, rows: rows.into_iter().map(|r| r.1).collect(), heads: vec![] }
}

/// What a set-point code is: an alarm, a trip, a switch or the controller's set point.
fn kind_of(code: &str) -> &'static str {
    let c = code.to_uppercase();
    if c == "SP" {
        "Set point"
    } else if c.len() >= 3 && (c[1..].starts_with('Z') || c.ends_with("HH") || c.ends_with("LL")) {
        "Trip"
    } else if c.len() >= 3 && c[1..].starts_with('A') {
        "Alarm"
    } else if c.len() >= 3 && c[1..].starts_with('S') {
        "Switch"
    } else {
        "Set point"
    }
}

/// Alarms and trips: every set point written on an instrument ("SP 50 %, LAH 80 %, LALL 10 %").
pub fn alarms(p: &Project) -> List {
    let mut seen = std::collections::HashSet::new();
    let mut rows: Vec<(String, Row)> = vec![];
    for it in &p.items {
        if it.obj == 0 || !seen.insert(it.obj) || !symbols::get(&it.sym).is_some_and(|s| s.cat.instrument() || s.cat == Cat::Control) {
            continue;
        }
        let tag = p.tag_of(it).to_string();
        let sp = text(p, it.obj, "setpoints");
        for part in sp.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()) {
            let mut w = part.splitn(2, char::is_whitespace);
            let code = w.next().unwrap_or("").trim().to_string();
            let value = w.next().unwrap_or("").trim().to_string();
            if code.is_empty() || !code.chars().all(|c| c.is_ascii_alphabetic()) {
                continue;
            }
            let cells = vec![tag.clone(), code.to_uppercase(), kind_of(&code).into(), value, text(p, it.obj, "system"), crate::tags::loop_of(&tag).1, sheet_of(p, it)];
            rows.push((tag.clone(), Row { ids: vec![it.id], sheet: it.sheet, cells, status: None }));
        }
    }
    rows.sort_by(|a, b| {
        let (x, y) = (crate::tags::split(&a.0), crate::tags::split(&b.0));
        (x.1, x.0, &a.0).cmp(&(y.1, y.0, &b.0))
    });
    let cols = ["Tag", "Code", "Kind", "Set point", "System", "Loop", "P&ID"].iter().enumerate().map(|(k, l)| col(&format!("c{k}"), l)).collect();
    List { kind: "alarms".into(), title: "Alarms and trips".into(), cols, rows: rows.into_iter().map(|r| r.1).collect(), heads: vec![] }
}

/// The workbook for the control-system team: I/O list, tag list, alarms and trips, I/O summary.
pub fn xlsx(p: &Project) -> Result<Vec<u8>, String> {
    let mut wb = rust_xlsxwriter::Workbook::new();
    crate::lists::write_sheet(&mut wb, p, &io_list(p))?;
    let mut tags = crate::lists::build(p, "instruments")?;
    tags.title = "Instrument tag list".into();
    crate::lists::write_sheet(&mut wb, p, &tags)?;
    crate::lists::write_sheet(&mut wb, p, &alarms(p))?;
    crate::lists::write_sheet(&mut wb, p, &crate::lists::build(p, "io")?)?;
    wb.save_to_buffer().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::{Item, Val};

    #[test]
    fn virtual_instruments_and_the_control_system_export() {
        let mut d = Doc::new(crate::examples::get("worked").unwrap());
        let s = d.p.sheets[0].id;
        let lic = d.p.items.iter().find(|i| d.p.tag_of(i) == "LIC-101").unwrap().clone();
        // A virtual alarm block in loop 101: in the lists, the loop and the I/O, not on the sheet.
        let v = d.apply(Op::AddItem { item: Item { sheet: s, sym: "inst_dcs".into(), x: lic.x, y: lic.y, virt: true, ..Item::default() }, on: None, tag: "LY-101".into(), obj: None }).unwrap()[0];
        let o = d.p.item(v).unwrap().obj;
        for (k, val) in [("function", "Level deviation calculation"), ("setpoints", "LAHH 90 %, LALL 10 %"), ("system", "DCS")] {
            d.apply(Op::SetData { ids: vec![o], key: k.into(), val: Some(Val::Text(val.into())) }).unwrap();
        }
        assert!(d.p.item(v).unwrap().virt);
        let net = crate::graph::Net::new(&d.p);
        let view = crate::view::sheet(&d.p, &d.p.sheets[0], &net, &[]);
        assert!(view.items.iter().all(|i| i.id != v), "not drawn");
        assert!(crate::dexpi::write(&d.p, &d.p.sheets[0]).matches("LY-101").count() == 0, "not in DEXPI");
        let inst = crate::lists::build(&d.p, "instruments").unwrap();
        assert!(inst.rows.iter().any(|r| r.cells[0] == "LY-101" && r.cells.last().unwrap().ends_with("(virtual)")));
        assert!(crate::loops::all(&d.p).iter().any(|l| l.key.contains("101") && l.tags.contains(&"LY-101".to_string())));
        assert!(crate::query::find(&d.p, "virtual instruments").unwrap().rows.iter().any(|r| r.tag == "LY-101"));
        // No new errors; kept through save and open.
        assert!(crate::api::derived(&d.p, s).issues.iter().all(|i| i.level != "error"));
        let path = std::env::temp_dir().join(format!("virt-{}.dschema", std::process::id()));
        crate::file::write(&path, &d.p, &[]).unwrap();
        let (back, _, w) = crate::file::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(w.is_empty(), "{w:?}");
        assert!(back.item(v).unwrap().virt);
        // Drawn later: cleared, and it is on the sheet.
        let mut it = d.p.item(v).unwrap().clone();
        it.virt = false;
        d.apply(Op::SetItem { item: it.clone() }).unwrap();
        assert!(crate::view::sheet(&d.p, &d.p.sheets[0], &crate::graph::Net::new(&d.p), &[]).items.iter().any(|i| i.id == v));
        // Only instruments, and none with lines on it.
        let hv = d.p.items.iter().find(|i| d.p.tag_of(i) == "HV-101").unwrap().clone();
        assert!(d.apply(Op::SetItem { item: Item { virt: true, ..hv } }).is_err());
        let lt = d.p.items.iter().find(|i| d.p.tag_of(i) == "LT-101").unwrap().clone();
        assert!(d.apply(Op::SetItem { item: Item { virt: true, ..lt } }).is_err(), "LT-101 has lines on it");
        it.virt = true;
        d.apply(Op::SetItem { item: it }).unwrap();

        // The control-system export.
        let io = io_list(&d.p);
        let row = |t: &str| io.rows.iter().find(|r| r.cells[0] == t).map(|r| r.cells.clone());
        assert_eq!(row("LT-101").unwrap()[2], "AI");
        let lv = row("LV-101").unwrap();
        assert_eq!((lv[2].as_str(), lv[5].as_str()), ("AO", "FC"));
        assert!(row("LY-101").is_none(), "a function in the DCS takes no I/O");
        let al = alarms(&d.p);
        let a = |t: &str, c: &str| al.rows.iter().find(|r| r.cells[0] == t && r.cells[1] == c).map(|r| (r.cells[2].clone(), r.cells[3].clone()));
        assert_eq!(a("LIC-101", "LAH"), Some(("Alarm".into(), "80 %".into())));
        assert_eq!(a("LIC-101", "SP"), Some(("Set point".into(), "50 %".into())));
        assert_eq!(a("LY-101", "LAHH"), Some(("Trip".into(), "90 %".into())));
        let bytes = xlsx(&d.p).unwrap();
        assert_eq!(&bytes[..2], b"PK");
        assert!(crate::lists::build(&d.p, "io_list").unwrap().rows.len() == io.rows.len());
        assert!(crate::lists::build(&d.p, "alarms").unwrap().rows.len() == al.rows.len());
    }
}
