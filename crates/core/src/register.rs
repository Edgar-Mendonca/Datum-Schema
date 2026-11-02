//! The tag register: tags created before they are drawn (typed, pasted from a spreadsheet, or
//! read from Excel or CSV), each an object of its class with its service, marked as registered
//! ("~reg"). The register list shows every tagged object: drawn, registered but not drawn, or drawn
//! but not in the register (once the project keeps a register). A registered tag is drawn later
//! from the Plant panel or the register list, as the same object.

use crate::model::{Project, Val, LINE_CLASS, STREAM_CLASS};
use crate::symbols;
use serde::{Deserialize, Serialize};

/// One row of a register to add: what it says and what would happen.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Row {
    pub tag: String,
    pub class: String,
    pub service: String,
    /// new · exists (marked registered, its service filled if empty) · bad (and why, in `note`).
    pub status: String,
    pub note: String,
}

/// The class a tag's letters suggest: a symbol whose tag prefix they are (P → a centrifugal pump,
/// V → a vertical vessel, PSV → a safety valve), else an ISA instrument (…V a control valve).
pub fn guess_class(tag: &str) -> String {
    let (letters, _, _) = crate::tags::split(tag);
    let letters = letters.trim_end_matches(['-', '_', ' ']).to_uppercase();
    if letters.is_empty() {
        return String::new();
    }
    let syms = symbols::all();
    if let Some(s) = syms.iter().filter(|s| s.prefix.eq_ignore_ascii_case(&letters)).min_by_key(|s| (s.rare, s.cat.instrument())) {
        return crate::classes::class_name_for(s.key);
    }
    let isa = letters.len() >= 2 && letters.len() <= 5 && letters.chars().all(|c| c.is_ascii_uppercase());
    if isa && letters.ends_with('V') {
        return crate::classes::class_name_for("cv_pneu");
    }
    if isa {
        return crate::classes::class_name_for("inst_field");
    }
    String::new()
}

/// The class named in a row (by name or label), else guessed from the tag.
fn class_of(class: &str, tag: &str) -> Option<String> {
    let c = class.trim();
    if !c.is_empty() {
        // A class by name or label, or a symbol by name ("Vertical vessel").
        return crate::classes::all().iter().find(|x| x.name.eq_ignore_ascii_case(c) || x.label.eq_ignore_ascii_case(c)).map(|x| x.name.clone()).or_else(|| symbols::all().iter().find(|s| s.name.eq_ignore_ascii_case(c)).map(|s| crate::classes::class_name_for(s.key)));
    }
    Some(guess_class(tag)).filter(|x| !x.is_empty())
}

/// Reads rows (tag, class, service) from pasted text or a CSV/Excel file: a header row with a Tag
/// column is used when there is one; otherwise the columns are tag, class, service in that order.
pub fn read(p: &Project, bytes: &[u8]) -> Result<Vec<Row>, String> {
    let grids: Vec<Vec<Vec<String>>> = if bytes.starts_with(b"PK") {
        crate::import::read(bytes)?.into_iter().map(|g| g.1).collect()
    } else {
        let text = std::str::from_utf8(bytes).map_err(|_| "This is not text, a CSV file or an Excel workbook.".to_string())?;
        vec![split(text)]
    };
    let mut out: Vec<Row> = vec![];
    let mut seen = std::collections::HashSet::new();
    for g in grids {
        let head = g.iter().take(10).position(|r| r.iter().any(|c| matches!(c.trim().to_lowercase().as_str(), "tag" | "tag no" | "tag no." | "tag number" | "item tag")));
        let (start, ti, ci, si) = match head {
            Some(h) => {
                let r = &g[h];
                let col = |names: &[&str]| r.iter().position(|c| names.iter().any(|n| c.trim().to_lowercase().starts_with(n)));
                (h + 1, col(&["tag"]).unwrap_or(0), col(&["class", "type", "kind"]), col(&["service", "description", "name", "function"]))
            }
            None => (0, 0, Some(1), Some(2)),
        };
        for r in g.iter().skip(start).take(5000) {
            let tag = crate::clip(r.get(ti).map_or("", |s| s.trim()), 60);
            if tag.is_empty() {
                continue;
            }
            let class = ci.and_then(|c| r.get(c)).map_or(String::new(), |s| crate::clip(s.trim(), 80));
            let service = si.and_then(|c| r.get(c)).map_or(String::new(), |s| crate::clip(s.trim(), 200));
            let mut row = Row { tag: tag.clone(), class: class.clone(), service, ..Row::default() };
            if !seen.insert(tag.to_uppercase()) {
                (row.status, row.note) = ("bad".into(), "Listed twice.".into());
            } else if p.objects.iter().any(|o| o.tag.eq_ignore_ascii_case(&tag)) {
                row.status = "exists".into();
                row.class = p.objects.iter().find(|o| o.tag.eq_ignore_ascii_case(&tag)).map(|o| o.class.clone()).unwrap_or_default();
            } else {
                match class_of(&class, &tag) {
                    Some(c) => (row.status, row.class) = ("new".into(), c),
                    None if !class.is_empty() => (row.status, row.note) = ("bad".into(), format!("“{class}” is not a class (see Options › Classes).")),
                    None => (row.status, row.note) = ("bad".into(), "Its letters say no class: give one.".into()),
                }
            }
            out.push(row);
        }
    }
    if out.is_empty() {
        return Err("No tags found: one tag per line, optionally with its class and service (tab, comma or semicolon between).".into());
    }
    Ok(out)
}

/// Text split into rows and cells: tab, semicolon or comma separated (pasted from a spreadsheet).
fn split(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let sep = if first.contains('\t') { '\t' } else if first.contains(';') { ';' } else { ',' };
    text.lines().map(|l| l.split(sep).map(|c| c.trim().trim_matches('"').to_string()).collect()).collect()
}

/// Adds the rows: new tags as objects not yet drawn, existing ones marked as registered.
pub fn add(p: &mut Project, rows: &[Row], created: &mut Vec<u32>) -> Result<usize, String> {
    let mut n = 0;
    for r in rows.iter().take(5000) {
        let tag = crate::clip(r.tag.trim(), 60);
        if tag.is_empty() {
            continue;
        }
        if let Some(o) = p.objects.iter_mut().find(|o| o.tag.eq_ignore_ascii_case(&tag)) {
            o.data.insert("~reg".into(), Val::Text("1".into()));
            if !r.service.trim().is_empty() && !o.data.contains_key("service") {
                o.data.insert("service".into(), Val::Text(crate::clip(r.service.trim(), 200)));
            }
            n += 1;
            continue;
        }
        let Some(class) = class_of(&r.class, &tag) else { continue };
        let o = p.new_obj(&class, &tag);
        if let Some(ob) = p.obj_mut(o) {
            ob.data.insert("~reg".into(), Val::Text("1".into()));
            if !r.service.trim().is_empty() {
                ob.data.insert("service".into(), Val::Text(crate::clip(r.service.trim(), 200)));
            }
        }
        created.push(o);
        n += 1;
    }
    if n == 0 {
        return Err("No tag could be added.".into());
    }
    Ok(n)
}

/// Whether an object belongs in the register (tagged plant items, not lines or streams).
pub fn listed(o: &crate::model::Obj) -> bool {
    !o.tag.trim().is_empty() && o.class != LINE_CLASS && o.class != STREAM_CLASS
}

/// An object's register status.
pub fn status(p: &Project, o: &crate::model::Obj, keeps_register: bool) -> &'static str {
    let drawn = p.items.iter().any(|i| i.obj == o.id);
    match (drawn, o.data.contains_key("~reg")) {
        (true, true) => "Drawn",
        (true, false) if keeps_register => "Drawn, not in the register",
        (true, false) => "Drawn",
        (false, _) => "Not drawn",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    #[test]
    fn tags_are_registered_before_they_are_drawn() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        assert_eq!(guess_class("P-201"), crate::classes::class_name_for("pump_cent"));
        assert_eq!(guess_class("FT-201"), crate::classes::class_name_for("inst_field"));
        assert_eq!(guess_class("FV-201"), crate::classes::class_name_for("cv_pneu"));
        assert_eq!(guess_class("PSV-201"), crate::classes::class_name_for("psv"));
        // Pasted from a spreadsheet: a header, a class by label, a duplicate, an existing tag, a bad class.
        let text = "Tag\tType\tService\nP-201\t\tTransfer pump\nV-201\tVertical vessel\tSurge drum\nP-201\t\tagain\nP-101A\t\tCharge pump A\nX-1\tNo such class\t\nFT-201\t\tFeed flow\n";
        let rows = read(&d.p, text.as_bytes()).unwrap();
        let st = |t: &str| rows.iter().filter(|r| r.tag == t).map(|r| r.status.as_str()).collect::<Vec<_>>();
        assert_eq!(st("P-201"), ["new", "bad"]);
        assert_eq!(st("V-201"), ["new"]);
        assert_eq!(st("P-101A"), ["exists"]);
        assert_eq!(st("X-1"), ["bad"]);
        let ok: Vec<Row> = rows.into_iter().filter(|r| r.status != "bad").collect();
        d.apply(Op::Register { rows: ok }).unwrap();
        let p201 = d.p.objects.iter().find(|o| o.tag == "P-201").unwrap();
        assert_eq!(p201.data.get("service").map(|v| v.text()), Some("Transfer pump".into()));
        assert_eq!(status(&d.p, p201, true), "Not drawn");
        // The register list: registered not drawn, drawn not in the register.
        let l = crate::lists::build(&d.p, "register").unwrap();
        let col = l.cols.iter().position(|c| c.key == "status").unwrap();
        let of = |t: &str| l.rows.iter().find(|r| r.cells[0] == t).map(|r| r.cells[col].clone()).unwrap();
        assert_eq!(of("P-201"), "Not drawn");
        assert_eq!(of("P-101A"), "Drawn");
        assert_eq!(of("V-101"), "Drawn, not in the register");
        // Drawn as the same object: now "Drawn".
        let s = d.p.sheets[0].id;
        let o = d.p.objects.iter().find(|o| o.tag == "P-201").unwrap().id;
        d.apply(Op::AddItem { item: crate::model::Item { sheet: s, sym: "pump_cent".into(), x: 300.0, y: 250.0, ..Default::default() }, on: None, tag: String::new(), obj: Some(o) }).unwrap();
        assert_eq!(status(&d.p, d.p.obj(o).unwrap(), true), "Drawn");
        // Hostile input.
        assert!(read(&d.p, b"").is_err());
        assert!(read(&d.p, &[0xff, 0xfe, 0x00]).is_err());
        assert!(d.apply(Op::Register { rows: vec![Row { tag: " ".into(), ..Row::default() }] }).is_err());
    }
}
