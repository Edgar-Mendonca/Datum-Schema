//! The electrical load list and the data completeness of each class.
//!
//! The load list has every piece of equipment that draws electrical power: pumps, compressors,
//! fans, motors, agitators, electric heaters and air coolers. Each has its rated power (the driver's
//! rating, else its power), its absorbed power (its power, else 80 % of the rating), its voltage and
//! its operation (continuous, intermittent or standby); its load is the absorbed power times 1, 0.5
//! or 0. Equipment driven by a steam turbine or an engine is listed with no load. A total row ends
//! the list. A sizing aid for the electrical engineer, checked by them.
//!
//! Data completeness says, for each class with objects, how much of the data its own section asks
//! for is filled in (the required data for line numbers), and which fields are most often empty.

use crate::lists::{Col, List, Row};
use crate::model::{Obj, Project};

/// Classes that draw electrical power (with their subclasses).
const CONSUMERS: [&str; 6] = ["Pump", "Compressor", "Driver", "Mover", "ElectricHeater", "AirCoolingSystem"];

fn is_a(o: &Obj, class: &str) -> bool {
    o.class == class || crate::classes::chain(&o.class).iter().any(|c| c.name == class)
}

fn num(o: &Obj, k: &str) -> Option<f64> {
    o.data.get(k).and_then(|v| v.num()).filter(|x| x.is_finite() && *x >= 0.0)
}

fn text(o: &Obj, k: &str) -> String {
    o.data.get(k).map(|v| v.text().trim().to_string()).unwrap_or_default()
}

/// One consumer: (object, rated W, absorbed W, load W, electric).
pub type Consumer<'a> = (&'a Obj, Option<f64>, Option<f64>, f64, bool);

pub fn consumers(p: &Project) -> Vec<Consumer<'_>> {
    let mut v: Vec<&Obj> = p.objects.iter().filter(|o| !o.tag.trim().is_empty() && CONSUMERS.iter().any(|c| is_a(o, c))).collect();
    v.sort_by_key(|o| {
        let (a, n, _) = crate::tags::split(&o.tag);
        (a, n, o.tag.clone())
    });
    v.into_iter()
        .map(|o| {
            let driver = text(o, "driver").to_lowercase();
            let electric = driver.is_empty() || driver.contains("motor") || driver.contains("electric") || is_a(o, "ElectricHeater") || is_a(o, "ElectricMotor");
            let duty = if is_a(o, "ElectricHeater") { num(o, "duty") } else { None };
            let rated = num(o, "motor_power").or(num(o, "power")).or(duty);
            let absorbed = num(o, "power").or(duty).or(rated.map(|r| r * 0.8));
            let factor = match text(o, "operation").to_lowercase().as_str() {
                "standby" => 0.0,
                "intermittent" => 0.5,
                _ => 1.0,
            };
            let load = if electric { absorbed.unwrap_or(0.0) * factor } else { 0.0 };
            (o, rated, absorbed, load, electric)
        })
        .collect()
}

fn col(key: &str, label: &str, q: &'static str, edit: bool, us: bool) -> Col {
    let u = if q.is_empty() { String::new() } else { crate::units::shown(us).into_iter().find(|s| s.q == q).map(|s| s.label).unwrap_or_default() };
    Col { key: key.into(), label: if u.is_empty() { label.into() } else { format!("{label} ({u})") }, q, edit, num: !q.is_empty() }
}

fn kw(x: Option<f64>, us: bool) -> String {
    x.map_or(String::new(), |v| crate::fmt_sig(crate::units::to_display(v, "power", us), 4))
}

/// The electrical load list.
pub fn load_list(p: &Project) -> List {
    let us = p.units == "US";
    let cols = vec![
        col("tag", "Tag", "", true, us),
        col("service", "Service", "", true, us),
        col("class", "Type", "", false, us),
        col("driver", "Driver", "", true, us),
        col("motor_power", "Rated", "power", true, us),
        col("power", "Absorbed", "power", true, us),
        col("operation", "Operation", "", true, us),
        col("voltage", "Voltage", "", true, us),
        col("load", "Load", "power", false, us),
        col("area", "Area", "", false, us),
        col("sheet", "Sheet", "", false, us),
    ];
    let mut rows: Vec<Row> = vec![];
    let mut total = 0.0;
    for (o, rated, absorbed, load, electric) in consumers(p) {
        total += load;
        let it = p.items.iter().find(|i| i.obj == o.id);
        let cells = cols
            .iter()
            .map(|c| match c.key.as_str() {
                "tag" => o.tag.clone(),
                "class" => crate::classes::get(&o.class).map_or(o.class.clone(), |x| x.label.clone()),
                "driver" => {
                    let d = text(o, "driver");
                    if d.is_empty() { "Electric motor".into() } else { d }
                }
                "motor_power" => kw(rated, us),
                "power" => kw(absorbed, us),
                "operation" => {
                    let d = text(o, "operation");
                    if d.is_empty() { "Continuous".into() } else { d }
                }
                "load" => if electric { kw(Some(load), us) } else { "none (not electric)".into() },
                "area" => p.node_path(o.node),
                "sheet" => it.and_then(|i| p.sheet(i.sheet)).map(|s| s.number.clone()).unwrap_or_default(),
                k => text(o, k),
            })
            .collect();
        rows.push(Row { ids: vec![it.map_or(o.id, |i| i.id)], sheet: it.map_or(0, |i| i.sheet), cells, status: (electric && rated.is_none()).then_some(1) });
    }
    let mut heads = vec![];
    if !rows.is_empty() {
        let mut cells = vec![String::new(); cols.len()];
        cells[0] = format!("Total ({} consumers)", rows.len());
        cells[8] = kw(Some(total), us);
        heads.push(rows.len());
        rows.push(Row { ids: vec![], sheet: 0, cells, status: None });
    }
    List { kind: "loads".into(), title: "Electrical load list".into(), cols, rows, heads }
}

/// The data a class's objects are asked for: the required data, else its own section's.
fn asked(p: &Project, class: &str) -> Vec<crate::classes::Attr> {
    let a = crate::classes::attrs(Some(p), class);
    let req: Vec<_> = a.iter().filter(|x| x.required).cloned().collect();
    if !req.is_empty() {
        return req;
    }
    a.into_iter().filter(|x| x.group.is_empty() && !crate::propagate::hidden(&x.key) && !matches!(x.key.as_str(), "nozzles" | "relief" | "number" | "stream" | "limits" | "fittings" | "safety_na")).collect()
}

/// Data completeness by class.
pub fn completeness(p: &Project) -> List {
    let us = p.units == "US";
    let cols = vec![col("class", "Class", "", false, us), col("n", "Objects", "", false, us), col("filled", "Filled (%)", "", false, us), col("complete", "Complete objects", "", false, us), col("missing", "Most often empty", "", false, us)];
    let mut classes: Vec<&str> = p.objects.iter().filter(|o| o.class != crate::model::STREAM_CLASS).map(|o| o.class.as_str()).collect();
    classes.sort_unstable();
    classes.dedup();
    let mut rows = vec![];
    for c in classes {
        // Line numbers that are only an instrument's connection to the process are not lines.
        let real = |o: &Obj| o.class != crate::model::LINE_CLASS || p.lines.iter().any(|l| l.obj == o.id && !crate::graph::instrument_connection(p, l));
        let objs: Vec<&Obj> = p.objects.iter().filter(|o| o.class == c && real(o)).collect();
        let ask = asked(p, c);
        if ask.is_empty() || objs.is_empty() {
            continue;
        }
        let filled = |o: &Obj, k: &str| o.data.get(k).is_some_and(|v| !v.text().trim().is_empty());
        let total = objs.len() * ask.len();
        let have: usize = objs.iter().map(|o| ask.iter().filter(|a| filled(o, &a.key)).count()).sum();
        let complete = objs.iter().filter(|o| ask.iter().all(|a| filled(o, &a.key))).count();
        let mut empty: Vec<(usize, &str)> = ask.iter().map(|a| (objs.iter().filter(|o| !filled(o, &a.key)).count(), a.label.as_str())).filter(|x| x.0 > 0).collect();
        empty.sort_by_key(|x| std::cmp::Reverse(x.0));
        let pct = 100.0 * have as f64 / total as f64;
        let cells = vec![
            crate::classes::get(c).map_or(c.to_string(), |x| x.label.clone()),
            objs.len().to_string(),
            format!("{:.0}", pct.floor()),
            format!("{complete} of {}", objs.len()),
            empty.iter().take(3).map(|(n, l)| format!("{l} ({n})")).collect::<Vec<_>>().join(", "),
        ];
        let first = p.items.iter().find(|i| objs.iter().any(|o| o.id == i.obj));
        rows.push(Row { ids: first.map_or(vec![], |i| vec![i.id]), sheet: first.map_or(0, |i| i.sheet), cells, status: Some(if pct >= 99.5 { 0 } else if pct >= 50.0 { 1 } else { 2 }) });
    }
    List { kind: "completeness".into(), title: "Data completeness".into(), cols, rows, heads: vec![] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::Val;

    #[test]
    fn the_load_list_adds_up_and_standby_draws_nothing() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let a = d.p.objects.iter().find(|o| o.tag == "P-101A").unwrap().id;
        let b = d.p.objects.iter().find(|o| o.tag == "P-101B").unwrap().id;
        d.apply(Op::SetData { ids: vec![a, b], key: "power".into(), val: Some(Val::Num(15e3)) }).unwrap();
        d.apply(Op::SetData { ids: vec![a, b], key: "motor_power".into(), val: Some(Val::Num(18.5e3)) }).unwrap();
        d.apply(Op::SetData { ids: vec![b], key: "operation".into(), val: Some(Val::Text("Standby".into())) }).unwrap();
        let l = load_list(&d.p);
        let at = |k: &str| l.cols.iter().position(|c| c.key == k).unwrap();
        let row = |t: &str| l.rows.iter().find(|r| r.cells[0] == t).unwrap();
        assert_eq!(row("P-101A").cells[at("load")], "15");
        assert_eq!(row("P-101B").cells[at("load")], "0");
        assert_eq!(row("P-101A").cells[at("motor_power")], "18.5");
        let total = l.rows.last().unwrap();
        assert!(total.cells[0].starts_with("Total") && l.heads == vec![l.rows.len() - 1]);
        assert_eq!(total.cells[at("load")], "15");
        // A turbine-driven pump draws nothing.
        d.apply(Op::SetData { ids: vec![a], key: "driver".into(), val: Some(Val::Text("Steam turbine".into())) }).unwrap();
        let l = load_list(&d.p);
        assert!(l.rows.iter().find(|r| r.cells[0] == "P-101A").unwrap().cells[at("load")].starts_with("none"));
        // It is a list like the others, and reads back from Excel.
        assert_eq!(crate::lists::build(&d.p, "loads").unwrap().rows.len(), l.rows.len());
        assert_eq!(&crate::lists::xlsx(&d.p, "loads").unwrap()[..2], b"PK");
    }

    #[test]
    fn completeness_counts_what_is_filled() {
        let p = crate::examples::get("pump").unwrap();
        let l = completeness(&p);
        let lines = l.rows.iter().find(|r| r.cells[0] == "Line number" || r.cells[0].contains("ine")).expect("line numbers are listed");
        assert_eq!(lines.cells[2], "100", "every example line has its size, fluid, sequence and class: {:?}", lines.cells);
        assert!(l.rows.iter().any(|r| r.cells[2] != "100" && !r.cells[4].is_empty()));
        assert_eq!(crate::lists::build(&p, "completeness").unwrap().rows.len(), l.rows.len());
    }
}
