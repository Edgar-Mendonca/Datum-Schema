//! Develop a P&ID from a process flow diagram: a new P&ID sheet with the PFD's equipment (the same
//! objects, so tags and data are never copied), each stream made a line number with the stream's
//! conditions, a size suggested from the flow and the project's velocity limits, the project's
//! first pipe class, and the standard arrangements placed round the equipment. Each line keeps
//! its stream ("stream" by number, "~stream" by object) so the two stay comparable. Equipment
//! already on a P&ID is left where it is, so developing again adds only what is new.

use crate::doc::{exec_op, Op};
use crate::model::{End, Item, Line, Project, Sheet, Val, LINE_CLASS, SIZES, STREAM_CLASS};
use crate::symbols::{self, Cat};
use std::collections::HashMap;

/// What a development did, for the window to say.
#[derive(Clone, Debug, serde::Serialize, Default, PartialEq)]
pub struct Report {
    pub sheet: u32,
    pub equipment: usize,
    pub lines: usize,
    pub arranged: usize,
    pub notes: Vec<String>,
}

fn num(d: &crate::model::Data, k: &str) -> Option<f64> {
    d.get(k).and_then(|v| v.num()).filter(|x| x.is_finite())
}

/// The velocity limit's service for a stream's phase.
fn service(phase: &str) -> &'static str {
    match phase {
        "Gas" | "Vapour" => "gas",
        "Steam" => "steam",
        "Two-phase" => "twophase",
        _ => "liquid",
    }
}

const COMMON: [&str; 17] = ["1/2", "3/4", "1", "1-1/2", "2", "3", "4", "6", "8", "10", "12", "14", "16", "18", "20", "24", "30"];

/// The smallest nominal size (schedule 40, else the size's first schedule) that keeps the
/// velocity at or below the limit's maximum for this volume flow (m³/s).
pub fn suggest_size(p: &Project, q: f64, phase: &str) -> Option<String> {
    if !(q.is_finite() && q > 0.0) {
        return None;
    }
    let lim = p.settings.limits.iter().find(|l| l.service == service(phase)).or(p.settings.limits.first())?;
    let vmax = if lim.vmax > 0.0 { lim.vmax } else { 3.0 };
    for s in crate::pipes::sizes() {
        // Only the sizes plants use (no 1-1/4", 2-1/2", 3-1/2" or 5").
        if !COMMON.contains(&s.nps) {
            continue;
        }
        let sched = s.schedules.iter().find(|x| x.0 == "40").or(s.schedules.first())?;
        let d = s.od - 2.0 * sched.1;
        if d <= 0.0 {
            continue;
        }
        let v = q / (std::f64::consts::PI * d * d / 4.0);
        if v <= vmax {
            return Some(s.nps.to_string());
        }
    }
    None
}

/// The fluid code for a stream's phase.
fn fluid(phase: &str) -> &'static str {
    match phase {
        "Gas" | "Vapour" => "PG",
        "Steam" => "S",
        _ => "P",
    }
}

/// A free P&ID sheet number from a PFD's ("PFD-300" → "PID-300").
fn number_for(p: &Project, pfd: &Sheet) -> String {
    let base = if pfd.number.to_uppercase().contains("PFD") { pfd.number.replace("PFD", "PID").replace("pfd", "PID") } else { format!("{}-PID", pfd.number) };
    let mut n = base.clone();
    let mut k = 2;
    while p.sheets.iter().any(|s| s.number == n) {
        n = format!("{base}-{k}");
        k += 1;
    }
    n
}

/// Develops the PFD sheet `sheet` into a new P&ID sheet. Returns what was done.
pub fn develop(p: &mut Project, sheet: u32, created: &mut Vec<u32>) -> Result<Report, String> {
    let pfd = p.sheet(sheet).cloned().ok_or("That sheet no longer exists.")?;
    if !pfd.pfd {
        return Err("Develop a P&ID from a process flow diagram sheet (Sheets › PFD Sheet).".into());
    }
    let pid_sheets: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| !s.pfd && !s.legend).map(|s| s.id).collect();
    // Objects already drawn on a P&ID, and streams already made into lines.
    let on_pid: std::collections::HashSet<u32> = p.items.iter().filter(|i| pid_sheets.contains(&i.sheet) && i.obj != 0).map(|i| i.obj).collect();
    let done_streams: std::collections::HashSet<u32> = p.objects.iter().filter(|o| o.class == LINE_CLASS).filter_map(|o| o.data.get("~stream").and_then(|v| v.num())).map(|x| x as u32).collect();
    let items: Vec<Item> = p.items.iter().filter(|i| i.sheet == sheet).filter(|i| symbols::get(&i.sym).is_some_and(|s| !s.cat.instrument())).cloned().collect();
    let lines: Vec<Line> = p.lines.iter().filter(|l| l.sheet == sheet).cloned().collect();
    let new_items: Vec<&Item> = items.iter().filter(|i| i.obj == 0 || !on_pid.contains(&i.obj) || symbols::get(&i.sym).is_some_and(|s| s.cat == Cat::Mark || s.cat == Cat::Connector)).collect();
    let new_lines: Vec<&Line> = lines.iter().filter(|l| !l.kind.process() || !done_streams.contains(&l.obj)).collect();
    let mut rep = Report::default();
    if new_items.iter().all(|i| symbols::get(&i.sym).is_some_and(|s| !s.cat.equipment())) && new_lines.iter().all(|l| !l.kind.process()) {
        return Err("Everything on this PFD is already on a P&ID: nothing new to develop.".into());
    }
    // The new sheet: A1, the PFD spread out so the arrangements fit.
    let (x0, y0, x1, y1) = new_items.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, i| (a.0.min(i.x), a.1.min(i.y), a.2.max(i.x), a.3.max(i.y)));
    let (w, h) = SIZES.iter().find(|s| s.0 == "A1").map_or((841.0, 594.0), |s| (s.1, s.2));
    let (bw, bh) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let scale = (2.0f64).min((w - 140.0) / bw).min((h - 200.0) / bh).max(1.0);
    let at = |x: f64, y: f64| [70.0 + (x - x0) * scale, 90.0 + (y - y0) * scale];
    let mut sh = Sheet { number: number_for(p, &pfd), title: format!("{} (developed from {})", if pfd.title.is_empty() { "P&ID".into() } else { pfd.title.replace("process flow diagram", "P&ID") }, pfd.number), size: "A1".into(), ..Sheet::default() };
    sh.pfd = false;
    let mut out = vec![];
    exec_op(p, Op::AddSheet { sheet: sh }, &mut out)?;
    let sid = *out.first().ok_or("no sheet")?;
    created.extend(out);
    rep.sheet = sid;
    // Equipment and battery limits: views of the same objects (a battery limit has none of its own data).
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut equipment = vec![];
    for it in &new_items {
        let [x, y] = at(it.x, it.y);
        let mut out = vec![];
        let obj = if it.obj != 0 && p.obj(it.obj).is_some() { Some(it.obj) } else { None };
        exec_op(p, Op::AddItem { item: Item { sheet: sid, sym: it.sym.clone(), x, y, rot: it.rot, flip: it.flip, scale: it.scale, ..Item::default() }, on: None, tag: String::new(), obj }, &mut out)?;
        let nid = out[0];
        created.extend(out);
        map.insert(it.id, nid);
        if symbols::get(&it.sym).is_some_and(|s| s.cat.equipment() || s.cat == Cat::Control) {
            equipment.push(nid);
            rep.equipment += 1;
        }
    }
    // Streams become line numbers with their data.
    let class = p.settings.specs.first().map(|s| s.name.clone()).unwrap_or_default();
    for l in new_lines {
        let end = |e: &End| match e {
            End::Port { item, port } => map.get(item).map(|i| End::Port { item: *i, port: *port }),
            End::Free { x, y } => {
                let q = at(*x, *y);
                Some(End::Free { x: q[0], y: q[1] })
            }
            End::Line { .. } => None,
        };
        let (Some(a), Some(b)) = (end(&l.a), end(&l.b)) else {
            rep.notes.push("A stream joined to another stream (a tee) was left out: draw it on the P&ID.".into());
            continue;
        };
        let mut obj = 0;
        if l.kind.process() {
            let s = p.data_of(l.obj).clone();
            let is_stream = p.obj(l.obj).is_some_and(|o| o.class == STREAM_CLASS);
            let o = p.new_obj(LINE_CLASS, "");
            let seq = crate::tags::next_seq(p, sid);
            let phase = s.get("phase").map(|v| v.text()).unwrap_or_default();
            let mut d = crate::model::Data::new();
            d.insert("seq".into(), Val::Text(seq));
            if is_stream {
                let txt = |k: &str| s.get(k).map(|v| v.text()).filter(|t| !t.trim().is_empty());
                if let Some(n) = txt("number") {
                    d.insert("stream".into(), Val::Text(n));
                }
                if let Some(n) = txt("name") {
                    d.insert("service".into(), Val::Text(n));
                }
                if !phase.is_empty() {
                    d.insert("phase".into(), Val::Text(phase.clone()));
                }
                for (from, to) in [("mflow", "mflow"), ("flow", "flow"), ("temp", "oper_t"), ("press", "oper_p"), ("density", "density")] {
                    if let Some(x) = num(&s, from) {
                        d.insert(to.into(), Val::Num(x));
                    }
                }
                let q = num(&s, "flow").or_else(|| num(&s, "mflow").zip(num(&s, "density")).filter(|x| x.1 > 0.0).map(|(m, r)| m / r));
                if let Some(size) = q.and_then(|q| suggest_size(p, q, &phase)) {
                    d.insert("size".into(), Val::Text(size));
                    d.insert("sched".into(), Val::Text("40".into()));
                }
                d.insert("fluid".into(), Val::Text(fluid(&phase).into()));
                if !class.is_empty() {
                    d.insert("class".into(), Val::Text(class.clone()));
                }
                d.insert("~stream".into(), Val::Num(l.obj as f64));
            }
            if let Some(ob) = p.obj_mut(o) {
                ob.data = d;
            }
            obj = o;
        }
        let mut out = vec![];
        exec_op(p, Op::AddLine { line: Line { sheet: sid, kind: l.kind, a, b, obj, ..Line::default() } }, &mut out)?;
        created.extend(out);
        if l.kind.process() {
            rep.lines += 1;
        }
    }
    // The standard arrangements round the equipment.
    if !equipment.is_empty() {
        let (n, notes) = crate::arrange::apply(p, &equipment, "", created).unwrap_or((0, vec![]));
        rep.arranged = n;
        rep.notes.extend(notes);
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;

    #[test]
    fn the_crude_preheat_pfd_becomes_a_p_and_id() {
        let mut d = Doc::new(crate::examples::get("pfd").unwrap());
        let pfd = d.p.sheets[0].id;
        let n_obj = d.p.objects.len();
        let made = d.apply(Op::Develop { sheet: pfd }).unwrap();
        let sid = made[0];
        let sh = d.p.sheet(sid).unwrap();
        assert!(!sh.pfd && sh.number == "PID-300", "{}", sh.number);
        // The same equipment objects, now on two sheets.
        let p301 = d.p.objects.iter().find(|o| o.tag == "P-301").unwrap().id;
        assert_eq!(d.p.items.iter().filter(|i| i.obj == p301).count(), 2);
        // Every stream is a line number with its data, a suggested size and a class.
        let lines: Vec<&crate::model::Obj> = d.p.objects.iter().filter(|o| o.class == LINE_CLASS && o.data.contains_key("~stream")).collect();
        assert_eq!(lines.len(), 8);
        let feed = lines.iter().find(|o| o.data.get("stream").map(|v| v.text()) == Some("1".into())).unwrap();
        assert_eq!(feed.data.get("service").map(|v| v.text()), Some("Crude feed".into()));
        assert!(matches!(feed.data.get("oper_p"), Some(Val::Num(x)) if (x - 1.5e5).abs() < 1.0));
        // 10 kg/s of 850 kg/m³ at no more than 3 m/s: 2.75" inside at least, so 3".
        assert_eq!(feed.data.get("size").map(|v| v.text()), Some("3".into()));
        assert_eq!(lines.iter().find(|o| o.data.get("stream").map(|v| v.text()) == Some("5".into())).unwrap().data.get("fluid").map(|v| v.text()), Some("PG".into()));
        // The arrangements were placed: the pump has its strainer and check valve.
        assert!(d.p.items.iter().any(|i| i.sheet == sid && i.sym == "check"));
        assert!(d.p.items.iter().any(|i| i.sheet == sid && i.sym == "strainer_y"));
        assert!(d.p.objects.len() > n_obj);
        // The line-versus-stream check finds nothing to say: the P&ID agrees with the PFD.
        let issues = crate::api::derived(&d.p, sid).issues;
        assert!(!issues.iter().any(|i| i.level == "error"), "{:#?}", issues.iter().filter(|i| i.level == "error").collect::<Vec<_>>());
        // Developing again adds nothing new.
        assert!(d.apply(Op::Develop { sheet: pfd }).unwrap_err().contains("nothing new"));
        // One undo takes it all back.
        d.undo();
        assert_eq!(d.p.objects.len(), n_obj);
        // Only a PFD can be developed.
        let pid = Doc::new(crate::examples::get("pump").unwrap());
        let mut pid = pid;
        let s = pid.p.sheets[0].id;
        assert!(pid.apply(Op::Develop { sheet: s }).is_err());
    }

    #[test]
    fn sizes_follow_the_flow_and_the_limits() {
        let p = Project::new();
        // 100 m³/h of liquid at 3 m/s needs 108.6 mm inside: a 4" schedule 40 is 102.3 mm, too small.
        let s = suggest_size(&p, 100.0 / 3600.0, "Liquid").unwrap();
        assert!(crate::pipes::nps_inches(&s).unwrap() >= 5.0, "{s}");
        // Gas at 20 m/s needs a much smaller line for the same volume.
        let g = suggest_size(&p, 100.0 / 3600.0, "Gas").unwrap();
        assert!(crate::pipes::nps_inches(&g).unwrap() < crate::pipes::nps_inches(&s).unwrap());
        assert!(suggest_size(&p, f64::NAN, "Liquid").is_none());
        assert!(suggest_size(&p, -1.0, "Liquid").is_none());
    }
}
