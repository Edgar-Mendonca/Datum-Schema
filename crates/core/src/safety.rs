//! Safety analysis: the protections each kind of equipment needs (high and low pressure, high
//! and low level, high temperature, flame failure, relief), kept as a table by class the company
//! can edit, and what provides each on the drawing. A protection is provided by an instrument whose
//! letters name it (a switch PSH, an alarm PAH, their trips PSHH, PAHH, an interlock PZHH), by a
//! transmitter whose setpoints name it, or, for relief, by a relief device on the equipment or its
//! lines or a relief path said in the equipment's data. Check 27 notes what is missing; the
//! Safety analysis list tabulates it all. A protection can be marked not needed for one piece of
//! equipment (its "Safety, not needed" field, such as "PSL, TSH").

use crate::checks::Issue;
use crate::graph::Net;
use crate::model::{Project, LINE_CLASS};
use crate::symbols;
use serde::{Deserialize, Serialize};

/// The protections Datum Schema knows: (code, what it is, the letters that provide it).
pub const CODES: [(&str, &str, &[&str]); 8] = [
    ("PSH", "high pressure", &["PSH", "PSHH", "PAH", "PAHH", "PZH", "PZHH"]),
    ("PSL", "low pressure", &["PSL", "PSLL", "PAL", "PALL", "PZL", "PZLL"]),
    ("LSH", "high level", &["LSH", "LSHH", "LAH", "LAHH", "LZH", "LZHH"]),
    ("LSL", "low level", &["LSL", "LSLL", "LAL", "LALL", "LZL", "LZLL"]),
    ("TSH", "high temperature", &["TSH", "TSHH", "TAH", "TAHH", "TZH", "TZHH"]),
    ("TSL", "low temperature", &["TSL", "TSLL", "TAL", "TALL"]),
    ("BSL", "flame failure", &["BSL", "BAL", "BZL", "BE", "BS"]),
    ("PSV", "relief", &[]),
];

/// What a class of equipment needs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Need {
    pub class: String,
    pub codes: Vec<String>,
}

/// The starter table, after the usual safety analysis of process components.
pub fn starter() -> Vec<Need> {
    let n = |c: &str, codes: &[&str]| Need { class: c.into(), codes: codes.iter().map(|s| s.to_string()).collect() };
    vec![
        n("Vessel", &["PSH", "PSL", "LSH", "LSL", "PSV"]),
        n("Tank", &["LSH", "LSL"]),
        n("Column", &["PSH", "LSH", "LSL", "PSV"]),
        n("Pump", &["PSL", "PSH"]),
        n("Compressor", &["PSH", "PSL", "TSH", "PSV"]),
        n("HeatExchanger", &["PSH", "PSV"]),
        n("Heater", &["TSH", "BSL", "PSV"]),
    ]
}

pub fn normalize(v: &mut Vec<Need>) {
    v.truncate(100);
    for n in v.iter_mut() {
        n.class = crate::clip(n.class.trim(), 80);
        n.codes.retain(|c| CODES.iter().any(|x| x.0 == c.as_str()));
        n.codes.dedup();
    }
    let mut seen = std::collections::HashSet::new();
    v.retain(|n| !n.class.is_empty() && seen.insert(n.class.clone()));
}

/// The codes a class needs: its own row, else its nearest parent's.
fn needs(p: &Project, class: &str) -> Vec<String> {
    let chain = crate::classes::chain(class);
    let names: Vec<&str> = if chain.is_empty() { vec![class] } else { chain.iter().map(|c| c.name.as_str()).collect() };
    names.iter().find_map(|n| p.settings.safety.iter().find(|x| x.class == *n)).map(|x| x.codes.clone()).unwrap_or_default()
}

/// One requirement and what meets it.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Row {
    pub item: u32,
    pub sheet: u32,
    pub tag: String,
    pub code: String,
    pub what: String,
    pub by: Vec<String>,
    /// ok · missing · not needed
    pub status: &'static str,
}

/// The safety analysis of every piece of equipment drawn on a P&ID.
pub fn analysis(p: &Project, net: &Net) -> Vec<Row> {
    let pfd: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    // Each instrument's or relief device's process source, once.
    let mut sources: Vec<(u32, String, &'static str)> = vec![];
    let mut seen_obj = std::collections::HashSet::new();
    for it in p.items.iter().filter(|i| i.obj != 0 && !pfd.contains(&i.sheet)) {
        let Some(s) = symbols::get(&it.sym) else { continue };
        let relief = matches!(it.base_key().as_str(), "psv" | "rupture" | "breather");
        if !(s.cat.instrument() || relief) || !seen_obj.insert(it.obj) {
            continue;
        }
        let Some((src, _)) = crate::datasheets::process_source(p, net, it) else { continue };
        let tag = p.tag_of(it).to_string();
        sources.push((src, tag, if relief { "relief" } else { "instrument" }));
    }
    let mut rows = vec![];
    let mut done = std::collections::HashSet::new();
    for it in p.items.iter().filter(|i| i.obj != 0 && !pfd.contains(&i.sheet)) {
        if !symbols::get(&it.sym).is_some_and(|s| s.cat.equipment()) || !done.insert(it.obj) {
            continue;
        }
        let Some(o) = p.obj(it.obj) else { continue };
        let codes = needs(p, &o.class);
        if codes.is_empty() {
            continue;
        }
        // The equipment and the lines on its nozzles (on every sheet that shows it).
        let mut mine: std::collections::HashSet<u32> = [o.id].into();
        for v in p.items.iter().filter(|i| i.obj == o.id) {
            for l in p.lines.iter().filter(|l| l.kind.process() && l.obj != 0 && [&l.a, &l.b].iter().any(|e| e.item().is_some_and(|x| x.0 == v.id))) {
                if p.obj(l.obj).is_some_and(|x| x.class == LINE_CLASS) {
                    mine.insert(l.obj);
                }
            }
        }
        let na: Vec<String> = o.data.get("safety_na").map(|v| v.text().split([',', ';', ' ']).map(|s| s.trim().to_uppercase()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
        let relief_said = o.data.get("relief").is_some_and(|v| !v.text().trim().is_empty());
        for code in codes {
            let Some(&(c, what, letters)) = CODES.iter().find(|x| x.0 == code) else { continue };
            let mut by: Vec<String> = vec![];
            for (src, tag, kind) in &sources {
                if !mine.contains(src) {
                    continue;
                }
                let ok = if c == "PSV" {
                    *kind == "relief"
                } else {
                    let (l, _) = crate::tags::loop_of(tag);
                    letters.iter().any(|x| l == *x) || {
                        // An instrument of that variable whose setpoints name the protection (an alarm in a transmitter or controller).
                        l.len() >= 2 && l.starts_with(&c[..1]) && p.objects.iter().find(|x| x.tag == *tag).and_then(|x| x.data.get("setpoints")).is_some_and(|v| {
                            let t = v.text().to_uppercase();
                            letters.iter().any(|w| t.split(|ch: char| !ch.is_ascii_alphanumeric()).any(|word| word == *w))
                        })
                    }
                };
                if ok && !by.contains(tag) {
                    by.push(tag.clone());
                }
            }
            if c == "PSV" && by.is_empty() && relief_said {
                by.push(format!("relief path: {}", o.data.get("relief").map(|v| v.text()).unwrap_or_default()));
            }
            let status = if !by.is_empty() { "ok" } else if na.iter().any(|x| x == c) { "not needed" } else { "missing" };
            rows.push(Row { item: it.id, sheet: it.sheet, tag: o.tag.clone(), code: c.into(), what: what.into(), by, status });
        }
    }
    rows
}

/// Check 27: a protection the equipment's class needs that nothing on the drawing provides.
pub fn checks(p: &Project, net: &Net) -> Vec<Issue> {
    let mut by_item: std::collections::BTreeMap<u32, Vec<Row>> = Default::default();
    for r in analysis(p, net).into_iter().filter(|r| r.status == "missing") {
        by_item.entry(r.item).or_default().push(r);
    }
    by_item
        .into_iter()
        .filter_map(|(item, rows)| {
            let it = p.item(item)?;
            let list = rows.iter().map(|r| format!("{} ({})", r.code, r.what)).collect::<Vec<_>>().join(", ");
            let who = if rows[0].tag.is_empty() { "A piece of equipment".to_string() } else { rows[0].tag.clone() };
            Some(Issue { rule: 27, level: "note", msg: format!("{who} has no {list} on the drawing: add it, or say it is not needed (Safety, not needed)."), ids: vec![item], sheet: it.sheet, at: [it.x, it.y] })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::Val;

    fn rows(d: &Doc, tag: &str) -> Vec<Row> {
        analysis(&d.p, &Net::new(&d.p)).into_iter().filter(|r| r.tag == tag).collect()
    }

    #[test]
    fn what_each_vessel_and_pump_has_and_lacks() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        // V-101: LT-101's setpoints name LAH and LAL, PSV-101 relieves it; no pressure switches.
        let v = rows(&d, "V-101");
        let st = |c: &str| v.iter().find(|r| r.code == c).map(|r| (r.status, r.by.clone())).unwrap();
        assert_eq!(st("LSH").0, "ok");
        assert_eq!(st("LSL").0, "ok");
        assert_eq!(st("PSV"), ("ok", vec!["PSV-101".to_string()]));
        assert_eq!(st("PSH").0, "missing");
        // Check 27 says so, once per piece of equipment.
        let n = |d: &Doc| crate::api::derived(&d.p, 0).issues.into_iter().filter(|i| i.rule == 27 && i.msg.starts_with("V-101")).count();
        assert_eq!(n(&d), 1);
        // A pressure switch on the vessel's outlet line provides PSH; PSL marked not needed.
        let line = d.p.lines.iter().find(|l| d.p.data_of(l.obj).get("seq").map(|v| v.text()) == Some("1007".into())).unwrap().id;
        let (x, y) = { let l = d.p.line(line).unwrap(); let a = crate::graph::end_pt(&d.p, &l.a); (a[0] + 10.0, a[1] + 15.0) };
        let s = d.p.sheets[0].id;
        let psh = d.apply(Op::AddItem { item: crate::model::Item { sheet: s, sym: "inst_field".into(), x, y: y - 20.0, ..Default::default() }, on: None, tag: "PSH-130".into(), obj: None }).unwrap()[0];
        d.apply(Op::AddLine { line: crate::model::Line { sheet: s, kind: crate::model::Kind::Minor, a: crate::model::End::Port { item: psh, port: 0 }, b: crate::model::End::Line { line, x, y }, ..Default::default() } }).unwrap();
        let v101 = d.p.objects.iter().find(|o| o.tag == "V-101").unwrap().id;
        d.apply(Op::SetData { ids: vec![v101], key: "safety_na".into(), val: Some(Val::Text("PSL".into())) }).unwrap();
        let v = rows(&d, "V-101");
        assert!(v.iter().all(|r| r.status != "missing"), "{v:#?}");
        assert_eq!(n(&d), 0);
        // The list tabulates it.
        let l = crate::lists::build(&d.p, "safety").unwrap();
        assert!(l.rows.iter().any(|r| r.cells.iter().any(|c| c == "PSH-130")));
        // Hostile tables are cleaned.
        let mut t = vec![Need { class: " ".into(), codes: vec![] }, Need { class: "Pump".into(), codes: vec!["XYZ".into(), "PSH".into()] }, Need { class: "Pump".into(), codes: vec![] }];
        normalize(&mut t);
        assert_eq!(t, vec![Need { class: "Pump".into(), codes: vec!["PSH".into()] }]);
    }
}
