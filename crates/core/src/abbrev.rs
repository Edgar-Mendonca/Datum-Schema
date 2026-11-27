//! Abbreviations and codes: a company's or client's lists of fluid and service codes, pipe classes,
//! insulation codes, equipment tag prefixes and instrument letters. Read from the table on their
//! legend sheet (DXF, DWG or PDF) or from Excel or CSV, corrected by the user, named as they like
//! and kept in the project (`Settings.codes`) and, if they choose, on their computer for every
//! project. A project with codes of a kind has the drawing checked against them (check 28), the
//! Inspector suggests them, and its legend sheet lists the ones the drawing uses.

use crate::checks::Issue;
use crate::dxfin::{self, Ent};
use crate::model::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The kinds of code, with their headings.
pub const KINDS: [(&str, &str); 6] = [
    ("fluid", "Fluid and service codes"),
    ("class", "Pipe classes"),
    ("insulation", "Insulation and tracing codes"),
    ("equipment", "Equipment tag prefixes"),
    ("instrument", "Instrument letters"),
    ("other", "Other abbreviations"),
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Code {
    pub kind: String,
    pub code: String,
    pub meaning: String,
}

/// A named list of codes (`.dschemacodes` on the computer, or in a project).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct CodeList {
    pub name: String,
    /// Set once from its first name, never changed (a list renamed is the same list).
    pub id: String,
    /// The file it was read from.
    pub source: String,
    pub codes: Vec<Code>,
}

const MAX_CODES: usize = 5000;

/// The kind a heading names ("FLUID CODES", "Piping classes", "INSULATION"…).
pub fn kind_of(heading: &str) -> Option<&'static str> {
    let u = heading.to_uppercase();
    let has = |w: &[&str]| w.iter().any(|x| u.contains(x));
    if has(&["INSUL", "TRACING", "TRACED", "JACKET"]) {
        Some("insulation")
    } else if has(&["FLUID", "SERVICE", "COMMODIT", "MEDIUM", "MEDIA"]) {
        Some("fluid")
    } else if has(&["PIPING CLASS", "PIPE CLASS", "PIPING SPEC", "PIPE SPEC", "MATERIAL CLASS", "LINE CLASS"]) {
        Some("class")
    } else if has(&["EQUIPMENT", "EQUIP."]) {
        Some("equipment")
    } else if has(&["INSTRUMENT", "IDENTIFICATION LETTER", "MEASURED VARIABLE", "FIRST LETTER"]) {
        Some("instrument")
    } else if has(&["ABBREVIATION", "GENERAL"]) {
        Some("other")
    } else {
        None
    }
}

/// A list made safe and tidy: names and codes trimmed and clipped, known kinds, no empty or
/// repeated codes (of the same kind), at most `MAX_CODES`.
pub fn validate(mut l: CodeList) -> CodeList {
    l.name = crate::clip(l.name.trim(), 80);
    if l.name.is_empty() {
        l.name = "Codes".into();
    }
    if l.id.trim().is_empty() || crate::symlib::slug(&l.id) != l.id {
        l.id = crate::symlib::slug(if l.id.trim().is_empty() { &l.name } else { &l.id });
    }
    if l.id.is_empty() {
        l.id = "codes".into();
    }
    l.source = crate::clip(&l.source, 200);
    let mut seen = std::collections::HashSet::new();
    l.codes = std::mem::take(&mut l.codes)
        .into_iter()
        .map(|mut c| {
            c.code = crate::clip(c.code.trim(), 24);
            c.meaning = crate::clip(c.meaning.trim(), 200);
            if !KINDS.iter().any(|k| k.0 == c.kind) {
                c.kind = "other".into();
            }
            c
        })
        .filter(|c| !c.code.is_empty() && seen.insert((c.kind.clone(), c.code.to_uppercase())))
        .take(MAX_CODES)
        .collect();
    l
}

/// Does a text look like a code (short, no sentence): "P", "PW", "A1A", "HC-50", "ET"?
fn code_like(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.chars().count() <= 12 && t.split_whitespace().count() <= 2 && t.chars().any(|c| c.is_ascii_alphanumeric()) && !t.ends_with(':')
}

/// The codes in a drawing's tables (a DXF, DWG or PDF as `dxfin::source` gives it): its texts in
/// rows; a row of one text that names a kind starts a section; in other rows the cells are paired
/// (code, meaning), so tables side by side are read too. Rows before any heading are "other".
pub fn read_drawing(text: &str, name: &str) -> Result<(CodeList, Vec<String>), String> {
    let (d, mut warnings) = dxfin::load(text)?;
    let mut texts: Vec<([f64; 2], f64, String)> = d.ents.iter().filter_map(|e| match e { Ent::Text { p, h, s, .. } if !s.trim().is_empty() => Some(([p[0] * d.mm, p[1] * d.mm], (h * d.mm).max(0.5), s.trim().to_string())), _ => None }).collect();
    // Block attributes (a table drawn as blocks) count as texts too.
    for e in &d.ents {
        if let Ent::Insert { attribs, .. } = e {
            texts.extend(attribs.iter().filter(|a| a.visible && !a.value.trim().is_empty()).map(|a| ([a.p[0] * d.mm, a.p[1] * d.mm], (a.h * d.mm).max(0.5), a.value.trim().to_string())));
        }
    }
    if texts.is_empty() {
        return Err("That drawing has no text to read codes from.".into());
    }
    // Rows: top to bottom (y up), texts within half a text height of the row's first.
    texts.sort_by(|a, b| b.0[1].total_cmp(&a.0[1]).then(a.0[0].total_cmp(&b.0[0])));
    let mut rows: Vec<Vec<([f64; 2], f64, String)>> = vec![];
    for t in texts {
        match rows.last_mut() {
            Some(r) if (r[0].0[1] - t.0[1]).abs() <= 0.6 * r[0].1.max(t.1) => r.push(t),
            _ => rows.push(vec![t]),
        }
    }
    let mut kind: &str = "other";
    // Per column of tables side by side, the kind its heading set.
    let mut col_kind: Vec<(f64, &str)> = vec![];
    let mut codes = vec![];
    for mut r in rows {
        r.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]));
        // Cells: texts closer than 4 text heights are one cell.
        let mut cells: Vec<([f64; 2], String)> = vec![];
        let mut last_end = f64::MIN;
        for (p, h, s) in &r {
            let w = 0.6 * h * s.chars().count() as f64;
            match cells.last_mut() {
                Some(c) if p[0] - last_end < 4.0 * h => {
                    c.1.push(' ');
                    c.1.push_str(s);
                }
                _ => cells.push((*p, s.clone())),
            }
            last_end = p[0] + w;
        }
        // Headings: a cell that names a kind, alone or with others that do.
        let heads: Vec<(f64, &'static str)> = cells.iter().filter_map(|c| kind_of(&c.1).filter(|_| !code_like(&c.1) || c.1.len() > 8).map(|k| (c.0[0], k))).collect();
        if !heads.is_empty() && heads.len() * 2 >= cells.len() {
            if heads.len() == 1 && cells.len() == 1 {
                kind = heads[0].1;
                col_kind.retain(|c| (c.0 - heads[0].0).abs() > 5.0);
            }
            col_kind.extend(heads);
            continue;
        }
        let mut i = 0;
        while i + 1 < cells.len() {
            let (c, m) = (&cells[i], &cells[i + 1]);
            if code_like(&c.1) && !m.1.is_empty() {
                let k = col_kind.iter().filter(|x| x.0 <= c.0[0] + 5.0).max_by(|a, b| a.0.total_cmp(&b.0)).map_or(kind, |x| x.1);
                codes.push(Code { kind: k.into(), code: c.1.clone(), meaning: m.1.clone() });
                i += 2;
            } else {
                i += 1;
            }
        }
    }
    if codes.is_empty() {
        return Err("No codes were found: a table of codes has each code with its meaning beside it, on the same row.".into());
    }
    let l = validate(CodeList { name: name.into(), id: String::new(), source: String::new(), codes });
    if l.codes.iter().all(|c| c.kind == "other") {
        warnings.push("No headings were recognised (fluid codes, pipe classes, insulation…): set each code's kind in the table.".into());
    }
    Ok((l, warnings))
}

/// The codes in a workbook or CSV: a header row naming the code and meaning columns (and a kind
/// column if there is one), or else the first two columns; each worksheet's name can give the
/// kind ("Fluids", "Insulation").
pub fn read_table(bytes: &[u8], name: &str) -> Result<CodeList, String> {
    let grids: Vec<crate::import::Grid> = if bytes.starts_with(b"PK") {
        crate::import::read(bytes)?
    } else {
        let text = String::from_utf8_lossy(bytes);
        let sep = if text.lines().next().is_some_and(|l| l.matches(';').count() > l.matches(',').count()) { ';' } else { ',' };
        vec![(String::new(), text.lines().take(MAX_CODES + 10).map(|l| csv_row(l, sep)).collect())]
    };
    let mut codes = vec![];
    for (sheet, rows) in grids {
        let head = rows.first().cloned().unwrap_or_default();
        let find = |w: &[&str]| head.iter().position(|h| { let h = h.to_lowercase(); w.iter().any(|x| h.contains(x)) });
        let (ci, mi, ki) = (find(&["code", "abbrev", "letter", "prefix", "symbol"]), find(&["meaning", "description", "service", "name", "definition"]), find(&["kind", "type", "category"]));
        let has_head = ci.is_some() && mi.is_some();
        let (ci, mi) = (ci.unwrap_or(0), mi.unwrap_or(1));
        let sheet_kind = kind_of(&sheet).unwrap_or("other");
        for r in rows.iter().skip(usize::from(has_head)) {
            let cell = |i: usize| r.get(i).map(|x| x.trim().to_string()).unwrap_or_default();
            let (c, m) = (cell(ci), cell(mi));
            if c.is_empty() || m.is_empty() {
                continue;
            }
            let k = ki.map(cell).and_then(|x| kind_of(&x).or(KINDS.iter().find(|k| k.0.eq_ignore_ascii_case(x.trim())).map(|k| k.0))).unwrap_or(sheet_kind);
            codes.push(Code { kind: k.into(), code: c, meaning: m });
        }
    }
    if codes.is_empty() {
        return Err("No codes were found: give a column of codes and a column of their meanings (a header row such as Code, Meaning, Kind helps).".into());
    }
    Ok(validate(CodeList { name: name.into(), id: String::new(), source: String::new(), codes }))
}

fn csv_row(l: &str, sep: char) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut q = false;
    let mut it = l.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' if q && it.peek() == Some(&'"') => {
                cur.push('"');
                it.next();
            }
            '"' => q = !q,
            c if c == sep && !q => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// The code lists kept on this computer (`.dschemacodes` files in `dir`), by name, with their files.
pub fn load_dir(dir: &std::path::Path) -> Vec<(String, CodeList)> {
    let mut v: Vec<(String, CodeList)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "dschemacodes"))
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path()).ok().filter(|t| t.len() < (8 << 20))?;
            serde_json::from_str::<CodeList>(&text).ok().map(|l| (e.file_name().to_string_lossy().into_owned(), validate(l)))
        })
        .collect();
    v.sort_by_key(|a| a.1.name.to_lowercase());
    v
}

/// Saves a code list on this computer (its file named from its id, so a rename keeps the file).
pub fn save(dir: &std::path::Path, l: CodeList) -> Result<String, String> {
    let l = validate(l);
    let file = format!("{}.dschemacodes", l.id);
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not save the codes: {e}"))?;
    let text = serde_json::to_string_pretty(&l).map_err(|e| e.to_string())?;
    crate::doc::write_atomic(&dir.join(&file), text.as_bytes())?;
    Ok(file)
}

/// The project's codes of a kind (upper case → meaning).
pub fn of_kind(p: &Project, kind: &str) -> BTreeMap<String, String> {
    p.settings.codes.iter().flat_map(|l| l.codes.iter()).filter(|c| c.kind == kind).map(|c| (c.code.to_uppercase(), c.meaning.clone())).collect()
}

/// What the drawing uses, by kind: (kind, code as written) for line fluid, pipe class and
/// insulation codes, equipment tag prefixes and instrument first letters.
fn used(p: &Project) -> Vec<(&'static str, String, Vec<u32>)> {
    let mut v: Vec<(&'static str, String, Vec<u32>)> = vec![];
    let mut add = |k: &'static str, c: String, id: u32| {
        let c = c.trim().to_string();
        if c.is_empty() {
            return;
        }
        match v.iter_mut().find(|x| x.0 == k && x.1.eq_ignore_ascii_case(&c)) {
            Some(x) => x.2.push(id),
            None => v.push((k, c, vec![id])),
        }
    };
    let pfd: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| s.pfd || s.legend).map(|s| s.id).collect();
    for l in p.lines.iter().filter(|l| l.kind.process() && !pfd.contains(&l.sheet)) {
        let d = p.data_of(l.obj);
        for (k, key) in [("fluid", "fluid"), ("class", "class"), ("insulation", "insulation")] {
            if let Some(x) = d.get(key) {
                add(k, x.text(), l.id);
            }
        }
    }
    for it in p.items.iter().filter(|i| !pfd.contains(&i.sheet)) {
        let tag = p.tag_of(it);
        let (letters, num, _) = crate::tags::split(tag);
        let letters = letters.trim_end_matches(['-', '_', ' ']).to_string();
        if letters.is_empty() || num.is_none() {
            continue;
        }
        match crate::symbols::get(&it.sym).map(|s| s.cat) {
            Some(crate::symbols::Cat::Vessel | crate::symbols::Cat::Rotating | crate::symbols::Cat::Heat) => add("equipment", letters, it.id),
            Some(crate::symbols::Cat::Instrument) => add("instrument", letters.chars().next().map(String::from).unwrap_or_default(), it.id),
            _ => {}
        }
    }
    v
}

/// Check 28: a code on the drawing that the project's list of that kind does not have (only for
/// kinds the project has a list of).
pub fn checks(p: &Project) -> Vec<Issue> {
    if p.settings.codes.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    for (kind, code, ids) in used(p) {
        let known = of_kind(p, kind);
        if known.is_empty() || known.contains_key(&code.to_uppercase()) {
            continue;
        }
        let what = match kind {
            "fluid" => "fluid code",
            "class" => "pipe class",
            "insulation" => "insulation code",
            "equipment" => "equipment tag prefix",
            _ => "instrument first letter",
        };
        let (sheet, at) = p.items.iter().find(|i| i.id == ids[0]).map(|i| (i.sheet, [i.x, i.y])).or_else(|| p.lines.iter().find(|l| l.id == ids[0]).map(|l| (l.sheet, crate::graph::end_pt(p, &l.a)))).unwrap_or((0, [0.0, 0.0]));
        let n = ids.len();
        out.push(Issue { rule: 28, level: "warning", msg: format!("The {what} {code} ({n} {}) is not in the project's list of {}: add it to the list, or use a code from it.", if n == 1 { "place" } else { "places" }, KINDS.iter().find(|k| k.0 == kind).map_or("codes", |k| k.1).to_lowercase()), ids, sheet, at });
    }
    out
}

/// The codes the drawing uses that the project's lists explain, by kind, for the legend sheet.
pub fn for_legend(p: &Project) -> Vec<(&'static str, Vec<(String, String)>)> {
    let u = used(p);
    KINDS
        .iter()
        .filter_map(|(k, head)| {
            let known = of_kind(p, k);
            let mut v: Vec<(String, String)> = u.iter().filter(|x| x.0 == *k).filter_map(|x| known.get(&x.1.to_uppercase()).map(|m| (x.1.to_uppercase(), m.clone()))).collect();
            v.sort();
            v.dedup();
            (!v.is_empty()).then_some((*head, v))
        })
        .collect()
}

/// Every code of a kind, for suggestions as the user types.
pub fn suggestions(p: &Project) -> BTreeMap<String, Vec<(String, String)>> {
    let mut m: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for c in p.settings.codes.iter().flat_map(|l| l.codes.iter()) {
        m.entry(c.kind.clone()).or_default().push((c.code.clone(), c.meaning.clone()));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    /// A legend sheet's code tables as DXF text: two tables side by side, each under its heading.
    fn sheet() -> String {
        let mut e = String::new();
        let t = |e: &mut String, x: f64, y: f64, s: &str| e.push_str(&format!("0\nTEXT\n8\nTEXT\n10\n{x}\n20\n{y}\n40\n2.5\n1\n{s}\n"));
        t(&mut e, 10.0, 200.0, "FLUID CODES");
        t(&mut e, 150.0, 200.0, "INSULATION CODES");
        for (i, (c, m, ic, im)) in [("P", "Process", "H", "Hot insulation"), ("CW", "Cooling water", "C", "Cold insulation"), ("ST", "Steam", "ET", "Electrically traced"), ("N", "Nitrogen", "PP", "Personnel protection")].iter().enumerate() {
            let y = 190.0 - 6.0 * i as f64;
            t(&mut e, 10.0, y, c);
            t(&mut e, 30.0, y, m);
            t(&mut e, 150.0, y, ic);
            t(&mut e, 170.0, y, im);
        }
        t(&mut e, 10.0, 150.0, "PIPING CLASSES");
        t(&mut e, 10.0, 144.0, "A1A");
        t(&mut e, 30.0, 144.0, "150# carbon steel, 1.5 mm corrosion allowance");
        format!("0\nSECTION\n2\nENTITIES\n{e}0\nENDSEC\n0\nEOF\n")
    }

    #[test]
    fn code_lists_are_saved_on_the_computer() {
        let dir = std::env::temp_dir().join(format!("dschema-codes-{}", std::process::id()));
        let l = read_table(b"P,Process\nN,Nitrogen\n", "Client X codes").unwrap();
        let f = save(&dir, l.clone()).unwrap();
        assert_eq!(f, "client-x-codes.dschemacodes");
        let mut renamed = l.clone();
        renamed.name = "Client X, 2026".into();
        assert_eq!(save(&dir, renamed).unwrap(), f, "a rename keeps its file");
        let all = load_dir(&dir);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].1.name, "Client X, 2026");
        assert_eq!(all[0].1.codes, l.codes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn codes_are_read_from_a_drawings_tables() {
        let (l, w) = read_drawing(&sheet(), "Client codes").unwrap();
        assert!(w.is_empty(), "{w:?}");
        let has = |k: &str, c: &str, m: &str| l.codes.iter().any(|x| x.kind == k && x.code == c && x.meaning == m);
        assert!(has("fluid", "CW", "Cooling water"), "{:?}", l.codes);
        assert!(has("insulation", "ET", "Electrically traced"), "{:?}", l.codes);
        assert!(has("class", "A1A", "150# carbon steel, 1.5 mm corrosion allowance"), "{:?}", l.codes);
        assert_eq!(l.codes.len(), 9);
        assert_eq!(l.id, "client-codes");
        assert!(read_drawing("0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n", "x").is_err());
    }

    #[test]
    fn codes_are_read_from_a_csv_and_a_workbook() {
        let csv = "Code;Meaning;Kind\nP;Process;Fluid codes\n\"HC\";\"Hydrocarbon; flammable\";fluid\nH;Hot insulation;Insulation\n;;\n";
        let l = read_table(csv.as_bytes(), "From CSV").unwrap();
        assert_eq!(l.codes.len(), 3, "{:?}", l.codes);
        assert!(l.codes.iter().any(|c| c.code == "HC" && c.meaning == "Hydrocarbon; flammable" && c.kind == "fluid"));
        assert!(l.codes.iter().any(|c| c.code == "H" && c.kind == "insulation"));
        // Two plain columns and no header.
        let l2 = read_table(b"PW,Potable water\nIA,Instrument air\n", "Plain").unwrap();
        assert_eq!(l2.codes.len(), 2);
        assert!(read_table(b"", "Empty").is_err());
    }

    #[test]
    fn the_drawing_is_checked_against_the_projects_codes_and_the_legend_lists_them() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        assert!(checks(&d.p).is_empty(), "no codes, no check");
        let fluids: Vec<String> = used(&d.p).into_iter().filter(|x| x.0 == "fluid").map(|x| x.1).collect();
        assert!(!fluids.is_empty());
        // A list with every fluid but the first, and equipment prefixes without P.
        let mut codes: Vec<Code> = fluids.iter().skip(1).map(|f| Code { kind: "fluid".into(), code: f.clone(), meaning: format!("Fluid {f}") }).collect();
        codes.push(Code { kind: "equipment".into(), code: "T".into(), meaning: "Tank".into() });
        let mut st = d.p.settings.clone();
        st.codes.push(CodeList { name: "Client".into(), codes, ..CodeList::default() });
        d.apply(Op::SetSettings { settings: Box::new(st) }).unwrap();
        let v = checks(&d.p);
        assert!(v.iter().any(|i| i.msg.contains(&format!("fluid code {}", fluids[0]))), "{v:?}");
        assert!(v.iter().any(|i| i.msg.contains("equipment tag prefix P ")), "{v:?}");
        assert!(v.iter().all(|i| i.rule == 28 && !i.ids.is_empty()));
        // Through the checks, numbered 28; off when switched off.
        let net = crate::graph::Net::new(&d.p);
        let hyd = crate::hydraulics::all(&d.p, &net);
        assert!(crate::checks::run(&d.p, &net, &hyd).iter().any(|i| i.rule == 28));
        // The legend lists the codes used that the list explains.
        let lg = for_legend(&d.p);
        assert!(lg.iter().any(|(h, v)| *h == "Equipment tag prefixes" && v.iter().any(|x| x.0 == "T")), "{lg:?}");
        assert!(!lg.iter().any(|(_, v)| v.iter().any(|x| x.0 == fluids[0].to_uppercase())));
        assert_eq!(d.p.settings.codes[0].id, "client");
    }
}
