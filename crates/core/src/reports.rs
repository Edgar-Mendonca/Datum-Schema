//! Reports the user designs: a source (any of the lists, or every object of a class with all its
//! attributes), the columns and their order, filters, a sort and a grouping, saved in the project
//! by name. A report is built like a list (`lists::build("report:<name>")`), so it shows in the
//! bottom panel, edits its cells back into the data, prints, and exports to Excel and CSV.
//! Filters compare the text a list shows, numbers in the column's display unit.

use crate::lists::{self, Col, List, Row};
use crate::model::{Obj, Project};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Filter {
    /// The column's key.
    pub col: String,
    pub test: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Report {
    pub name: String,
    /// A list kind ("valves") or "class:<Class>".
    pub source: String,
    /// The columns shown, by key, in order (empty: every column of the source).
    pub cols: Vec<String>,
    /// Every filter must pass.
    pub filters: Vec<Filter>,
    /// The column to group by (empty: no groups), and to sort by within them.
    pub group: String,
    pub sort: String,
    pub desc: bool,
}

pub const TESTS: [&str; 8] = ["is", "is not", "contains", "does not contain", "is empty", "is not empty", "at least", "at most"];

pub fn normalize(v: &mut Vec<Report>) {
    v.truncate(100);
    let mut seen = std::collections::HashSet::new();
    v.retain(|r| !r.name.trim().is_empty() && !r.source.trim().is_empty() && seen.insert(r.name.trim().to_lowercase()));
    for r in v.iter_mut() {
        r.name = crate::clip(r.name.trim(), 60);
        r.source = crate::clip(r.source.trim(), 100);
        r.cols.truncate(100);
        r.cols.iter_mut().for_each(|c| *c = crate::clip(c, 100));
        r.filters.truncate(20);
        for f in &mut r.filters {
            f.col = crate::clip(&f.col, 100);
            f.value = crate::clip(&f.value, 200);
            if !TESTS.contains(&f.test.as_str()) {
                f.test = "is".into();
            }
        }
        r.group = crate::clip(&r.group, 100);
        r.sort = crate::clip(&r.sort, 100);
    }
}

/// What a report can be made from: every list, then every class with objects in the project.
pub fn sources(p: &Project) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = lists::KINDS.iter().map(|(k, l)| (k.to_string(), l.to_string())).collect();
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for o in &p.objects {
        // The class and each parent: "every pump" takes the centrifugal and the reciprocating ones.
        let chain = crate::classes::chain(&o.class);
        if chain.is_empty() {
            *counts.entry(o.class.clone()).or_default() += 1;
        }
        for c in chain.iter().filter(|c| c.name != "PlantItem") {
            *counts.entry(c.name.clone()).or_default() += 1;
        }
    }
    for (c, n) in counts {
        let label = crate::classes::get(&c).map_or(c.clone(), |x| x.label.clone());
        v.push((format!("class:{c}"), format!("Every {} ({n})", label.to_lowercase())));
    }
    v
}

fn is_a(o: &Obj, class: &str) -> bool {
    o.class == class || crate::classes::chain(&o.class).iter().any(|c| c.name == class)
}

/// Every object of a class (and its subclasses): tag, class, sheet, area, then every attribute.
fn by_class(p: &Project, class: &str) -> List {
    let us = p.units == "US";
    let attrs = crate::classes::attrs(Some(p), class);
    let shown = crate::units::shown(us);
    let unit = |q: &str| shown.iter().find(|s| s.q == q).map(|s| s.label.clone()).unwrap_or_default();
    let mut cols = vec![
        Col { key: "tag".into(), label: "Tag".into(), q: "", edit: false, num: false },
        Col { key: "class".into(), label: "Class".into(), q: "", edit: false, num: false },
        Col { key: "sheet".into(), label: "Sheet".into(), q: "", edit: false, num: false },
        Col { key: "area".into(), label: "Area".into(), q: "", edit: false, num: false },
    ];
    for a in attrs.iter().filter(|a| !crate::propagate::hidden(&a.key)) {
        if cols.iter().any(|c| c.key == a.key) {
            continue;
        }
        let q: &'static str = crate::units::QUANTITIES.iter().find(|x| x.0 == a.q).map_or("", |x| x.0);
        let u = unit(q);
        cols.push(Col { key: a.key.clone(), label: if u.is_empty() { a.label.clone() } else { format!("{} ({u})", a.label) }, q, edit: true, num: !q.is_empty() });
    }
    let mut objs: Vec<&Obj> = p.objects.iter().filter(|o| is_a(o, class)).collect();
    objs.sort_by_key(|o| {
        let (a, n, _) = crate::tags::split(&o.tag);
        (a, n, o.tag.clone())
    });
    let rows = objs
        .into_iter()
        .map(|o| {
            let it = p.items.iter().find(|i| i.obj == o.id);
            let cells = cols
                .iter()
                .map(|c| match c.key.as_str() {
                    "tag" => o.tag.clone(),
                    "class" => crate::classes::get(&o.class).map_or(o.class.clone(), |x| x.label.clone()),
                    "sheet" => it.and_then(|i| p.sheet(i.sheet)).map(|s| s.number.clone()).unwrap_or_default(),
                    "area" => p.node_path(o.node),
                    k => match o.data.get(k) {
                        None => String::new(),
                        Some(v) if !c.q.is_empty() => v.num().map(|x| crate::fmt_sig(crate::units::to_display(x, c.q, us), 4)).unwrap_or_else(|| v.text()),
                        Some(v) => v.text(),
                    },
                })
                .collect();
            Row { ids: vec![it.map_or(o.id, |i| i.id)], sheet: it.map_or(0, |i| i.sheet), cells, status: None }
        })
        .collect();
    let label = crate::classes::get(class).map_or(class.to_string(), |x| x.label.clone());
    List { kind: format!("class:{class}"), title: format!("Every {}", label.to_lowercase()), cols, rows, heads: vec![] }
}

/// The source's list, every column.
pub fn source(p: &Project, src: &str) -> Result<List, String> {
    match src.strip_prefix("class:") {
        Some(c) => Ok(by_class(p, c)),
        None if src.starts_with("report:") => Err("A report cannot be made from another report.".into()),
        None => lists::build(p, src),
    }
}

fn number(s: &str) -> Option<f64> {
    s.split_whitespace().next().and_then(|x| x.trim_end_matches('"').replace(',', ".").parse::<f64>().ok()).filter(|x| x.is_finite())
}

fn passes(f: &Filter, cell: &str) -> bool {
    let (c, v) = (cell.trim().to_lowercase(), f.value.trim().to_lowercase());
    match f.test.as_str() {
        "is empty" => c.is_empty(),
        "is not empty" => !c.is_empty(),
        "is" => v.split(',').any(|x| x.trim() == c),
        "is not" => !v.split(',').any(|x| x.trim() == c),
        "contains" => c.contains(&v),
        "does not contain" => !c.contains(&v),
        "at least" => matches!((number(&c), number(&v)), (Some(a), Some(b)) if a >= b),
        "at most" => matches!((number(&c), number(&v)), (Some(a), Some(b)) if a <= b),
        _ => true,
    }
}

/// Text that sorts numbers as numbers ("2" before "10", "3\"" before "10\"").
fn order(a: &str, b: &str) -> std::cmp::Ordering {
    match (number(a.trim_end_matches('"')), number(b.trim_end_matches('"'))) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

/// A report: its source's rows filtered, its columns chosen, sorted and grouped (each group
/// opens with a heading row that counts its rows and adds up its numeric columns).
pub fn run(p: &Project, r: &Report) -> Result<List, String> {
    let base = source(p, &r.source)?;
    let at = |k: &str| base.cols.iter().position(|c| c.key == k);
    for f in &r.filters {
        if at(&f.col).is_none() {
            return Err(format!("“{}” filters on a column its source no longer has.", r.name));
        }
    }
    let mut rows: Vec<&Row> = base.rows.iter().filter(|row| r.filters.iter().all(|f| at(&f.col).is_none_or(|c| passes(f, row.cells.get(c).map_or("", |s| s.as_str()))))).collect();
    if let Some(c) = at(&r.sort) {
        rows.sort_by(|a, b| order(&a.cells[c], &b.cells[c]));
        if r.desc {
            rows.reverse();
        }
    }
    let group = at(&r.group);
    if let Some(g) = group {
        rows.sort_by(|a, b| order(&a.cells[g], &b.cells[g]));
    }
    let pick: Vec<usize> = if r.cols.is_empty() { (0..base.cols.len()).collect() } else { r.cols.iter().filter_map(|k| at(k)).collect() };
    if pick.is_empty() {
        return Err(format!("“{}” has no columns its source still has.", r.name));
    }
    let cols: Vec<Col> = pick.iter().map(|&i| base.cols[i].clone()).collect();
    let mut out: Vec<Row> = vec![];
    let mut heads = vec![];
    let mut i = 0;
    while i < rows.len() {
        let end = match group {
            Some(g) => i + rows[i..].iter().take_while(|x| x.cells[g] == rows[i].cells[g]).count(),
            None => rows.len(),
        };
        if let Some(g) = group {
            let name = if rows[i].cells[g].trim().is_empty() { "(blank)".to_string() } else { rows[i].cells[g].clone() };
            let n = end - i;
            let mut cells: Vec<String> = pick
                .iter()
                .map(|&c| {
                    if !base.cols[c].num {
                        return String::new();
                    }
                    let xs: Vec<f64> = rows[i..end].iter().filter_map(|x| number(&x.cells[c])).collect();
                    if xs.is_empty() { String::new() } else { crate::fmt_sig(xs.iter().sum(), 4) }
                })
                .collect();
            cells[0] = format!("{}: {name} ({n})", base.cols[g].label);
            heads.push(out.len());
            out.push(Row { ids: rows[i..end].iter().flat_map(|x| x.ids.clone()).collect(), sheet: 0, cells, status: None });
        }
        for row in &rows[i..end] {
            out.push(Row { ids: row.ids.clone(), sheet: row.sheet, cells: pick.iter().map(|&c| row.cells.get(c).cloned().unwrap_or_default()).collect(), status: row.status });
        }
        i = end;
    }
    Ok(List { kind: format!("report:{}", r.name), title: r.name.clone(), cols, rows: out, heads })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(source: &str) -> Report {
        Report { name: "Mine".into(), source: source.into(), ..Report::default() }
    }

    #[test]
    fn a_report_filters_sorts_groups_and_exports() {
        let mut p = crate::examples::get("pump").unwrap();
        // Every valve 3" and up, size and tag only, grouped by type, largest first.
        let mut r = rep("valves");
        let base = source(&p, "valves").unwrap();
        let key = |label: &str| base.cols.iter().find(|c| c.label.starts_with(label)).unwrap_or_else(|| panic!("{label}: {:?}", base.cols.iter().map(|c| &c.label).collect::<Vec<_>>())).key.clone();
        r.cols = vec![key("Tag"), key("Size"), key("Type")];
        r.filters = vec![Filter { col: key("Size"), test: "at least".into(), value: "3".into() }];
        r.group = key("Type");
        r.sort = key("Size");
        r.desc = true;
        let l = run(&p, &r).unwrap();
        assert_eq!(l.cols.len(), 3);
        assert!(!l.heads.is_empty() && l.heads[0] == 0 && l.rows[0].cells[0].starts_with("Type: "), "{:?}", l.rows.iter().map(|x| &x.cells).collect::<Vec<_>>());
        let body: Vec<&Row> = l.rows.iter().enumerate().filter(|(i, _)| !l.heads.contains(i)).map(|x| x.1).collect();
        assert!(!body.is_empty() && body.iter().all(|x| number(&x.cells[1]).unwrap() >= 3.0));
        // Group heads count their rows.
        let n: usize = l.heads.iter().map(|&h| l.rows[h].cells[0].rsplit('(').next().unwrap().trim_end_matches(')').parse::<usize>().unwrap()).sum();
        assert_eq!(n, body.len());
        // Saved in the project, it is a list like the others: Excel and CSV too.
        p.settings.reports = vec![r];
        assert_eq!(lists::build(&p, "report:Mine").unwrap().rows.len(), l.rows.len());
        assert_eq!(&lists::xlsx(&p, "report:mine").unwrap()[..2], b"PK");
        assert!(lists::csv(&l).starts_with("Tag,Size"));
        assert!(lists::build(&p, "report:Nobody").is_err());
    }

    #[test]
    fn every_object_of_a_class_with_its_attributes() {
        let p = crate::examples::get("pump").unwrap();
        assert!(sources(&p).iter().any(|s| s.0 == "class:Pump"));
        let mut r = rep("class:Pump");
        r.filters = vec![Filter { col: "flow".into(), test: "is not empty".into(), value: String::new() }];
        let l = run(&p, &r).unwrap();
        assert!(l.rows.iter().any(|x| x.cells[0] == "P-101A") && l.cols.iter().any(|c| c.key == "npshr" && c.edit));
        // Hostile reports never panic: unknown columns, tests and sources.
        let mut v = vec![Report { name: " x ".into(), source: "class:Nothing".into(), cols: vec!["zz".into()], filters: vec![Filter { col: "tag".into(), test: "??".into(), value: "\u{0}".into() }], group: "qq".into(), sort: "tag".into(), desc: true }, rep(""), Report { name: "X".into(), source: "lines".into(), ..Report::default() }];
        normalize(&mut v);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].filters[0].test, "is");
        assert!(run(&p, &v[0]).is_err());
        assert!(run(&p, &rep("report:x")).is_err());
        for s in sources(&p) {
            let mut r = rep(&s.0);
            let l = source(&p, &s.0).unwrap();
            if let Some(c) = l.cols.first() {
                r.group = c.key.clone();
                r.sort = l.cols.last().unwrap().key.clone();
                r.filters = vec![Filter { col: c.key.clone(), test: "at least".into(), value: "1".into() }];
            }
            let _ = run(&p, &r);
        }
    }
}
