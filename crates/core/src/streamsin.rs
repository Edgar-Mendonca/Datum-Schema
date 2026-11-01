//! Stream results from a process simulator, read from Excel or CSV into the process flow
//! diagrams' streams. A simulator's stream table has the streams as columns (their names or
//! numbers in a header row) and a property per row, the unit in the label ("Temperature [C]",
//! "Pressure (bar)") or in the next column; the transposed table (one row per stream) is read
//! too. Rows are matched by what they mean (mass flow, actual volume flow, temperature,
//! pressure, vapour fraction, mass density, enthalpy or heat flow, phase) and mass fractions by
//! the project's components, under a "Mass fractions" heading or in the label ("Mass frac
//! Water"). Absolute pressures (bara, psia, kPa(a)) are kept as gauge, as the drawing shows
//! them. Nothing changes until the plan is applied; blank cells change nothing.

use crate::import::{norm, number, quantity, Change, Plan};
use crate::model::{Project, Val, STREAM_CLASS};
use crate::units;
use std::collections::{BTreeSet, HashMap};

const ATM: f64 = 101_325.0;
const MAX_ROWS: usize = 5000;
const MAX_COLS: usize = 500;

/// The stream properties read: (data key, quantity, the stream table's label).
const PROPS: [(&str, &str, &str); 8] = [("mflow", "mflow", "Mass flow"), ("flow", "flow", "Volume flow"), ("temp", "temp", "Temperature"), ("press", "press", "Pressure"), ("vfrac", "", "Vapour fraction"), ("density", "density", "Density"), ("hflow", "power", "Enthalpy flow"), ("phase", "", "Phase")];

/// What a row's label means: a property key, or None.
fn prop(l: &str) -> Option<&'static str> {
    let has = |w: &str| l.contains(w);
    let first = l.split_whitespace().next().unwrap_or("");
    if has("frac") && !has("vap") || has("std") || has("standard") || has("ideal") || has("molar") || has("mole") || has("cp") || has("cv") {
        return None;
    }
    if has("mass flow") || l == "mass rate" || l == "total mass flow" || l == "mass flowrate" {
        return Some("mflow");
    }
    if has("volume flow") || has("vol flow") || has("volumetric flow") || has("vol. flow") {
        return Some("flow");
    }
    if first == "temperature" || l == "temp" || l == "t" {
        return Some("temp");
    }
    if first == "pressure" || l == "press" || l == "p" {
        return Some("press");
    }
    if has("vapour frac") || has("vapor frac") || l == "vf" || l == "vapour fraction" || l == "quality" {
        return Some("vfrac");
    }
    if has("density") {
        return Some("density");
    }
    if has("enthalpy flow") || has("heat flow") || l == "enthalpy rate" {
        return Some("hflow");
    }
    if l == "phase" || l == "phase state" {
        return Some("phase");
    }
    None
}

/// A unit that marks an absolute pressure, and the unit without the mark.
fn absolute(u: &str) -> Option<String> {
    let t = u.trim();
    for s in ["(a)", " abs", "(abs)", "a"] {
        if let Some(base) = t.strip_suffix(s) {
            let base = base.trim();
            if ["bar", "psi", "kPa", "MPa", "Pa", "kg/cm2", "kg/cm^2", "atm"].iter().any(|x| x.eq_ignore_ascii_case(base)) {
                return Some(base.to_string());
            }
        }
    }
    None
}

/// Exponents typed as digits (m3/h, kg/m3, ft2) to the unit table's form (m^3/h).
fn powers(u: &str) -> String {
    match u.trim() {
        "C" | "deg C" | "degC" | "oC" => return "°C".into(),
        "F" | "deg F" | "degF" | "oF" => return "°F".into(),
        _ => {}
    }
    let cs: Vec<char> = u.chars().collect();
    let mut out = String::new();
    for (i, &c) in cs.iter().enumerate() {
        if (c == '2' || c == '3') && i > 0 && cs[i - 1].is_ascii_alphabetic() && !cs.get(i + 1).is_some_and(|x| x.is_ascii_digit()) {
            out.push('^');
        }
        out.push(c);
    }
    out
}

/// A CSV text as a grid: comma, semicolon or tab separated, quotes as usual.
fn csv(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let head = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let sep = [('\t', head.matches('\t').count()), (';', head.matches(';').count()), (',', head.matches(',').count())].into_iter().max_by_key(|x| x.1).map_or(',', |x| x.0);
    let mut rows = vec![];
    let (mut row, mut cur, mut q) = (vec![], String::new(), false);
    let mut cs = text.chars().peekable();
    while let Some(c) = cs.next() {
        match c {
            '"' if q && cs.peek() == Some(&'"') => {
                cur.push('"');
                cs.next();
            }
            '"' => q = !q,
            c if c == sep && !q => row.push(crate::clip(std::mem::take(&mut cur).trim(), 1000)),
            '\r' if !q => {}
            '\n' if !q => {
                row.push(crate::clip(std::mem::take(&mut cur).trim(), 1000));
                rows.push(std::mem::take(&mut row));
                if rows.len() >= MAX_ROWS {
                    return rows;
                }
            }
            c => {
                if cur.len() < 4000 {
                    cur.push(c)
                }
            }
        }
        if row.len() > MAX_COLS {
            row.truncate(MAX_COLS);
        }
    }
    if !cur.is_empty() || !row.is_empty() {
        row.push(crate::clip(cur.trim(), 1000));
        rows.push(row);
    }
    rows
}

/// The workbook's or CSV file's grids.
fn grids(bytes: &[u8]) -> Result<Vec<crate::import::Grid>, String> {
    if bytes.starts_with(b"PK") {
        return crate::import::read(bytes);
    }
    if bytes.len() > crate::doc::MAX_FILE as usize {
        return Err("That file is too large to read.".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "This is not an Excel workbook (.xlsx) or a CSV text file.".to_string())?;
    if text.chars().any(|c| c == '\0') {
        return Err("This is not an Excel workbook (.xlsx) or a CSV text file.".into());
    }
    Ok(vec![("CSV".into(), csv(text))])
}

fn transpose(g: &[Vec<String>]) -> Vec<Vec<String>> {
    let w = g.iter().map(|r| r.len()).max().unwrap_or(0).min(MAX_COLS);
    (0..w).map(|c| g.iter().take(MAX_COLS).map(|r| r.get(c).cloned().unwrap_or_default()).collect()).collect()
}

/// What importing a simulator's stream results would change.
pub fn plan(p: &Project, bytes: &[u8]) -> Result<Plan, String> {
    let book = grids(bytes)?;
    let us = p.units == "US";
    // The project's streams, by number and by name (upper case).
    let mut streams: HashMap<String, u32> = HashMap::new();
    for o in p.objects.iter().filter(|o| o.class == STREAM_CLASS) {
        for k in ["number", "name"] {
            if let Some(v) = o.data.get(k).map(|v| v.text().trim().to_uppercase()).filter(|v| !v.is_empty()) {
                streams.entry(v).or_insert(o.id);
            }
        }
        if !o.tag.trim().is_empty() {
            streams.entry(o.tag.trim().to_uppercase()).or_insert(o.id);
        }
    }
    if streams.is_empty() {
        return Err("This project has no streams: draw a process flow diagram and number its streams first.".into());
    }
    let comps: Vec<(String, String)> = p.settings.components.iter().map(|c| (c.to_lowercase(), c.clone())).collect();
    let mut plan = Plan { sheets: book.len(), ..Plan::default() };
    let (mut unknown, mut unmatched) = (BTreeSet::new(), BTreeSet::new());
    let mut seen: HashMap<(u32, String), usize> = HashMap::new();
    for (name, grid) in &book {
        // Streams in columns (a header row of stream names) or in rows. Stream numbers are often
        // small numbers that values repeat, so a header is scored by the distinct streams it names
        // plus the property labels (mass flow, temperature…) in the first column below it.
        let known = |l: &str| {
            let l = norm(&l.replace('[', "(").replace(']', ")")).0;
            prop(&l).is_some() || comps.iter().any(|(c, _)| *c == l) || l.contains("frac")
        };
        let best = |g: &[Vec<String>]| -> Option<(usize, usize)> {
            g.iter().take(40).enumerate().map(|(i, r)| {
                let named = r.iter().skip(1).filter_map(|c| streams.get(&c.trim().to_uppercase())).collect::<BTreeSet<_>>().len();
                let labels = g.iter().skip(i + 1).filter(|r| r.first().is_some_and(|c| known(c))).count();
                (i, if named > 0 { named + labels } else { 0 })
            }).filter(|x| x.1 > 0).max_by_key(|x| (x.1, usize::MAX - x.0))
        };
        let t = transpose(grid);
        let (g, hr) = match (best(grid), best(&t)) {
            (Some(a), Some(b)) if b.1 > a.1 => (t, b.0),
            (Some(a), _) => (grid.clone(), a.0),
            (None, Some(b)) => (t, b.0),
            (None, None) => continue,
        };
        plan.used += 1;
        let head = &g[hr];
        // Columns holding streams (known or not: a name in the header row right of the labels).
        let cols: Vec<(usize, Option<u32>, String)> = head.iter().enumerate().skip(1).filter(|(_, c)| !c.trim().is_empty()).map(|(i, c)| (i, streams.get(&c.trim().to_uppercase()).copied(), c.trim().to_string())).collect();
        let first_col = cols.iter().find(|c| c.1.is_some()).map_or(1, |c| c.0);
        let cols: Vec<_> = cols.into_iter().filter(|c| c.0 >= first_col).collect();
        for c in cols.iter().filter(|c| c.1.is_none()) {
            unmatched.insert(c.2.clone());
        }
        // A unit column between the labels and the first stream.
        let unit_col = (1..first_col).find(|&c| g.iter().skip(hr + 1).filter_map(|r| r.get(c)).any(|u| !u.trim().is_empty() && number(u).is_none()));
        let mut fractions = ""; // "mass", "mole" or "" under a heading
        for row in g.iter().skip(hr + 1) {
            let label = row.first().map(|s| s.trim()).unwrap_or("");
            if label.is_empty() {
                continue;
            }
            let has_values = cols.iter().any(|c| row.get(c.0).is_some_and(|v| !v.trim().is_empty()));
            let (l, unit) = norm(&label.replace('[', "(").replace(']', ")"));
            let unit = unit_col.and_then(|c| row.get(c)).map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).unwrap_or(unit);
            let unit = powers(&unit);
            // A heading: what the rows under it are.
            if !has_values {
                fractions = if l.contains("mass frac") || l.contains("mass composition") || l.contains("weight frac") { "mass" } else if l.contains("frac") || l.contains("composition") { "mole" } else { "" };
                continue;
            }
            // A component's mass fraction: under a heading, or named in the label.
            let comp = comps.iter().find(|(lc, _)| l == *lc || (l.contains("frac") && (l.ends_with(&format!(" {lc}")) || l.starts_with(&format!("{lc} ")))));
            let in_label = if l.contains("mass frac") || l.contains("weight frac") { "mass" } else if l.contains("frac") && !l.contains("vap") { "mole" } else { "" };
            let mode = if in_label.is_empty() { fractions } else { in_label };
            let (key, q, what): (String, &str, String) = if let (Some((_, c)), "mass") = (comp, mode) {
                (format!("x:{c}"), "", format!("Mass fraction {c}"))
            } else if let Some(k) = (mode.is_empty() || comp.is_none()).then(|| prop(&l)).flatten() {
                let (_, q, lab) = PROPS.iter().find(|x| x.0 == k).copied().unwrap_or(("", "", ""));
                (k.to_string(), q, lab.to_string())
            } else {
                let why = if mode == "mole" { " (mole fractions: give mass fractions)" } else if mode == "mass" { " (not a component of this project: add it in Options › Components)" } else { "" };
                unknown.insert(format!("{}{why}", crate::clip(label, 80)));
                continue;
            };
            let abs = (q == "press").then(|| absolute(&unit)).flatten();
            for &(col, obj, ref sname) in &cols {
                let Some(obj) = obj else { continue };
                let text = row.get(col).map(|s| s.trim()).unwrap_or("");
                if text.is_empty() {
                    continue;
                }
                let o = p.obj(obj);
                let cur = o.and_then(|o| o.data.get(&key));
                let old = cur.map(|v| if q.is_empty() { v.text() } else { v.num().map_or(v.text(), |x| units::show(x, q, us)) }).unwrap_or_default();
                let mut c = Change { obj, tag: sname.clone(), key: key.clone(), label: what.clone(), old, new: text.to_string(), val: None, kind: String::new(), note: String::new(), sheet: name.clone() };
                let val = if key == "phase" {
                    Ok(Val::Text(crate::clip(text, 40)))
                } else if q.is_empty() {
                    match number(text) {
                        Some((x, _)) if (0.0..=1.0).contains(&x) => Ok(Val::Num(x)),
                        Some((x, u)) if u == "%" && (0.0..=100.0).contains(&x) => Ok(Val::Num(x / 100.0)),
                        _ => Err(format!("“{text}” is not a fraction between 0 and 1.")),
                    }
                } else {
                    let r = match &abs {
                        Some(base) => quantity(text, base, q, us).map(|x| x - ATM),
                        None => quantity(text, &unit, q, us),
                    };
                    r.map(Val::Num)
                };
                match val {
                    Ok(v) => {
                        if let Val::Num(x) = v {
                            c.new = if q.is_empty() { crate::fmt_sig(x, 4) } else { units::show(x, q, us) };
                        }
                        if abs.is_some() {
                            c.note = "Absolute in the file; kept as gauge.".into();
                        }
                        let same = match (cur, &v) {
                            (Some(Val::Num(a)), Val::Num(b)) => (a - b).abs() <= 1e-6 * b.abs().max(1e-9),
                            (Some(a), b) => a.text() == b.text(),
                            _ => false,
                        };
                        c.kind = if same { "same" } else { "set" }.into();
                        c.val = (!same).then_some(v);
                    }
                    Err(e) => (c.kind, c.note) = ("bad".into(), e),
                }
                // The same value twice (two rows read as mass flow): the first wins, the rest are reported.
                let n = seen.entry((obj, key.clone())).or_insert(0);
                *n += 1;
                if *n > 1 {
                    (c.kind, c.note, c.val) = ("bad".into(), format!("{what} was already read from an earlier row."), None);
                }
                plan.changes.push(c);
            }
        }
    }
    if plan.used == 0 {
        return Err("No stream of this project is named in the file: its header row (or first column) should hold the stream numbers or names as on the drawing.".into());
    }
    plan.unmatched = unmatched.into_iter().collect();
    plan.unknown = unknown.into_iter().collect();
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    fn project() -> Project {
        let mut p = crate::examples::get("pfd").unwrap();
        if !p.settings.components.iter().any(|c| c == "Water") {
            p.settings.components = vec!["Water".into(), "Methanol".into()];
        }
        p
    }

    fn numbers(p: &Project) -> Vec<String> {
        let mut v: Vec<String> = p.objects.iter().filter(|o| o.class == STREAM_CLASS).filter_map(|o| o.data.get("number").map(|v| v.text())).collect();
        v.sort();
        v
    }

    #[test]
    fn a_simulator_table_is_read_into_the_streams() {
        let p = project();
        let n = numbers(&p);
        assert!(n.len() >= 2, "{n:?}");
        let (a, b) = (&n[0], &n[1]);
        let text = format!(
            "Stream results\nName,Units,{a},{b},S-99\nTemperature,C,25,80.5,10\nPressure,bara,5.01325,2,1\nMass Flow,kg/h,1000,2000,5\nStd Ideal Liq Vol Flow,m3/h,1,2,3\nVapour Fraction,,0,0.25,0\nMass Density,kg/m3,998,abc,1\nMass Fractions\nWater,,0.9,0.5,1\nMethanol,,0.1,0.5,0\nNitrogen,,0,0,0\nMole Fractions\nWater,,0.8,0.4,1\nColour,,blue,red,green\n"
        );
        let pl = plan(&p, text.as_bytes()).unwrap();
        let get = |s: &str, k: &str| pl.changes.iter().find(|c| c.tag == *s && c.key == k).cloned().unwrap_or_else(|| panic!("{s} {k}: {:#?}", pl.changes));
        let t = get(a, "temp");
        assert!(t.kind != "bad" && t.new == "25 °C", "{t:?}");
        // 5.01325 bar absolute is 4 bar gauge, as the drawing shows pressures.
        let pr = get(a, "press");
        assert!(matches!(pr.val, Some(Val::Num(x)) if (x - 4e5).abs() < 1e-6) && pr.note.contains("gauge"), "{pr:?}");
        assert!(matches!(get(b, "mflow").val, Some(Val::Num(x)) if (x - 2000.0 / 3600.0).abs() < 1e-12));
        assert!(matches!(get(b, "vfrac").val, Some(Val::Num(x)) if x == 0.25));
        assert_eq!(get(b, "density").kind, "bad");
        assert!(matches!(get(a, "x:Water").val, Some(Val::Num(x)) if x == 0.9));
        // Standard volume flow, mole fractions and unknown rows are not read; nor is S-99.
        assert!(!pl.changes.iter().any(|c| c.key == "flow"));
        assert_eq!(pl.changes.iter().filter(|c| c.key == "x:Water" && c.tag == *a).count(), 1);
        assert!(pl.unknown.iter().any(|u| u.starts_with("Nitrogen")) && pl.unknown.iter().any(|u| u.starts_with("Colour")) && pl.unknown.iter().any(|u| u.contains("mole fractions")), "{:?}", pl.unknown);
        assert_eq!(pl.unmatched, ["S-99"]);
        // Applied: one undoable step; read again, nothing is left to change.
        let mut d = Doc::new(p);
        let ops: Vec<Op> = pl.changes.iter().filter(|c| c.kind == "set").map(|c| Op::SetData { ids: vec![c.obj], key: c.key.clone(), val: c.val.clone() }).collect();
        d.apply(Op::Batch { ops }).unwrap();
        assert!(plan(&d.p, text.as_bytes()).unwrap().changes.iter().all(|c| c.kind != "set"));
    }

    #[test]
    fn streams_in_rows_and_excel_are_read_too() {
        let p = project();
        let n = numbers(&p);
        let mut wb = rust_xlsxwriter::Workbook::new();
        let ws = wb.add_worksheet();
        let rows: Vec<Vec<String>> = vec![vec!["Stream".into(), "Temperature [°F]".into(), "Pressure (psig)".into(), "Mass flow (lb/h)".into(), "Phase".into()], vec![n[0].clone(), "212".into(), "14.5".into(), "1000".into(), "Liquid".into()]];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                ws.write_string(r as u32, c as u16, v).unwrap();
            }
        }
        let pl = plan(&p, &wb.save_to_buffer().unwrap()).unwrap();
        let get = |k: &str| pl.changes.iter().find(|c| c.key == k).cloned().unwrap();
        assert!(matches!(get("temp").val, Some(Val::Num(x)) if (x - 373.15).abs() < 1e-6));
        assert!(matches!(get("press").val, Some(Val::Num(x)) if (x - 14.5 * 6894.757).abs() < 1.0));
        assert!(matches!(get("mflow").val, Some(Val::Num(x)) if (x - 1000.0 * 0.45359237 / 3600.0).abs() < 1e-9));
        assert_eq!(get("phase").new, "Liquid");
    }

    #[test]
    fn anything_else_is_refused_politely() {
        let p = project();
        assert!(plan(&p, b"a,b,c\nx,y,z\n").unwrap_err().contains("No stream"));
        assert!(plan(&Project::new(), b"x").unwrap_err().contains("no streams"));
        let mut r = 11u64;
        for n in [0usize, 10, 1000, 30_000] {
            let bytes: Vec<u8> = (0..n).map(|_| { r ^= r << 13; r ^= r >> 7; r ^= r << 17; r as u8 }).collect();
            let _ = plan(&p, &bytes);
            let text: String = bytes.iter().map(|b| [',', '\n', '"', ';', '\t', 'a', '1', '.'][(*b % 8) as usize]).collect();
            let _ = plan(&p, text.as_bytes());
        }
    }
}
