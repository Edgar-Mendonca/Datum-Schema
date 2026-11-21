//! The handover package: everything an owner or operator takes over at the end of a project, in one
//! zip file. Tables follow the shape of the CFIHOS handover specification (tags, their classes and
//! property values, the plant breakdown, documents and which tags each document shows), written as
//! CSV with a header row; beside them each P&ID in DEXPI, the plant as one web page, the lists and
//! the datasheets in Excel, the open checks, and a README that says what each file is.
//!
//! The package is read from the drawing when it is made; nothing in the project changes. Values
//! are in the project's units, with the unit in its own column.

use crate::model::{Project, Val, LINE_CLASS, STREAM_CLASS};
use crate::symbols::{self, Cat};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

/// One CSV table: its header and rows.
struct Table {
    head: Vec<&'static str>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn new(head: &[&'static str]) -> Self {
        Self { head: head.to_vec(), rows: vec![] }
    }
    fn push(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }
    /// CSV, every cell quoted when it needs to be, text a spreadsheet would run as a formula made safe.
    fn csv(&self) -> String {
        let q = |s: &str| {
            let s = crate::lists::safe(s);
            if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
        };
        let mut out = self.head.iter().map(|h| q(h)).collect::<Vec<_>>().join(",");
        out.push_str("\r\n");
        for r in &self.rows {
            out.push_str(&r.iter().map(|c| q(c)).collect::<Vec<_>>().join(","));
            out.push_str("\r\n");
        }
        out
    }
}

/// What the package will hold, for the window to show before it is saved.
#[derive(Clone, Debug, Serialize, PartialEq, Default)]
pub struct Summary {
    pub tags: usize,
    pub drawn: usize,
    pub not_drawn: usize,
    pub properties: usize,
    pub documents: usize,
    pub files: Vec<String>,
    /// What is worth fixing before handing over (each says how many).
    pub gaps: Vec<String>,
}

/// A tagged object's row facts.
struct TagInfo {
    obj: u32,
    tag: String,
    class: String,
    class_label: String,
    discipline: &'static str,
    description: String,
    status: &'static str,
    node: u32,
}

fn discipline(cat: Option<Cat>, line: bool) -> &'static str {
    match (cat, line) {
        (_, true) => "Piping",
        (Some(Cat::Vessel | Cat::Rotating | Cat::Heat), _) => "Mechanical",
        (Some(Cat::Valve | Cat::Fitting), _) => "Piping",
        (Some(Cat::Control | Cat::Instrument | Cat::Primary), _) => "Instrumentation",
        _ => "Process",
    }
}

fn tags(p: &Project) -> Vec<TagInfo> {
    let pfd: BTreeSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    let mut cat: BTreeMap<u32, Cat> = BTreeMap::new();
    let mut drawn: BTreeSet<u32> = BTreeSet::new();
    for it in &p.items {
        if let Some(s) = symbols::get(&it.sym) {
            cat.entry(it.obj).or_insert(s.cat);
        }
        if !pfd.contains(&it.sheet) {
            drawn.insert(it.obj);
        }
    }
    for l in p.lines.iter().filter(|l| !pfd.contains(&l.sheet)) {
        drawn.insert(l.obj);
    }
    let mut out = vec![];
    for o in &p.objects {
        if o.class == STREAM_CLASS {
            continue;
        }
        let line = o.class == LINE_CLASS;
        let tag = if line { crate::graph::obj_number(p, o.id) } else { o.tag.clone() };
        if tag.trim().is_empty() || (line && !drawn.contains(&o.id)) {
            continue;
        }
        let registered = o.data.contains_key("~reg");
        let class_label = crate::classes::get(&o.class).map_or(o.class.clone(), |c| c.label.clone());
        let status = if drawn.contains(&o.id) { "Drawn" } else if registered { "Registered" } else { "Not drawn" };
        if status == "Not drawn" && !cat.contains_key(&o.id) {
            continue;
        }
        let mut description = o.data.get("service").or_else(|| o.data.get("function")).or_else(|| o.data.get("name")).map(|v| v.text()).unwrap_or_default();
        // A valve or fitting with no service: its type and the line it sits on ("Gate valve on 4\"-P-1001-A1A").
        if description.trim().is_empty() && !line {
            if let Some(it) = p.items.iter().find(|i| i.obj == o.id && !pfd.contains(&i.sheet)) {
                if symbols::get(&it.sym).is_some_and(|s| s.inline) {
                    let on = p.lines.iter().find(|l| l.kind.process() && [&l.a, &l.b].iter().any(|e| e.item().is_some_and(|x| x.0 == it.id))).map(|l| crate::graph::obj_number(p, l.obj)).filter(|n| !n.is_empty());
                    if let Some(n) = on {
                        description = format!("{class_label} on {n}");
                    }
                }
            }
        }
        out.push(TagInfo { obj: o.id, tag, class: o.class.clone(), class_label, discipline: discipline(cat.get(&o.id).copied(), line), description, status, node: o.node });
    }
    out.sort_by(|a, b| a.tag.cmp(&b.tag));
    out
}

fn node_code(p: &Project, id: u32) -> String {
    p.nodes.iter().find(|n| n.id == id).map(|n| n.name.clone()).unwrap_or_default()
}

/// A value in the project's units: (value, unit).
fn value(v: &Val, q: &str, us: bool) -> (String, String) {
    match v {
        Val::Num(x) if !q.is_empty() => {
            let unit = crate::units::shown(us).into_iter().find(|s| s.q == q).map(|s| s.label).unwrap_or_default();
            (crate::fmt_sig(crate::units::to_display(*x, q, us), 6), unit)
        }
        Val::Num(x) => (crate::fmt_sig(*x, 6), String::new()),
        Val::Text(t) => (t.clone(), String::new()),
    }
}

/// Every file of the package, by its path inside the zip.
pub fn files(p: &Project, stamp: &str) -> Vec<(String, Vec<u8>)> {
    let us = p.units == "US";
    let info = tags(p);
    let mut out: Vec<(String, Vec<u8>)> = vec![];
    let mut put = |name: &str, bytes: Vec<u8>| out.push((format!("handover/{name}"), bytes));

    // Tags.
    let mut t = Table::new(&["TAG_NUMBER", "TAG_DESCRIPTION", "TAG_CLASS_NAME", "TAG_CLASS_CODE", "DISCIPLINE", "PLANT_BREAKDOWN", "TAG_STATUS"]);
    for x in &info {
        t.push(vec![x.tag.clone(), x.description.clone(), x.class_label.clone(), x.class.clone(), x.discipline.into(), node_code(p, x.node), x.status.into()]);
    }
    put("tag.csv", t.csv().into_bytes());

    // Tag property values, in each class's order, then anything else (hidden keys left out).
    let mut v = Table::new(&["TAG_NUMBER", "PROPERTY_NAME", "PROPERTY_CODE", "PROPERTY_VALUE", "PROPERTY_VALUE_UOM"]);
    for x in &info {
        let Some(o) = p.obj(x.obj) else { continue };
        let attrs = crate::classes::attrs(Some(p), &o.class);
        let mut done = BTreeSet::new();
        for a in &attrs {
            if let Some(val) = o.data.get(&a.key) {
                let (s, u) = value(val, &a.q, us);
                if !s.trim().is_empty() {
                    v.push(vec![x.tag.clone(), a.label.clone(), a.key.clone(), s, u]);
                }
                done.insert(a.key.clone());
            }
        }
        for (k, val) in &o.data {
            if k.starts_with('~') || done.contains(k) {
                continue;
            }
            let (s, u) = value(val, "", us);
            if !s.trim().is_empty() {
                v.push(vec![x.tag.clone(), k.clone(), k.clone(), s, u]);
            }
        }
    }
    let properties = v.rows.len();
    put("tag_property_value.csv", v.csv().into_bytes());

    // Classes used, with their parents.
    let used: BTreeSet<String> = info.iter().map(|x| x.class.clone()).collect();
    let mut c = Table::new(&["TAG_CLASS_CODE", "TAG_CLASS_NAME", "PARENT_CLASS_CODE"]);
    let mut all: BTreeSet<String> = BTreeSet::new();
    for name in &used {
        for k in crate::classes::chain(name) {
            all.insert(k.name.clone());
        }
        all.insert(name.clone());
    }
    for name in &all {
        let k = crate::classes::get(name);
        c.push(vec![name.clone(), k.map_or(name.clone(), |k| k.label.clone()), k.and_then(|k| k.parent.clone()).unwrap_or_default()]);
    }
    put("tag_class.csv", c.csv().into_bytes());

    // The plant breakdown.
    let mut b = Table::new(&["PLANT_BREAKDOWN", "LEVEL", "PARENT"]);
    for n in &p.nodes {
        b.push(vec![n.name.clone(), n.level.clone(), node_code(p, n.parent)]);
    }
    put("plant_breakdown.csv", b.csv().into_bytes());

    // Documents and the tags each shows.
    let mut d = Table::new(&["DOCUMENT_NUMBER", "DOCUMENT_TITLE", "DOCUMENT_TYPE", "REVISION", "REVISION_DATE", "STATUS", "FILE"]);
    let mut dt = Table::new(&["DOCUMENT_NUMBER", "TAG_NUMBER"]);
    let tag_of = |obj: u32| info.iter().find(|x| x.obj == obj).map(|x| x.tag.clone());
    for s in &p.sheets {
        let kind = if s.legend { "Legend" } else if s.pfd { "Process flow diagram" } else { "P&ID" };
        let file = if s.legend || s.pfd { String::new() } else { format!("dexpi/{}.xml", file_name(&s.number)) };
        d.push(vec![s.number.clone(), s.title.clone(), kind.into(), s.rev.clone(), s.date.clone(), if s.status.is_empty() { "Draft".into() } else { s.status.clone() }, file]);
        let on: BTreeSet<String> = p.items.iter().filter(|i| i.sheet == s.id).map(|i| i.obj).chain(p.lines.iter().filter(|l| l.sheet == s.id).map(|l| l.obj)).filter_map(tag_of).collect();
        for tag in on {
            dt.push(vec![s.number.clone(), tag]);
        }
    }
    // Issued datasheets: one document per tag, at its last revision.
    let mut last: BTreeMap<u32, &crate::model::Issue> = BTreeMap::new();
    for i in &p.issues {
        last.insert(i.obj, i);
    }
    for (obj, i) in last {
        let Some(tag) = tag_of(obj) else { continue };
        let num = format!("DS-{tag}");
        d.push(vec![num.clone(), format!("Datasheet {tag}"), "Datasheet".into(), i.rev.clone(), i.date.clone(), i.status.clone(), "lists/datasheets.xlsx".into()]);
        dt.push(vec![num, tag]);
    }
    for k in &p.settings.testpacks.packs {
        d.push(vec![k.id.clone(), format!("Test pack {}", k.id), "Pressure test pack".into(), String::new(), k.date.clone(), k.status.clone(), format!("lists/test pack {}.xlsx", file_name(&k.id))]);
        for o in &k.lines {
            if let Some(tag) = tag_of(*o) {
                dt.push(vec![k.id.clone(), tag]);
            }
        }
    }
    if !p.settings.hazop.nodes.is_empty() {
        let num = "HAZOP".to_string();
        d.push(vec![num.clone(), if p.settings.hazop.title.is_empty() { "HAZOP worksheet".into() } else { p.settings.hazop.title.clone() }, "HAZOP worksheet".into(), String::new(), p.settings.hazop.date.clone(), String::new(), "lists/HAZOP worksheet.xlsx".into()]);
        let mut seen = BTreeSet::new();
        for n in &p.settings.hazop.nodes {
            for id in &n.ids {
                let obj = p.item(*id).map(|i| i.obj).or_else(|| p.line(*id).map(|l| l.obj)).unwrap_or(0);
                if let Some(tag) = tag_of(obj).filter(|t| seen.insert(t.clone())) {
                    dt.push(vec![num.clone(), tag]);
                }
            }
        }
    }
    let documents = d.rows.len();
    put("document.csv", d.csv().into_bytes());
    put("document_tag.csv", dt.csv().into_bytes());

    // The open checks.
    let net = crate::graph::Net::new(p);
    let hyd = crate::hydraulics::all(p, &net);
    let issues = crate::checks::run(p, &net, &hyd);
    let mut q = Table::new(&["RULE", "LEVEL", "SHEET", "MESSAGE"]);
    for i in &issues {
        q.push(vec![i.rule.to_string(), i.level.into(), p.sheet(i.sheet).map(|s| s.number.clone()).unwrap_or_default(), i.msg.clone()]);
    }
    put("open_checks.csv", q.csv().into_bytes());

    // Each P&ID in DEXPI.
    for s in p.sheets.iter().filter(|s| !s.legend && !s.pfd) {
        put(&format!("dexpi/{}.xml", file_name(&s.number)), crate::dexpi::write(p, s).into_bytes());
    }
    // The plant as one web page.
    put(&format!("{}.html", file_name(&p.name)), crate::webpage::page(p, stamp).into_bytes());
    // The lists and datasheets in Excel (only those with rows).
    let mut kinds: Vec<String> = ["lines", "equipment", "instruments", "valves", "register", "safety", "loads", "testpacks", "hazop", "hazop_recs", "datasheets"].iter().map(|s| s.to_string()).collect();
    kinds.extend(p.settings.testpacks.packs.iter().map(|k| format!("testpack:{}", k.id)));
    for k in kinds {
        let Ok(l) = crate::lists::build(p, &k) else { continue };
        if l.rows.is_empty() {
            continue;
        }
        if let Ok(bytes) = crate::lists::xlsx(p, &k) {
            put(&format!("lists/{}.xlsx", file_name(&l.title)), bytes);
        }
    }
    if !p.issues.is_empty() || p.items.iter().any(|i| symbols::get(&i.sym).is_some_and(|s| crate::datasheets::has_sheet(s.cat))) {
        if let Ok(bytes) = crate::datasheets::xlsx(p) {
            put("lists/datasheets.xlsx", bytes);
        }
    }
    let s = summary_of(&info, properties, documents, &issues, p);
    put("README.txt", readme(p, stamp, &s).into_bytes());
    out
}

/// A name safe for a file inside the package.
fn file_name(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || " -_&()".contains(c) { c } else { '-' }).collect();
    let t = t.trim().to_string();
    crate::clip(if t.is_empty() { "unnamed" } else { &t }, 80)
}

fn summary_of(info: &[TagInfo], properties: usize, documents: usize, issues: &[crate::checks::Issue], p: &Project) -> Summary {
    let mut gaps = vec![];
    let no_desc = info.iter().filter(|x| x.description.trim().is_empty() && x.class != LINE_CLASS).count();
    if no_desc > 0 {
        gaps.push(format!("{no_desc} tags have no description (service or function)."));
    }
    let unknown = info.iter().filter(|x| crate::classes::get(&x.class).is_none()).count();
    if unknown > 0 {
        gaps.push(format!("{unknown} tags have a class the class library does not know."));
    }
    let (e, w) = (issues.iter().filter(|i| i.level == "error").count(), issues.iter().filter(|i| i.level == "warning").count());
    if e + w > 0 {
        gaps.push(format!("{e} errors and {w} warnings are open in the checks (open_checks.csv)."));
    }
    let required = issues.iter().filter(|i| i.rule == 6).count();
    if required > 0 {
        gaps.push(format!("{required} objects miss required data (check 6)."));
    }
    let draft = p.sheets.iter().filter(|s| !s.legend && (s.status.is_empty() || s.status == "Draft")).count();
    if draft > 0 {
        gaps.push(format!("{draft} sheets are still Draft (Sheet Manager)."));
    }
    let open_recs = p.settings.hazop.nodes.iter().flat_map(|n| &n.rows).filter(|r| r.status == "Open").count();
    if open_recs > 0 {
        gaps.push(format!("{open_recs} HAZOP recommendations are open."));
    }
    let untested = p.settings.testpacks.packs.iter().filter(|k| k.status != "Accepted").count();
    if untested > 0 {
        gaps.push(format!("{untested} test packs are not accepted yet."));
    }
    Summary { tags: info.len(), drawn: info.iter().filter(|x| x.status == "Drawn").count(), not_drawn: info.iter().filter(|x| x.status != "Drawn").count(), properties, documents, files: vec![], gaps }
}

fn readme(p: &Project, stamp: &str, s: &Summary) -> String {
    let mut r = format!(
        "Handover package: {}\r\nMade with Datum Schema {} on {}. Values are in the project's units ({}), with the unit in its own column.\r\n\r\n",
        if p.title.project.is_empty() { &p.name } else { &p.title.project },
        env!("CARGO_PKG_VERSION"),
        if stamp.is_empty() { "the day it was saved" } else { stamp },
        p.units
    );
    r += "The tables follow the shape of the CFIHOS handover specification; they are CSV with a header row.\r\n\r\n";
    for (f, what) in [
        ("tag.csv", "every tag: equipment, valves, instruments and line numbers, with class, discipline, plant breakdown and status (drawn or registered)"),
        ("tag_property_value.csv", "each tag's data, one value per row, with its unit"),
        ("tag_class.csv", "the classes used, with their parents"),
        ("plant_breakdown.csv", "plant, units and areas"),
        ("document.csv", "the sheets, issued datasheets, test packs and the HAZOP worksheet, with revision and status"),
        ("document_tag.csv", "which tags each document shows"),
        ("open_checks.csv", "the checks still open when the package was made"),
        ("dexpi/", "each P&ID in the DEXPI P&ID exchange format (XML)"),
        ("<project>.html", "the plant as one web page: every sheet, click a tag for its data, search, the lists"),
        ("lists/", "the lists, test packs, HAZOP worksheet and datasheets in Excel"),
    ] {
        r += &format!("  {f:<24} {what}\r\n");
    }
    r += &format!("\r\n{} tags ({} drawn, {} registered or not drawn), {} property values, {} documents.\r\n", s.tags, s.drawn, s.not_drawn, s.properties, s.documents);
    if !s.gaps.is_empty() {
        r += "\r\nWorth fixing before handing over:\r\n";
        for g in &s.gaps {
            r += &format!("  - {g}\r\n");
        }
    }
    r += "\r\nHydraulic and sizing values are a sizing aid checked by the engineer.\r\n";
    r
}

/// What the package will hold.
pub fn summary(p: &Project) -> Summary {
    let fs = files(p, "");
    let info = tags(p);
    let net = crate::graph::Net::new(p);
    let hyd = crate::hydraulics::all(p, &net);
    let issues = crate::checks::run(p, &net, &hyd);
    let count = |name: &str| fs.iter().find(|f| f.0 == format!("handover/{name}")).map_or(0, |f| String::from_utf8_lossy(&f.1).lines().count().saturating_sub(1));
    let mut s = summary_of(&info, count("tag_property_value.csv"), count("document.csv"), &issues, p);
    s.files = fs.into_iter().map(|f| f.0.trim_start_matches("handover/").to_string()).collect();
    s
}

/// The package as a zip file.
pub fn zip(p: &Project, stamp: &str) -> Result<Vec<u8>, String> {
    let mut w = ::zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = ::zip::write::SimpleFileOptions::default().compression_method(::zip::CompressionMethod::Deflated);
    for (name, bytes) in files(p, stamp) {
        w.start_file(name, opts).map_err(|e| e.to_string())?;
        w.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    Ok(w.finish().map_err(|e| e.to_string())?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv_rows(fs: &[(String, Vec<u8>)], name: &str) -> Vec<Vec<String>> {
        let t = String::from_utf8(fs.iter().find(|f| f.0 == format!("handover/{name}")).unwrap_or_else(|| panic!("{name}")).1.clone()).unwrap();
        // CSV: quoted cells may hold commas and doubled quotes.
        t.lines()
            .map(|l| {
                let (mut cells, mut cur, mut quoted, mut chars) = (vec![], String::new(), false, l.chars().peekable());
                while let Some(c) = chars.next() {
                    match (c, quoted) {
                        ('"', true) if chars.peek() == Some(&'"') => {
                            cur.push('"');
                            chars.next();
                        }
                        ('"', _) => quoted = !quoted,
                        (',', false) => cells.push(std::mem::take(&mut cur)),
                        _ => cur.push(c),
                    }
                }
                cells.push(cur);
                cells
            })
            .collect()
    }

    #[test]
    fn the_worked_example_hands_over_its_tags_data_and_documents() {
        let mut p = crate::examples::get("worked").unwrap();
        crate::testpack::make(&mut p).unwrap();
        let fs = files(&p, "2026-11-21");
        let names: Vec<&str> = fs.iter().map(|f| f.0.as_str()).collect();
        for want in ["handover/tag.csv", "handover/tag_property_value.csv", "handover/tag_class.csv", "handover/plant_breakdown.csv", "handover/document.csv", "handover/document_tag.csv", "handover/open_checks.csv", "handover/dexpi/PID-110.xml", "handover/README.txt"] {
            assert!(names.contains(&want), "{want}: {names:?}");
        }
        assert!(names.iter().any(|n| n.ends_with(".html")) && names.iter().any(|n| n.starts_with("handover/lists/") && n.ends_with(".xlsx")));
        // Tags: the pump drawn, P-102 registered, a line number, each with class and discipline.
        let tags = csv_rows(&fs, "tag.csv");
        assert_eq!(tags[0][0], "TAG_NUMBER");
        let row = |t: &str| tags.iter().find(|r| r[0] == t).cloned().unwrap_or_else(|| panic!("{t}: {tags:?}"));
        assert_eq!((row("P-101A")[4].as_str(), row("P-101A")[6].as_str()), ("Mechanical", "Drawn"));
        assert_eq!(row("P-102")[6], "Registered");
        assert!(row("HV-101")[1].contains(" on 4\"-P-1001-A1A"), "{:?}", row("HV-101"));
        assert!(tags.iter().any(|r| r[0].contains("P-1003") && r[4] == "Piping"));
        // Its data with units; the documents and what they show.
        let props = csv_rows(&fs, "tag_property_value.csv");
        assert!(props.iter().any(|r| r[0] == "P-101A" && r[2] == "power" && r[3] == "7.5" && r[4] == "kW"), "{:?}", props.iter().filter(|r| r[0] == "P-101A").collect::<Vec<_>>());
        let docs = csv_rows(&fs, "document.csv");
        assert!(docs.iter().any(|r| r[0] == "PID-110" && r[2] == "P&ID" && r[5] == "Issued for approval"));
        assert!(docs.iter().any(|r| r[0] == "TP-001"));
        let dt = csv_rows(&fs, "document_tag.csv");
        assert!(dt.iter().any(|r| r[0] == "PID-110" && r[1] == "V-101"));
        // The README counts and says what to fix; the zip opens.
        let readme = String::from_utf8(fs.iter().find(|f| f.0.ends_with("README.txt")).unwrap().1.clone()).unwrap();
        assert!(readme.contains("tags (") && readme.contains("test packs are not accepted"));
        let z = zip(&p, "2026-11-21").unwrap();
        let mut a = ::zip::ZipArchive::new(std::io::Cursor::new(z)).unwrap();
        assert_eq!(a.len(), fs.len());
        assert!(a.by_name("handover/tag.csv").is_ok());
        let s = summary(&p);
        assert!(s.tags > 20 && s.not_drawn >= 1 && s.documents >= 5 && !s.files.is_empty());
    }

    #[test]
    fn every_example_hands_over_and_hostile_text_stays_text() {
        for ex in crate::examples::list().iter().map(|e| e.id) {
            let p = crate::examples::get(ex).unwrap();
            assert!(!zip(&p, "").unwrap().is_empty(), "{ex}");
        }
        let mut p = crate::examples::get("pump").unwrap();
        p.name = "../../evil:name".into();
        let o = p.objects.iter_mut().find(|o| o.tag == "P-101A").unwrap();
        o.data.insert("service".into(), Val::Text("=HYPERLINK(\"x\")".into()));
        let fs = files(&p, "");
        assert!(fs.iter().all(|f| !f.0.contains("..") && !f.0.contains(':')), "{:?}", fs.iter().map(|f| &f.0).collect::<Vec<_>>());
        let tag = String::from_utf8(fs.iter().find(|f| f.0.ends_with("tag.csv")).unwrap().1.clone()).unwrap();
        assert!(tag.contains("'=HYPERLINK") && !tag.contains(",=HYPERLINK"));
    }
}
