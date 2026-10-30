//! Datasheet import from Excel: values typed in a workbook (a vendor's datasheets, an older
//! project's index) are matched to the project's datasheets by tag and shown as a plan before
//! anything changes. Two layouts are read:
//!
//! - a table: a header row with a "Tag" column, one row per instrument, one column per attribute
//!   ("Manufacturer", "Set pressure (bar)");
//! - one worksheet per datasheet, as Datum Schema exports them: the tag in the sheet name or in the
//!   first rows, then rows of label, value and unit.
//!
//! Columns and rows are matched to datasheet rows by label (or attribute key), ignoring case, a
//! required mark and a unit in brackets. Only the object's own values are imported. Values that
//! come from the line, the vessel or the drawing are never written: where the file disagrees, the
//! plan says so, and the value is changed where it lives. Blank cells never clear anything.

use crate::datasheets::{self, Datasheet, Row};
use crate::graph::Net;
use crate::model::{Project, Val};
use crate::units;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

/// One value of the plan.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Change {
    pub obj: u32,
    pub tag: String,
    /// The attribute (empty for a value taken from the line, vessel or drawing).
    pub key: String,
    pub label: String,
    /// What the datasheet says now and what the file says, both as shown (display units).
    pub old: String,
    pub new: String,
    /// The value to write, in SI units ("set" only).
    pub val: Option<Val>,
    /// set: will be written · same: already so · conflict: the file disagrees with a value that
    /// lives on the line, vessel or drawing · bad: the file's value cannot be used.
    pub kind: String,
    pub note: String,
    /// The worksheet it came from.
    pub sheet: String,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Plan {
    pub changes: Vec<Change>,
    /// Tags in the file that match no datasheet.
    pub unmatched: Vec<String>,
    /// Column or row labels that match no datasheet row.
    pub unknown: Vec<String>,
    /// Worksheets read, and how many matched a datasheet.
    pub sheets: usize,
    pub used: usize,
}

const MAX_SHEETS: usize = 500;
const MAX_ROWS: usize = 5000;
const MAX_COLS: usize = 200;

/// A worksheet's name and its cells as text, row by row.
pub type Grid = (String, Vec<Vec<String>>);

/// The workbook's worksheets as text grids (blank cells are empty strings).
pub fn read(bytes: &[u8]) -> Result<Vec<Grid>, String> {
    use calamine::Reader;
    if bytes.len() > crate::doc::MAX_FILE as usize {
        return Err("That workbook is too large to read.".into());
    }
    let mut wb: calamine::Xlsx<_> = calamine::open_workbook_from_rs(std::io::Cursor::new(bytes)).map_err(|e| format!("This is not an Excel workbook (.xlsx) Datum Schema can read ({e})."))?;
    let names: Vec<String> = wb.sheet_names().into_iter().take(MAX_SHEETS).collect();
    let mut out = vec![];
    for n in names {
        let Ok(range) = wb.worksheet_range(&n) else { continue };
        let grid: Vec<Vec<String>> = range.rows().take(MAX_ROWS).map(|r| r.iter().take(MAX_COLS).map(|c| crate::clip(c.to_string().trim(), 1000)).collect()).collect();
        out.push((crate::clip(&n, 100), grid));
    }
    Ok(out)
}

/// A label made comparable: lower case, no required mark, no unit in brackets, single spaces.
/// Returns it with the unit it named, if any.
fn norm(s: &str) -> (String, String) {
    let s = s.trim().trim_end_matches('*').trim();
    let (head, unit) = match (s.rfind('('), s.ends_with(')')) {
        (Some(i), true) => (&s[..i], s[i + 1..s.len() - 1].trim().to_string()),
        _ => (s, String::new()),
    };
    (head.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase(), unit)
}

/// A unit as typed (m³/h, kg/m³, °C, psig) to the unit table's form (m^3/h, kg/m^3, °C, psi).
fn unit_expr(u: &str) -> String {
    let u = u.trim().replace('³', "^3").replace('²', "^2").replace('·', "*").replace("deg C", "°C").replace("deg F", "°F").replace("degC", "°C").replace("degF", "°F");
    // Gauge marks: the file's gauge pressure is taken as the pressure the datasheet shows.
    u.strip_suffix('g').filter(|x| ["bar", "psi", "kPa", "MPa"].contains(x)).map_or(u.clone(), |x| x.to_string())
}

/// A number typed with or without a unit ("4.5", "4,5", "4.5 bar") and the unit after it.
fn number(s: &str) -> Option<(f64, String)> {
    let s = s.trim();
    let end = s.char_indices().find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || c == ',' || c == 'e' || c == 'E' || ((c == '-' || c == '+') && (i == 0 || s[..i].ends_with(['e', 'E']))))).map_or(s.len(), |x| x.0);
    let (num, rest) = s.split_at(end);
    let v: f64 = num.replace(',', ".").parse().ok()?;
    v.is_finite().then(|| (v, rest.trim().to_string()))
}

/// A value for a row with a quantity, in SI units: the unit typed with it, else the column's, else
/// the project's display unit. Errors say why it cannot be used.
fn quantity(text: &str, col_unit: &str, q: &str, us: bool) -> Result<f64, String> {
    let (v, own) = number(text).ok_or_else(|| format!("“{text}” is not a number."))?;
    let unit = if own.is_empty() { col_unit.to_string() } else { own };
    if unit.is_empty() {
        return Ok(units::from_display(v, q, us));
    }
    let want = units::QUANTITIES.iter().find(|x| x.0 == q).map(|x| x.2).filter(|s| !s.is_empty());
    let (f, dim, off) = units::expr(&unit_expr(&unit)).map_err(|_| format!("“{unit}” is not a unit Datum Schema knows."))?;
    if let Some(si) = want {
        let (_, wd, _) = units::expr(si).map_err(|e| e.to_string())?;
        if !units::same(&dim, &wd) {
            return Err(format!("{unit} is not a unit of the same kind as {}.", units::pretty(si)));
        }
    }
    Ok(f * v + off)
}

/// A number with its unit in SI, if the unit is known.
fn si(v: f64, unit: &str) -> Option<(f64, units::Dim)> {
    let (f, dim, off) = units::expr(&unit_expr(unit)).ok()?;
    Some((f * v + off, dim))
}

/// The file's value says the same as a datasheet row (as text, or as numbers in any units of the
/// same kind, to the four figures a datasheet shows).
fn agrees(text: &str, col_unit: &str, r: &Row) -> bool {
    let shown = format!("{} {}", r.value, r.unit);
    if text.eq_ignore_ascii_case(r.value.trim()) || text.eq_ignore_ascii_case(shown.trim()) {
        return true;
    }
    let (Some((a, ua)), Some((b, _))) = (number(text), number(&r.value)) else { return false };
    let ua = if ua.is_empty() { col_unit.to_string() } else { ua };
    let close = |x: f64, y: f64| (x - y).abs() <= 2e-3 * x.abs().max(y.abs()).max(1e-12);
    if ua.is_empty() || r.unit.is_empty() || ua == r.unit {
        return close(a, b);
    }
    match (si(a, &ua), si(b, &r.unit)) {
        (Some((x, dx)), Some((y, dy))) => units::same(&dx, &dy) && close(x, y),
        _ => false,
    }
}

/// The datasheet row a label names: by label, then by attribute key.
fn find<'a>(rows: &'a [(String, &'a Row)], label: &str) -> Option<&'a Row> {
    rows.iter().find(|(l, _)| l == label).or_else(|| rows.iter().find(|(_, r)| !r.key.is_empty() && r.key.to_lowercase() == label.replace(' ', "_"))).map(|x| x.1)
}

struct Ctx<'a> {
    p: &'a Project,
    us: bool,
    plan: Plan,
    unknown: BTreeSet<String>,
    unmatched: BTreeSet<String>,
}

impl Ctx<'_> {
    /// One value from the file for one datasheet.
    fn value(&mut self, d: &Datasheet, rows: &[(String, &Row)], label: &str, col_unit: &str, text: &str, sheet: &str) {
        let text = text.trim();
        let (l, unit_in_label) = norm(label);
        if text.is_empty() || l.is_empty() || l == "tag" {
            return;
        }
        let Some(r) = find(rows, &l) else {
            self.unknown.insert(label.trim().trim_end_matches('*').trim().to_string());
            return;
        };
        let col_unit = if col_unit.is_empty() { unit_in_label.as_str() } else { col_unit };
        let shown = format!("{} {}", r.value, r.unit).trim().to_string();
        let mut c = Change { obj: d.obj, tag: d.tag.clone(), key: r.key.clone(), label: r.label.clone(), old: shown.clone(), new: text.to_string(), val: None, kind: String::new(), note: String::new(), sheet: sheet.to_string() };
        if r.key.is_empty() {
            // A value that lives elsewhere: never written; reported where it disagrees.
            if agrees(text, col_unit, r) || r.label == "Tag" {
                return;
            }
            c.kind = "conflict".into();
            c.note = format!("Comes from {}: change it there if the file is right.", if r.source.is_empty() { "the drawing" } else { &r.source });
            self.plan.changes.push(c);
            return;
        }
        let obj = self.p.obj(d.obj);
        let cur = obj.and_then(|o| o.data.get(&r.key));
        if !r.q.is_empty() {
            match quantity(text, col_unit, &r.q, self.us) {
                Ok(si) => {
                    c.new = units::show(si, &r.q, self.us);
                    let same = matches!(cur, Some(Val::Num(x)) if (x - si).abs() <= 1e-9 * si.abs().max(1e-12));
                    c.kind = if same { "same" } else { "set" }.into();
                    c.val = (!same).then_some(Val::Num(si));
                }
                Err(e) => (c.kind, c.note) = ("bad".into(), e),
            }
        } else if !r.choices.is_empty() {
            match r.choices.iter().find(|ch| ch.eq_ignore_ascii_case(text) || ch.replace('–', "-").eq_ignore_ascii_case(&text.replace('–', "-"))) {
                Some(ch) => {
                    let same = matches!(cur, Some(Val::Text(t)) if t == ch);
                    c.new = ch.clone();
                    c.kind = if same { "same" } else { "set" }.into();
                    c.val = (!same).then(|| Val::Text(ch.clone()));
                }
                None => (c.kind, c.note) = ("bad".into(), format!("Not one of: {}.", r.choices.join(", "))),
            }
        } else {
            let same = cur.is_some_and(|v| v.text() == text);
            c.kind = if same { "same" } else { "set" }.into();
            c.val = (!same).then(|| Val::Text(crate::clip(text, 1000)));
        }
        self.plan.changes.push(c);
    }
}

/// What importing a workbook would change: nothing is written until the plan is applied.
pub fn plan(p: &Project, bytes: &[u8]) -> Result<Plan, String> {
    let book = read(bytes)?;
    let net = Net::new(p);
    // Every datasheet, by tag (upper case), with its rows by comparable label.
    let mut sheets: HashMap<String, Datasheet> = HashMap::new();
    for d in datasheets::all_full(p, &net) {
        sheets.entry(d.tag.to_uppercase()).or_insert(d);
    }
    let labels = |d: &Datasheet| -> Vec<(String, Row)> { d.sections.iter().flat_map(|s| s.rows.iter().map(|r| (norm(&r.label).0, r.clone()))).collect() };
    let mut cx = Ctx { p, us: p.units == "US", plan: Plan { sheets: book.len(), ..Plan::default() }, unknown: BTreeSet::new(), unmatched: BTreeSet::new() };
    let is_tag_header = |s: &str| ["tag", "tag no", "tag no.", "tag number", "instrument tag", "item tag"].contains(&norm(s).0.as_str());
    for (name, grid) in &book {
        if name.eq_ignore_ascii_case("index") {
            continue;
        }
        let mut matched = false;
        // One datasheet: the tag is the sheet's name, a "Tag" row, or a word of the first rows.
        let tag_row = grid.iter().find(|r| r.first().is_some_and(|c| is_tag_header(c))).and_then(|r| r.get(1)).map(|t| t.trim().to_uppercase());
        let words = || grid.iter().take(4).flatten().flat_map(|c| c.split(|ch: char| ch.is_whitespace() || ch == '·' || ch == ',' || ch == ';').map(|w| w.trim().to_uppercase()).collect::<Vec<_>>());
        let named = [Some(name.trim().to_uppercase()), tag_row.clone()].into_iter().flatten().find(|t| sheets.contains_key(t));
        // A table: a header row with a Tag column in the first rows, and other columns or known tags.
        let header = grid.iter().take(10).enumerate().find_map(|(i, r)| r.iter().position(|c| is_tag_header(c)).map(|c| (i, c))).filter(|&(hr, tc)| {
            grid[hr].iter().filter(|c| !c.is_empty()).count() >= 3 || grid[hr + 1..].iter().any(|r| r.get(tc).is_some_and(|t| sheets.contains_key(&t.trim().to_uppercase())))
        });
        let one = named.or_else(|| if header.is_none() { words().find(|w| sheets.contains_key(w)) } else { None });
        let single = one.as_ref().and_then(|t| sheets.get(t));
        if single.is_some() || header.is_none() {
            let Some(d) = single else {
                if let Some(t) = tag_row.filter(|t| !t.is_empty()) {
                    cx.unmatched.insert(t);
                }
                continue;
            };
            matched = true;
            let owned = labels(d);
            let rows: Vec<(String, &Row)> = owned.iter().map(|(l, r)| (l.clone(), r)).collect();
            for r in grid {
                if r.first().is_some_and(|l| l == "Revisions") {
                    break;
                }
                if let (Some(l), Some(v)) = (r.first(), r.get(1)) {
                    cx.value(d, &rows, l, r.get(2).map_or("", |s| s.as_str()), v, name);
                }
            }
        } else if let Some((hr, tc)) = header {
            let head = &grid[hr];
            for row in &grid[hr + 1..] {
                let tag = row.get(tc).map(|t| t.trim()).unwrap_or("");
                if tag.is_empty() {
                    continue;
                }
                let Some(d) = sheets.get(&tag.to_uppercase()) else {
                    cx.unmatched.insert(tag.to_string());
                    continue;
                };
                matched = true;
                let owned = labels(d);
                let rows: Vec<(String, &Row)> = owned.iter().map(|(l, r)| (l.clone(), r)).collect();
                for (c, h) in head.iter().enumerate() {
                    if c != tc {
                        cx.value(d, &rows, h, "", row.get(c).map_or("", |s| s.as_str()), name);
                    }
                }
            }
        }
        if matched {
            cx.plan.used += 1;
        }
    }
    cx.plan.unmatched = cx.unmatched.into_iter().collect();
    cx.plan.unknown = cx.unknown.into_iter().collect();
    // Datasheet sections and title lines are labels with no value: not worth reporting.
    let titles: BTreeSet<String> = sheets.values().flat_map(|d| d.sections.iter().map(|s| s.title.to_lowercase())).collect();
    cx.plan.unknown.retain(|u| !titles.contains(&u.to_lowercase()));
    Ok(cx.plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    fn table(rows: &[&[&str]]) -> Vec<u8> {
        let mut wb = rust_xlsxwriter::Workbook::new();
        let ws = wb.add_worksheet();
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                match v.parse::<f64>() {
                    Ok(n) => ws.write_number(r as u32, c as u16, n).unwrap(),
                    Err(_) => ws.write_string(r as u32, c as u16, *v).unwrap(),
                };
            }
        }
        wb.save_to_buffer().unwrap()
    }

    #[test]
    fn a_table_is_matched_by_tag_and_never_writes_line_values() {
        let p = crate::examples::get("pump").unwrap();
        let book = table(&[
            &["Instrument data from the vendor"],
            &["Tag No.", "Manufacturer", "Set pressure (psig)", "Fail position", "Operating pressure (bar)", "Signal", "Colour", "Range"],
            &["lt-101", "Acme", "", "", "", "4-20 mA HART", "Blue", ""],
            &["PSV-101", "Relief Co", "150", "", "", "", "", ""],
            &["LV-101", "", "", "FC", "", "", "", ""],
            &["LV-102", "", "", "Open", "", "", "", ""],
            &["PI-102", "", "", "", "9", "", "", "0–16 bar"],
            &["XX-999", "Nobody", "", "", "", "", "", ""],
        ]);
        let pl = plan(&p, &book).unwrap();
        let get = |t: &str, l: &str| pl.changes.iter().find(|c| c.tag == t && c.label == l).cloned().unwrap_or_else(|| panic!("{t} {l}: {:#?}", pl.changes));
        let m = get("LT-101", "Manufacturer");
        assert_eq!((m.kind.as_str(), m.val.clone()), ("set", Some(Val::Text("Acme".into()))));
        // A choice matches whatever dash it was typed with.
        let sg = get("LT-101", "Signal");
        assert_eq!((sg.kind.as_str(), sg.new.as_str()), ("same", "4–20 mA HART"));
        // 150 psig is about 10.34 bar(g), kept in pascals.
        let sp = get("PSV-101", "Set pressure");
        assert!(matches!(sp.val, Some(Val::Num(x)) if (x - 150.0 * 6894.757).abs() < 1.0), "{sp:?}");
        assert_eq!(get("LV-101", "Fail position").kind, "same");
        assert_eq!(get("PI-102", "Range").kind, "set");
        // Operating pressure lives on the line: reported, never written.
        let op = get("PI-102", "Operating pressure");
        assert!(op.kind == "conflict" && op.val.is_none() && op.note.contains("P-1003"), "{op:?}");
        assert!(pl.unmatched.contains(&"XX-999".to_string()) && pl.unmatched.contains(&"LV-102".to_string()));
        assert_eq!(pl.unknown, ["Colour"]);
        // Applying the plan: one undoable step that writes only the "set" rows.
        let mut d = Doc::new(p);
        let ops: Vec<Op> = pl.changes.iter().filter(|c| c.kind == "set").map(|c| Op::SetData { ids: vec![c.obj], key: c.key.clone(), val: c.val.clone() }).collect();
        d.apply(Op::Batch { ops }).unwrap();
        let o = |t: &str| d.p.objects.iter().find(|o| o.tag == t).unwrap();
        assert_eq!(o("LT-101").data.get("manufacturer"), Some(&Val::Text("Acme".into())));
        assert!(plan(&d.p, &book).unwrap().changes.iter().all(|c| c.kind != "set"), "imported again, nothing is left to change");
    }

    #[test]
    fn exported_datasheets_read_back() {
        let mut p = crate::examples::get("pump").unwrap();
        let lt = p.objects.iter().find(|o| o.tag == "LT-101").unwrap().id;
        p.obj_mut(lt).unwrap().data.insert("manufacturer".into(), Val::Text("Acme".into()));
        p.units = "US".into();
        let book = datasheets::xlsx(&p).unwrap();
        // Edited elsewhere since: the export's values come back as changes.
        let mut q = p.clone();
        q.obj_mut(lt).unwrap().data.remove("manufacturer");
        q.units = "SI".into();
        let pl = plan(&q, &book).unwrap();
        assert!(pl.changes.iter().any(|c| c.tag == "LT-101" && c.key == "manufacturer" && c.kind == "set"), "{:#?}", pl.changes);
        // The export's own process values (in US units) agree with the drawing: no conflicts.
        assert!(pl.changes.iter().all(|c| c.kind != "conflict" && c.kind != "bad"), "{:#?}", pl.changes.iter().filter(|c| c.kind == "conflict" || c.kind == "bad").collect::<Vec<_>>());
        assert!(pl.unknown.is_empty(), "{:?}", pl.unknown);
        assert_eq!(pl.used, datasheets::all(&p).len());
    }

    #[test]
    fn units_and_numbers_as_people_type_them() {
        assert_eq!(number("4,5 bar"), Some((4.5, "bar".into())));
        assert_eq!(number("-1.5e3"), Some((-1500.0, String::new())));
        assert!(number("abc").is_none());
        assert!((quantity("20", "°C", "temp", false).unwrap() - 293.15).abs() < 1e-9);
        assert!((quantity("68 °F", "", "temp", false).unwrap() - 293.15).abs() < 1e-6);
        assert!((quantity("10", "m³/h", "flow", false).unwrap() - 10.0 / 3600.0).abs() < 1e-12);
        assert!(quantity("10", "kg", "press", false).unwrap_err().contains("same kind"));
        assert!((quantity("1", "", "press", true).unwrap() - 6894.757).abs() < 0.01);
        assert_eq!(norm(" Set pressure (bar) *"), ("set pressure".into(), "bar".into()));
    }

    #[test]
    fn anything_else_is_refused_politely() {
        assert!(plan(&Project::new(), b"not a workbook").unwrap_err().contains("not an Excel workbook"));
        let mut r = 7u64;
        for n in [0usize, 10, 1000, 30_000] {
            let bytes: Vec<u8> = (0..n).map(|_| { r ^= r << 13; r ^= r >> 7; r ^= r << 17; r as u8 }).collect();
            assert!(plan(&Project::new(), &bytes).is_err());
        }
    }
}
