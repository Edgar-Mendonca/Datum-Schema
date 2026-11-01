//! The four lists, always generated from the drawing: the line list (one row per line number,
//! with from/to and hydraulics), the equipment list, the instrument index and the valve list.
//! Rows are text in the project's display units; columns say which data field they edit and in
//! which quantity, so the window can send edits back. CSV and Excel export.

use crate::graph::Net;
use crate::hydraulics::{self, Hyd};
use crate::model::{End, Item, Obj, Project};
use crate::symbols::{self, Cat};
use crate::units;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Col {
    pub key: String,
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
    /// A report's group heading rows (indices into `rows`).
    pub heads: Vec<usize>,
}

pub const KINDS: [(&str, &str); 12] = [("lines", "Line list"), ("streams", "Stream list"), ("io", "I/O summary"), ("takeoff", "Valve and fitting take-off"), ("comments", "Review comments"), ("jbs", "Junction boxes"), ("cables", "Cable schedule"), ("ioalloc", "I/O allocation"), ("equipment", "Equipment list"), ("instruments", "Instrument index"), ("valves", "Valve list"), ("datasheets", "Datasheet index")];

/// (key, label, quantity, editable)
type Spec = (&'static str, &'static str, &'static str, bool);

fn cols(specs: &[Spec], us: bool) -> Vec<Col> {
    specs
        .iter()
        .map(|&(key, label, q, edit)| {
            let u = if q.is_empty() { String::new() } else { units::shown(us).into_iter().find(|s| s.q == q).map(|s| s.label).unwrap_or_default() };
            Col { key: key.into(), label: if u.is_empty() { label.into() } else { format!("{label} ({u})") }, q, edit, num: !q.is_empty() }
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
    if let Some(name) = kind.strip_prefix("report:") {
        let r = p.settings.reports.iter().find(|r| r.name.eq_ignore_ascii_case(name.trim())).ok_or_else(|| format!("There is no report called {name}."))?;
        return crate::reports::run(p, r);
    }
    let net = Net::new(p);
    let us = p.units == "US";
    let title = KINDS.iter().find(|k| k.0 == kind).ok_or_else(|| format!("There is no list called {kind}."))?.1.to_string();
    // One row per object, at its first symbol.
    let items = |f: &dyn Fn(Cat) -> bool| -> Vec<(&Obj, &Item)> {
        let mut seen = std::collections::HashSet::new();
        let mut v: Vec<(&Obj, &Item)> = p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| f(s.cat)) && seen.insert(i.obj)).filter_map(|i| p.obj(i.obj).map(|o| (o, i))).collect();
        let key = |o: &Obj| { let (a, n, _) = crate::tags::split(&o.tag); (a, n, o.tag.clone()) };
        v.sort_by_key(|x| key(x.0));
        v
    };
    let sym_name = |it: &Item| symbols::get(&it.sym).map_or(String::new(), |s| s.name.to_string());
    let (cols_, rows): (Vec<Col>, Vec<Row>) = match kind {
        "lines" => {
            let specs: Vec<Spec> = vec![
                ("number", "Line number", "", false), ("size", "Size", "", true), ("sched", "Sched", "", true), ("fluid", "Fluid", "", true), ("seq", "Seq", "", true), ("class", "Class", "", true),
                ("service", "Service", "", true), ("from", "From", "", false), ("to", "To", "", false), ("stream", "PFD stream", "", true),
                ("flow", "Flow", "flow", true), ("v", "Velocity", "velocity", false), ("dp100", "ΔP", "dp100", false), ("dp", "ΔP total", "dp", false), ("status", "Hydraulics", "", false),
                ("design_p", "Design P", "press", true), ("design_t", "Design T", "temp", true), ("oper_p", "Oper P", "press", true), ("oper_t", "Oper T", "temp", true),
                ("density", "Density", "density", true), ("viscosity", "Viscosity", "visc", true), ("length", "Length", "length", true),
                ("ins_type", "Insulation", "", true), ("ins_thk", "Ins. thk", "thk", true), ("tracing", "Tracing", "", true), ("test_p", "Test P", "press", true), ("area", "Area", "", false), ("sheet", "Sheet", "", false),
            ];
            let hyd: Vec<Option<Hyd>> = hydraulics::all(p, &net);
            let mut rows: Vec<Row> = net
                .runs
                .iter()
                .enumerate()
                .filter(|(r, _)| p.obj(net.run_obj[*r]).is_none_or(|o| o.class != crate::model::STREAM_CLASS))
                .map(|(r, lines)| {
                    let l = net.lines[&lines[0]];
                    let data = p.data_of(net.run_obj[r]);
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
                            "area" => p.obj(net.run_obj[r]).map(|o| p.node_path(o.node)).unwrap_or_default(),
                            _ => val(data, k, q, us),
                        })
                        .collect();
                    Row { ids: lines.clone(), sheet: l.sheet, cells, status: h.map(|h| h.status) }
                })
                .collect();
            rows.sort_by(|a, b| a.cells[0].cmp(&b.cells[0]));
            (cols(&specs, us), rows)
        }
        "streams" => {
            let specs: Vec<Spec> = vec![
                ("number", "Stream", "", true), ("name", "Description", "", true), ("phase", "Phase", "", true), ("from", "From", "", false), ("to", "To", "", false),
                ("mflow", "Mass flow", "mflow", true), ("flow", "Volume flow", "flow", true), ("temp", "Temperature", "temp", true), ("press", "Pressure", "press", true),
                ("vfrac", "Vapour fraction", "", true), ("density", "Density", "density", true), ("hflow", "Enthalpy flow", "power", true), ("sheet", "Sheet", "", false),
            ];
            let mut rows: Vec<(u64, Row)> = net
                .runs
                .iter()
                .enumerate()
                .filter_map(|(r, lines)| {
                    let o = p.obj(net.run_obj[r]).filter(|o| o.class == crate::model::STREAM_CLASS)?;
                    let l = net.lines[&lines[0]];
                    let (from, to) = net.run_ends(r);
                    let mut cells = specs
                        .iter()
                        .map(|&(k, _, q, _)| match k {
                            "from" => from.as_ref().map(|e| net.describe(e)).unwrap_or_default(),
                            "to" => to.as_ref().map(|e| net.describe(e)).unwrap_or_default(),
                            "sheet" => sheet_no(p, l.sheet),
                            _ => val(&o.data, k, q, us),
                        })
                        .collect::<Vec<String>>();
                    cells.extend(p.settings.components.iter().map(|c| val(&o.data, &format!("x:{c}"), "", us)));
                    Some((cells[0].trim().parse().unwrap_or(u64::MAX), Row { ids: lines.clone(), sheet: l.sheet, cells, status: None }))
                })
                .collect();
            rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cells[0].cmp(&b.1.cells[0])));
            let mut cs = cols(&specs, us);
            cs.extend(p.settings.components.iter().map(|c| Col { key: format!("x:{c}"), label: format!("{c} (mass fraction)"), q: "", edit: true, num: true }));
            (cs, rows.into_iter().map(|x| x.1).collect())
        }
        "io" => {
            let specs: Vec<Spec> = vec![("system", "Control system", "", false), ("ai", "AI", "", false), ("ao", "AO", "", false), ("di", "DI", "", false), ("do", "DO", "", false), ("fb", "Fieldbus", "", false), ("total", "Total", "", false), ("spare", "With 20% spare", "", false)];
            let sum = crate::instruments::io_summary(p);
            let row = |name: String, c: [usize; 5], ids: Vec<u32>| {
                let total: usize = c.iter().sum();
                // 20% spare on each kind, rounded up.
                let spare: usize = c.iter().map(|n| (n * 12).div_ceil(10)).sum();
                let mut cells = vec![name];
                cells.extend(c.iter().map(|n| n.to_string()));
                cells.push(total.to_string());
                cells.push(spare.to_string());
                Row { sheet: ids.first().and_then(|i| p.item(*i)).map_or(0, |i| i.sheet), ids, cells, status: None }
            };
            let mut all = [0usize; 5];
            let mut ids_all = vec![];
            let mut rows: Vec<Row> = vec![];
            for (sys, c, ids) in sum {
                for (a, n) in all.iter_mut().zip(c) {
                    *a += n;
                }
                ids_all.extend(ids.iter().copied());
                rows.push(row(sys, c, ids));
            }
            if rows.len() > 1 {
                rows.push(row("All systems".into(), all, ids_all));
            }
            let mut cs = cols(&specs, us);
            cs.iter_mut().skip(1).for_each(|c| c.num = true);
            (cs, rows)
        }
        "equipment" => {
            let specs: Vec<Spec> = vec![("tag", "Tag", "", true), ("kind", "Type", "", false), ("service", "Service", "", true), ("design_p", "Design P", "press", true), ("design_t", "Design T", "temp", true), ("oper_p", "Oper P", "press", true), ("oper_t", "Oper T", "temp", true), ("material", "Material", "", true), ("capacity", "Size / capacity", "", false), ("power", "Power", "power", true), ("area", "Area", "", false), ("sheet", "Sheet", "", false)];
            let rows = items(&|c| c.equipment())
                .into_iter()
                .map(|(o, it)| {
                    let cap = [("flow", "flow"), ("head", "head"), ("duty", "power"), ("volume", "volume"), ("area", "area")].iter().filter_map(|(k, q)| o.data.get(*k).and_then(|v| v.num()).map(|x| units::show(x, q, us))).chain(o.data.get("size").map(|v| v.text())).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
                    let cells = specs.iter().map(|&(k, _, q, _)| match k { "tag" => o.tag.clone(), "kind" => sym_name(it), "capacity" => cap.clone(), "sheet" => sheet_no(p, it.sheet), "area" => p.node_path(o.node), _ => val(&o.data, k, q, us) }).collect();
                    Row { ids: vec![it.id], sheet: it.sheet, cells, status: None }
                })
                .collect();
            (cols(&specs, us), rows)
        }
        "instruments" => {
            let specs: Vec<Spec> = vec![("tag", "Tag", "", true), ("function", "Function", "", true), ("loop", "Loop", "", false), ("kind", "Type", "", false), ("location", "Location", "", true), ("range", "Range", "", true), ("setpoints", "Set points", "", true), ("signal", "Signal", "", true), ("on", "On", "", false), ("area", "Area", "", false), ("sheet", "Sheet", "", false)];
            let rows = items(&|c| c.instrument())
                .into_iter()
                .map(|(o, it)| {
                    let cells = specs.iter().map(|&(k, _, q, _)| match k { "tag" => o.tag.clone(), "loop" => crate::tags::loop_of(&o.tag).1, "kind" => sym_name(it), "on" => attached(&net, it), "sheet" => sheet_no(p, it.sheet), "area" => p.node_path(o.node), _ => val(&o.data, k, q, us) }).collect();
                    Row { ids: vec![it.id], sheet: it.sheet, cells, status: None }
                })
                .collect();
            (cols(&specs, us), rows)
        }
        "valves" => {
            let specs: Vec<Spec> = vec![("tag", "Tag", "", true), ("kind", "Type", "", false), ("size", "Size", "", true), ("pipe_class", "Pipe class", "", false), ("class", "Rating", "", true), ("operator", "Operator", "", true), ("position", "Position", "", true), ("fail", "Fail", "", true), ("on", "Line", "", false), ("service", "Service", "", true), ("area", "Area", "", false), ("sheet", "Sheet", "", false)];
            let rows = items(&|c| c.valve())
                .into_iter()
                .map(|(o, it)| {
                    let cells = specs.iter().map(|&(k, _, q, _)| match k { "tag" => o.tag.clone(), "kind" => sym_name(it), "on" => attached(&net, it), "sheet" => sheet_no(p, it.sheet), "area" => p.node_path(o.node), _ => val(&o.data, k, q, us) }).collect();
                    Row { ids: vec![it.id], sheet: it.sheet, cells, status: None }
                })
                .collect();
            (cols(&specs, us), rows)
        }
        "takeoff" => {
            let specs: Vec<Spec> = vec![("kind", "Item", "", false), ("size", "Size", "", false), ("pclass", "Pipe class", "", false), ("rating", "Rating", "", false), ("qty", "Quantity", "", false), ("tags", "Tags", "", false), ("sheet", "Sheets", "", false), ("desc", "Description (pipe spec)", "", false), ("code", "Commodity code", "", false)];
            let pfd: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
            // Every piping component on the P&IDs once (an object drawn twice counts once), by
            // what it is, its size, its line's pipe class and its own rating.
            type Key = (usize, String, (u64, String), String, String);
            // (items, tags, untagged count, sheets)
            type Group = (Vec<u32>, Vec<String>, usize, Vec<u32>);
            let mut groups: std::collections::BTreeMap<Key, Group> = std::collections::BTreeMap::new();
            let mut seen = std::collections::HashSet::new();
            for it in p.items.iter().filter(|i| !pfd.contains(&i.sheet)) {
                let Some(s) = symbols::get(&it.sym) else { continue };
                let piping = matches!(s.cat, Cat::Valve | Cat::Control | Cat::Fitting) || (s.cat == Cat::Primary && s.inline);
                if !piping || (it.obj != 0 && !seen.insert(it.obj)) {
                    continue;
                }
                let own = p.data_of(it.obj);
                let txt = |d: &crate::model::Data, k: &str| d.get(k).map(|v| v.text().trim().to_string()).unwrap_or_default();
                // The lines on its process ports, for its size and pipe class.
                let mut sizes: Vec<String> = vec![];
                let mut class = String::new();
                for k in (0..s.ports.len()).filter(|k| !s.ports[*k].signal) {
                    for &(l, _) in net.on_port(it.id, k as u8) {
                        let Some(line) = net.lines.get(&l).filter(|x| x.kind.process() && x.obj != 0) else { continue };
                        let d = p.data_of(line.obj);
                        let sz = txt(d, "size");
                        if !sz.is_empty() && !sizes.contains(&sz) {
                            sizes.push(sz);
                        }
                        if class.is_empty() {
                            class = txt(d, "class");
                        }
                    }
                }
                let inch = |s: &str| if crate::pipes::nps_inches(s.trim_end_matches('"')).is_some() { format!("{}\"", s.trim_end_matches('"')) } else { s.to_string() };
                let size = match txt(own, "size") {
                    s if !s.is_empty() => inch(&s),
                    _ => {
                        sizes.sort_by(|a, b| crate::pipes::nps_inches(b).unwrap_or(0.0).total_cmp(&crate::pipes::nps_inches(a).unwrap_or(0.0)));
                        sizes.iter().map(|s| inch(s)).collect::<Vec<_>>().join(" × ")
                    }
                };
                let order = (crate::pipes::nps_inches(size.split(' ').next().unwrap_or("").trim_end_matches('"')).map_or(u64::MAX, |x| (x * 1000.0) as u64), size.clone());
                let cat = Cat::ALL.iter().position(|c| *c == s.cat).unwrap_or(0);
                let g = groups.entry((cat, sym_name(it), order, class, txt(own, "class"))).or_default();
                g.0.push(it.id);
                let tag = p.tag_of(it).trim().to_string();
                if tag.is_empty() {
                    g.2 += 1;
                } else {
                    g.1.push(tag);
                }
                if !g.3.contains(&it.sheet) {
                    g.3.push(it.sheet);
                }
            }
            let mut rows: Vec<Row> = groups
                .into_iter()
                .map(|((_, name, (_, size), class, rating), (ids, mut tags, untagged, sheets))| {
                    tags.sort_by_key(|t| { let (a, n, _) = crate::tags::split(t); (a, n, t.clone()) });
                    if untagged > 0 {
                        tags.push(format!("{untagged} untagged"));
                    }
                    let mut sh: Vec<u32> = sheets;
                    sh.sort_by_key(|x| p.sheet_no(*x));
                    // From the pipe spec, when its class lists the component at this size.
                    let kind = ids.first().and_then(|i| p.item(*i)).map(|i| i.base_key()).unwrap_or_default();
                    let e = crate::specs::find(p, &class).and_then(|s| crate::specs::entry(s, &kind, size.split(' ').next().unwrap_or("").trim_end_matches('"')));
                    let (desc, code) = e.map(|e| (e.desc.clone(), e.code.clone())).unwrap_or_default();
                    let cells = vec![name, size, class, rating, ids.len().to_string(), tags.join(", "), sh.iter().map(|x| sheet_no(p, *x)).collect::<Vec<_>>().join(", "), desc, code];
                    Row { sheet: ids.first().and_then(|i| p.item(*i)).map_or(0, |i| i.sheet), ids, cells, status: None }
                })
                .collect();
            // Then what their ends bring with them (flanges, gaskets, bolts), from the pipe specs.
            let fmt = |q: f64| if (q - q.round()).abs() < 1e-9 { format!("{}", q.round() as i64) } else { crate::fmt_sig(q, 3) };
            rows.extend(crate::specs::implied(p, &net).into_iter().map(|r| {
                let rating = crate::specs::find(p, &r.class).map(|s| s.rating.clone()).unwrap_or_default();
                let size = if crate::pipes::nps_inches(&r.size).is_some() { format!("{}\"", r.size.trim_end_matches('"')) } else { r.size.clone() };
                let cells = vec![r.what, size, r.class, rating, fmt(r.qty), format!("implied by the {}", r.from.join(", ")), String::new(), r.desc, r.code];
                Row { sheet: r.ids.first().and_then(|i| p.item(*i)).map_or(0, |i| i.sheet), ids: r.ids, cells, status: None }
            }));
            let mut cs = cols(&specs, us);
            cs[4].num = true;
            (cs, rows)
        }
        "jbs" => {
            let specs: Vec<Spec> = vec![("tag", "Junction box", "", true), ("jb_type", "Type", "", true), ("location", "Location", "", true), ("terminals", "Terminals", "", true), ("used", "Used", "", false), ("hazardous_area", "Hazardous area", "", true), ("devices", "Devices", "", false)];
            let mut jbs: Vec<&crate::model::Obj> = p.objects.iter().filter(|o| o.class == crate::wiring::JB).collect();
            jbs.sort_by_key(|o| { let (a, n, _) = crate::tags::split(&o.tag); (a, n, o.tag.clone()) });
            let rows = jbs
                .into_iter()
                .map(|j| {
                    let devs: Vec<(&Item, &crate::model::Obj)> = p.items.iter().filter_map(|i| p.obj(i.obj).map(|o| (i, o))).filter(|(_, o)| o.data.get("jb").is_some_and(|v| v.text().trim().eq_ignore_ascii_case(&j.tag))).collect();
                    let mut seen = std::collections::HashSet::new();
                    let devs: Vec<_> = devs.into_iter().filter(|(_, o)| seen.insert(o.id)).collect();
                    let used: u32 = devs.iter().map(|(_, o)| o.data.get("jb_terminals").map(|v| v.text()).unwrap_or_default()).map(|t| t.split([',', ';', ' ']).filter(|x| !x.trim().is_empty()).map(|x| x.split_once('-').and_then(|(a, b)| Some(b.trim().parse::<u32>().ok()? + 1 - a.trim().parse::<u32>().ok()?)).unwrap_or(1)).sum::<u32>()).sum();
                    let cells = specs.iter().map(|&(k, _, q, _)| match k { "tag" => j.tag.clone(), "used" => used.to_string(), "devices" => devs.iter().map(|(_, o)| o.tag.clone()).collect::<Vec<_>>().join(", "), _ => val(&j.data, k, q, us) }).collect();
                    Row { ids: vec![j.id], sheet: devs.first().map_or(0, |d| d.0.sheet), cells, status: None }
                })
                .collect();
            (cols(&specs, us), rows)
        }
        "cables" => {
            let specs: Vec<Spec> = vec![("tag", "Cable", "", true), ("cable_type", "Type", "", true), ("pairs", "Pairs", "", true), ("used", "Used", "", false), ("from", "From", "", true), ("to", "To", "", true), ("length", "Length", "length", true), ("route", "Route", "", true), ("devices", "Devices", "", false)];
            let rows = crate::wiring::cables(p)
                .into_iter()
                .map(|r| {
                    let c = r.cable;
                    let jbs: Vec<String> = { let mut v: Vec<String> = r.devices.iter().filter_map(|(_, o)| o.data.get("jb").map(|x| x.text())).filter(|x| !x.is_empty()).collect(); v.sort(); v.dedup(); v };
                    let cells = specs
                        .iter()
                        .map(|&(k, _, q, _)| match k {
                            "tag" => c.tag.clone(),
                            "used" => r.devices.len().to_string(),
                            "devices" => r.devices.iter().map(|(_, o)| o.tag.clone()).collect::<Vec<_>>().join(", "),
                            // From the field devices to their junction box, unless typed.
                            "from" if !c.data.contains_key("from") && !r.devices.is_empty() => r.devices.iter().map(|(_, o)| o.tag.clone()).collect::<Vec<_>>().join(", "),
                            "to" if !c.data.contains_key("to") && !jbs.is_empty() => jbs.join(", "),
                            _ => val(&c.data, k, q, us),
                        })
                        .collect();
                    Row { ids: vec![c.id], sheet: r.devices.first().map_or(0, |d| d.0.sheet), cells, status: None }
                })
                .collect();
            let mut cs = cols(&specs, us);
            cs[2].num = true;
            cs[3].num = true;
            (cs, rows)
        }
        "ioalloc" => {
            let specs: Vec<Spec> = vec![("card", "Card", "", false), ("system", "Control system", "", false), ("rack", "Rack / slot", "", false), ("kind", "Kind", "", false), ("ch", "Channel", "", false), ("device", "Device", "", false), ("signal", "Signal", "", false), ("status", "Status", "", false)];
            let rows = crate::wiring::allocation(p)
                .into_iter()
                .map(|r| {
                    let t = |k: &str| r.card.data.get(k).map(|v| v.text()).unwrap_or_default();
                    let status = match r.devices.len() { 0 => "spare", 1 => "used", _ => "clash" };
                    let cells = vec![
                        r.card.tag.clone(),
                        t("system"),
                        [t("rack"), t("slot")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" / "),
                        t("io_kind"),
                        r.ch.to_string(),
                        r.devices.iter().map(|(_, o)| o.tag.clone()).collect::<Vec<_>>().join(", "),
                        r.devices.first().and_then(|(_, o)| o.data.get("signal")).map(|v| v.text()).unwrap_or_default(),
                        status.into(),
                    ];
                    Row { ids: r.devices.iter().map(|(i, _)| i.id).collect(), sheet: r.devices.first().map_or(0, |d| d.0.sheet), cells, status: Some(match r.devices.len() { 0 => 0, 1 => 0, _ => 2 }) }
                })
                .collect();
            let mut cs = cols(&specs, us);
            cs[4].num = true;
            (cs, rows)
        }
        "comments" => {
            let specs: Vec<Spec> = vec![("n", "No.", "", false), ("sheet", "Sheet", "", false), ("by", "By", "", false), ("date", "Date", "", false), ("text", "Comment", "", false), ("replies", "Replies", "", false), ("status", "Status", "", false)];
            let rows = p
                .comments
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let replies = c.replies.iter().map(|r| format!("{}: {}", if r.by.is_empty() { "?" } else { &r.by }, r.text)).collect::<Vec<_>>().join(" | ");
                    let status = if c.resolved { if c.resolved_by.is_empty() { "Resolved".to_string() } else { format!("Resolved ({})", c.resolved_by) } } else { "Open".into() };
                    let cells = vec![(i + 1).to_string(), sheet_no(p, c.sheet), c.by.clone(), crate::review::when(c.time), c.text.clone(), replies, status];
                    Row { ids: c.on.clone(), sheet: c.sheet, cells, status: Some(if c.resolved { 0 } else { 1 }) }
                })
                .collect();
            let mut cs = cols(&specs, us);
            cs[0].num = true;
            (cs, rows)
        }
        "datasheets" => {
            let specs: Vec<Spec> = vec![("tag", "Tag", "", false), ("kind", "Type", "", false), ("rev", "Rev", "", false), ("status", "Status", "", false), ("date", "Date", "", false), ("by", "By", "", false), ("changed", "Changed since issue", "", false), ("area", "Area", "", false), ("sheet", "Sheet", "", false)];
            let rows = crate::datasheets::all(p)
                .into_iter()
                .map(|d| {
                    let it = p.item(d.item);
                    let cells = specs
                        .iter()
                        .map(|&(k, _, _, _)| match k {
                            "tag" => d.tag.clone(),
                            "kind" => it.map(sym_name).unwrap_or_default(),
                            "rev" => d.rev.clone(),
                            "status" => if d.status.is_empty() { "Not issued".into() } else { d.status.clone() },
                            "date" => d.date.clone(),
                            "by" => d.by.clone(),
                            "changed" => if d.rev.is_empty() { String::new() } else if d.changed == 0 { "none".into() } else { format!("Δ {}", d.changed) },
                            "area" => p.obj(d.obj).map(|o| p.node_path(o.node)).unwrap_or_default(),
                            _ => sheet_no(p, d.sheet),
                        })
                        .collect();
                    Row { ids: vec![d.item], sheet: d.sheet, cells, status: if d.rev.is_empty() { None } else { Some(u8::from(d.changed > 0)) } }
                })
                .collect();
            (cols(&specs, us), rows)
        }
        _ => unreachable!(),
    };
    Ok(List { kind: kind.into(), title, cols: cols_, rows, heads: vec![] })
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
                    End::Port { item, .. } => net.items.get(item).filter(|o| symbols::get(&o.sym).is_some_and(|s| s.cat.equipment())).map(|o| net.p.tag_of(o).to_string()).unwrap_or_default(),
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
        if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
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
    // Excel forbids [ ] : * ? / \ in a worksheet's name ("I/O summary").
    let name: String = l.title.chars().map(|c| if "[]:*?/\\".contains(c) { '-' } else { c }).collect();
    ws.set_name(crate::clip(&name, 31)).map_err(e)?;
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
            if l.heads.contains(&r) {
                match cell.parse::<f64>() {
                    Ok(v) if v.is_finite() => ws.write_number_with_format(rr, cc, v, &bold).map_err(e)?,
                    _ => ws.write_string_with_format(rr, cc, cell, &bold).map_err(e)?,
                };
                continue;
            }
            match cell.parse::<f64>() {
                Ok(v) if l.cols[c].num && v.is_finite() => ws.write_number(rr, cc, v).map_err(e)?,
                _ => ws.write_string(rr, cc, cell).map_err(e)?,
            };
        }
    }
    ws.set_freeze_panes(3, 0).map_err(e)?;
    wb.save_to_buffer().map_err(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_take_off_counts_every_valve_and_fitting_once() {
        let p = crate::examples::get("pump").unwrap();
        let l = build(&p, "takeoff").unwrap();
        let row = |name: &str, size: &str| l.rows.iter().find(|r| r.cells[0] == name && r.cells[1] == size).map(|r| r.cells.clone());
        let g3 = row("Gate valve", "3\"").expect("3\" gate valves");
        assert_eq!(g3[2], "A1A");
        assert_eq!(g3[4], "4");
        assert!(g3[5].contains("HV-104") && g3[5].contains("HV-108"));
        // Named from the project's pipe spec.
        assert_eq!((g3[7].as_str(), g3[8].as_str()), ("Gate valve, flanged RF, A216 WCB, trim 8", "VGA150-01"));
        assert_eq!(row("Control valve, pneumatic", "3\"").unwrap()[3], "300#");
        assert!(row("Vent", "3\"").unwrap()[5].contains("1 untagged"));
        // Every piping component once: the quantities add up to the piping objects drawn.
        let piping = p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| matches!(s.cat, Cat::Valve | Cat::Control | Cat::Fitting) || (s.cat == Cat::Primary && s.inline))).map(|i| i.obj).collect::<std::collections::HashSet<_>>().len();
        assert_eq!(l.rows.iter().filter(|r| !r.cells[5].starts_with("implied by")).map(|r| r.cells[4].parse::<usize>().unwrap()).sum::<usize>(), piping);
        // A relief valve between two sizes shows both.
        let c = build(&crate::examples::get("compressor").unwrap(), "takeoff").unwrap();
        assert!(c.rows.iter().any(|r| r.cells[0] == "Pressure safety valve" && r.cells[1] == "3\" × 2\""));
        // Nothing from a process flow diagram.
        let f = build(&crate::examples::get("pfd").unwrap(), "takeoff").unwrap();
        assert!(f.rows.is_empty());
    }
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
