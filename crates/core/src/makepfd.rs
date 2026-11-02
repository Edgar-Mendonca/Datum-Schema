//! Make a process flow diagram from the P&IDs: the main equipment (the same objects) and one stream
//! for each process path between two pieces of equipment or a battery limit, found by walking the
//! piping through valves, fittings and off-page connectors. Instruments, vents, drains and relief
//! valves are left out. Each stream takes its first line's conditions, and each line on the path
//! is linked to its stream ("stream" by number, "~stream" by object), as when a P&ID is developed
//! from a PFD. Equipment already on a PFD and paths already linked are left alone.

use crate::doc::{exec_op, Op};
use crate::graph::Net;
use crate::model::{End, Item, Line, Project, Sheet, Val, LINE_CLASS, SIZES, STREAM_CLASS};
use crate::symbols::{self, Cat};
use std::collections::{HashMap, HashSet};

/// A path's ends: (item, port) at each, and the line-number objects along it.
struct Path {
    a: (u32, u8),
    b: (u32, u8),
    objs: Vec<u32>,
}

/// Where a path may start or end: equipment and battery limits.
fn is_end(sym: &symbols::Sym) -> bool {
    sym.cat.equipment() || sym.key == "battery_limit"
}

/// Walks from an item's port along the piping to the next equipment or battery limit, in the
/// drawn flow direction: away from the port if it is its line's start, else back against the flow.
/// Through in-line items and off-page pairs; at a tee onto another line, on along that line (to its
/// end downstream, or to its start upstream).
fn walk(net: &Net, start: (u32, u8)) -> Option<Path> {
    let mut objs = vec![];
    let &(l0, is_b0) = net.on_port(start.0, start.1).iter().find(|(l, _)| net.lines.get(l).is_some_and(|x| x.kind.process()))?;
    // Downstream when the port is the line's a end (lines are drawn with the flow).
    let down = !is_b0;
    let mut cur = (l0, is_b0);
    let mut seen = HashSet::new();
    for _ in 0..500 {
        let (l, from_b) = cur;
        if !seen.insert(l) {
            return None;
        }
        let line = net.lines.get(&l)?;
        if line.obj != 0 && !objs.contains(&line.obj) {
            objs.push(line.obj);
        }
        let next_end = net.other(l, from_b)?;
        let (item, port) = match *next_end {
            End::Port { item, port } => (item, port),
            End::Line { line: m, .. } => {
                // A tee: on along the joined line, to its end (downstream) or its start (upstream).
                let ml = net.lines.get(&m)?;
                if !ml.kind.process() {
                    return None;
                }
                if ml.obj != 0 && !objs.contains(&ml.obj) {
                    objs.push(ml.obj);
                }
                if !seen.insert(m) {
                    return None;
                }
                let e = if down { &ml.b } else { &ml.a };
                match *e {
                    End::Port { item, port } => (item, port),
                    _ => return None,
                }
            }
            End::Free { .. } => return None,
        };
        let it = net.items.get(&item)?;
        let s = symbols::get(&it.sym)?;
        if is_end(s) {
            return (item != start.0).then_some(Path { a: start, b: (item, port), objs });
        }
        // Through an in-line item to its other process port, or across an off-page pair.
        let at = if s.inline {
            let k = s.ports.iter().enumerate().find(|(k, q)| !q.signal && *k as u8 != port).map(|x| x.0 as u8)?;
            (item, k)
        } else if s.cat == Cat::Connector && it.link != 0 {
            (it.link, 0)
        } else {
            return None;
        };
        // The next piece: the process line on that port other than the one we came by.
        cur = *net.on_port(at.0, at.1).iter().find(|(x, _)| *x != l && net.lines.get(x).is_some_and(|y| y.kind.process()))?;
    }
    None
}

#[derive(Clone, Debug, serde::Serialize, Default, PartialEq)]
pub struct Report {
    pub sheet: u32,
    pub equipment: usize,
    pub streams: usize,
}

/// Makes a PFD sheet from the P&ID sheets `sheets` (empty: all of them).
pub fn make(p: &mut Project, sheets: &[u32], created: &mut Vec<u32>) -> Result<Report, String> {
    let pid: Vec<Sheet> = p.sheets.iter().filter(|s| !s.pfd && !s.legend && (sheets.is_empty() || sheets.contains(&s.id))).cloned().collect();
    if pid.is_empty() {
        return Err("There is no P&ID sheet to make a PFD from.".into());
    }
    let pid_ids: HashSet<u32> = pid.iter().map(|s| s.id).collect();
    let on_pfd: HashSet<u32> = p.items.iter().filter(|i| p.sheet(i.sheet).is_some_and(|s| s.pfd) && i.obj != 0).map(|i| i.obj).collect();
    let linked: HashSet<u32> = p.objects.iter().filter(|o| o.class == LINE_CLASS && o.data.get("~stream").and_then(|v| v.num()).is_some_and(|s| p.obj(s as u32).is_some())).map(|o| o.id).collect();
    // The paths, each once, from the equipment's nozzles.
    let (paths, ends): (Vec<Path>, Vec<Item>) = {
        let net = Net::new(p);
        let starts: Vec<&Item> = p.items.iter().filter(|i| pid_ids.contains(&i.sheet)).filter(|i| symbols::get(&i.sym).is_some_and(is_end)).collect();
        let mut paths: Vec<Path> = vec![];
        // A path is found from both its ends: kept once, by its two nozzles.
        let mut seen_ends: HashSet<((u32, u8), (u32, u8))> = HashSet::new();
        let key = |x: (u32, u8)| (p.item(x.0).map_or(x.0, |i| if i.obj != 0 { i.obj } else { i.id }), x.1);
        for it in &starts {
            let s = symbols::get(&it.sym).ok_or("no symbol")?;
            for k in (0..s.ports.len()).filter(|k| !s.ports[*k].signal) {
                let Some(path) = walk(&net, (it.id, k as u8)) else { continue };
                if !path.objs.is_empty() && path.objs.iter().all(|o| linked.contains(o)) {
                    continue;
                }
                let (ka, kb) = (key(path.a), key(path.b));
                if !seen_ends.insert(if ka <= kb { (ka, kb) } else { (kb, ka) }) {
                    continue;
                }
                paths.push(path);
            }
        }
        // Every end of a path, once per object (an object drawn twice is one piece of equipment).
        let mut seen = HashSet::new();
        let ends: Vec<Item> = paths.iter().flat_map(|x| [x.a.0, x.b.0]).filter_map(|i| p.item(i).cloned()).filter(|i| i.obj == 0 || seen.insert(i.obj)).collect();
        (paths, ends)
    };
    if paths.is_empty() {
        return Err("No process path between two pieces of equipment was found that is not already on a PFD.".into());
    }
    // The new PFD sheet: the P&IDs side by side at half scale.
    let order: HashMap<u32, usize> = pid.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
    let half = 0.5;
    let wide = pid.len() > 2;
    let size = if wide { "A1" } else { "A3" };
    let (w, _) = SIZES.iter().find(|s| s.0 == size).map_or((420.0, 297.0), |s| (s.1, s.2));
    let per_row = ((w - 40.0) / 210.0).floor().max(1.0) as usize;
    let place = |it: &Item| {
        let i = order.get(&it.sheet).copied().unwrap_or(0);
        let (col, row) = (i % per_row, i / per_row);
        [20.0 + col as f64 * 210.0 + it.x * half, 20.0 + row as f64 * 150.0 + it.y * half]
    };
    let mut out = vec![];
    exec_op(p, Op::AddSheet { sheet: Sheet { pfd: true, size: size.into(), title: "Process flow diagram (made from the P&IDs)".into(), number: String::new(), ..Sheet::default() } }, &mut out)?;
    let sid = *out.first().ok_or("no sheet")?;
    created.extend(out);
    let mut rep = Report { sheet: sid, ..Report::default() };
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut by_obj: HashMap<u32, u32> = HashMap::new();
    for it in &ends {
        if it.obj != 0 && on_pfd.contains(&it.obj) && symbols::get(&it.sym).is_some_and(|s| s.cat.equipment()) {
            // Already on a PFD: drawn again here so its streams have somewhere to go.
        }
        let [x, y] = place(it);
        let mut out = vec![];
        let obj = (it.obj != 0 && p.obj(it.obj).is_some()).then_some(it.obj);
        exec_op(p, Op::AddItem { item: Item { sheet: sid, sym: it.sym.clone(), x, y, rot: it.rot, flip: it.flip, scale: it.scale, ..Item::default() }, on: None, tag: String::new(), obj }, &mut out)?;
        map.insert(it.id, out[0]);
        if it.obj != 0 {
            by_obj.insert(it.obj, out[0]);
        }
        if symbols::get(&it.sym).is_some_and(|s| s.cat.equipment()) {
            rep.equipment += 1;
        }
        created.extend(out);
    }
    let pfd_item = |p: &Project, id: u32| map.get(&id).copied().or_else(|| p.item(id).and_then(|i| by_obj.get(&i.obj).copied()));
    // Stream numbers continue the project's.
    let mut next = p.objects.iter().filter(|o| o.class == STREAM_CLASS).filter_map(|o| o.data.get("number").and_then(|v| v.text().trim().parse::<u64>().ok())).max().unwrap_or(0) + 1;
    for path in &paths {
        let (Some(a), Some(b)) = (pfd_item(p, path.a.0), pfd_item(p, path.b.0)) else { continue };
        let first = path.objs.first().map(|o| p.data_of(*o).clone()).unwrap_or_default();
        let txt = |k: &str| first.get(k).map(|v| v.text()).filter(|t| !t.trim().is_empty());
        let number = txt("stream").unwrap_or_else(|| {
            let n = next.to_string();
            next += 1;
            n
        });
        let so = p.new_obj(STREAM_CLASS, "");
        let mut d = crate::model::Data::new();
        d.insert("number".into(), Val::Text(number.clone()));
        if let Some(n) = txt("service") {
            d.insert("name".into(), Val::Text(n));
        }
        if let Some(n) = txt("phase") {
            d.insert("phase".into(), Val::Text(n));
        }
        for (from, to) in [("mflow", "mflow"), ("flow", "flow"), ("oper_t", "temp"), ("oper_p", "press"), ("density", "density")] {
            if let Some(x) = first.get(from).and_then(|v| v.num()).filter(|x| x.is_finite()) {
                d.insert(to.into(), Val::Num(x));
            }
        }
        // Mass flow from the volume flow and density where the line has no mass flow.
        if !d.contains_key("mflow") {
            if let (Some(Val::Num(q)), Some(Val::Num(r))) = (d.get("flow").cloned(), d.get("density").cloned()) {
                d.insert("mflow".into(), Val::Num(q * r));
            }
        }
        if let Some(o) = p.obj_mut(so) {
            o.data = d;
        }
        // Every line number on the path is linked to the stream.
        for &lo in &path.objs {
            if let Some(o) = p.obj_mut(lo) {
                o.data.insert("stream".into(), Val::Text(number.clone()));
                o.data.insert("~stream".into(), Val::Num(so as f64));
            }
        }
        let mut out = vec![];
        exec_op(p, Op::AddLine { line: Line { sheet: sid, kind: crate::model::Kind::Major, a: End::Port { item: a, port: path.a.1 }, b: End::Port { item: b, port: path.b.1 }, obj: so, ..Line::default() } }, &mut out)?;
        created.extend(out);
        rep.streams += 1;
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;

    #[test]
    fn the_pump_transfer_p_and_id_becomes_a_pfd() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let n_obj = d.p.objects.len();
        let made = d.apply(Op::MakePfd { sheets: vec![] }).unwrap();
        let sid = made[0];
        assert!(d.p.sheet(sid).unwrap().pfd);
        let on = |t: &str| d.p.items.iter().any(|i| i.sheet == sid && d.p.tag_of(i) == t);
        // The equipment, not the valves or instruments.
        assert!(on("T-101") && on("P-101A") && on("P-101B") && on("V-101"));
        assert!(!d.p.items.iter().any(|i| i.sheet == sid && symbols::get(&i.sym).is_some_and(|s| s.cat.valve() || s.cat.instrument())));
        // Streams: tank → each pump, each pump → the vessel (through the check valves, the control
        // valve and the joining tee… the tee ends the walk) and the vessel → the battery limit.
        let streams: Vec<&crate::model::Obj> = d.p.objects.iter().filter(|o| o.class == STREAM_CLASS).collect();
        assert!(streams.len() >= 3, "{}", streams.len());
        let out = streams.iter().find(|s| s.data.get("name").map(|v| v.text()) == Some("V-101 outlet".into())).expect("the vessel outlet stream");
        assert!(matches!(out.data.get("mflow"), Some(Val::Num(x)) if (x - 30.0 / 3600.0 * 998.0).abs() < 1e-9));
        assert!(matches!(out.data.get("temp"), Some(Val::Num(x)) if (x - 293.15).abs() < 1e-6) && matches!(out.data.get("press"), Some(Val::Num(x)) if (x - 3.8e5).abs() < 1.0));
        // The lines on each path know their stream; the check comparing them is quiet.
        let lines_linked = d.p.objects.iter().filter(|o| o.class == LINE_CLASS && o.data.contains_key("~stream")).count();
        assert!(lines_linked >= streams.len());
        let issues = crate::api::derived(&d.p, sid).issues;
        assert!(!issues.iter().any(|i| i.rule == 16), "{:#?}", issues.iter().filter(|i| i.rule == 16).collect::<Vec<_>>());
        // Making it again finds nothing new; one undo takes it back.
        assert!(d.apply(Op::MakePfd { sheets: vec![] }).is_err());
        d.undo();
        assert_eq!(d.p.objects.len(), n_obj);
    }

    #[test]
    fn develop_and_make_go_round_trip() {
        // PFD → P&ID → PFD: the second PFD has the same streams between the same equipment.
        let mut d = Doc::new(crate::examples::get("pfd").unwrap());
        let pfd = d.p.sheets[0].id;
        let pid = d.apply(Op::Develop { sheet: pfd }).unwrap()[0];
        // Unlink the developed lines so the P&ID can make its own PFD.
        for o in d.p.objects.iter_mut().filter(|o| o.class == LINE_CLASS) {
            o.data.remove("~stream");
        }
        let n_streams = d.p.objects.iter().filter(|o| o.class == STREAM_CLASS).count();
        let made = d.apply(Op::MakePfd { sheets: vec![pid] }).unwrap();
        let new_streams = d.p.objects.iter().filter(|o| o.class == STREAM_CLASS).count() - n_streams;
        assert_eq!(new_streams, 8, "every stream found again through the arrangements");
        assert!(d.p.sheet(made[0]).unwrap().pfd);
        // Hostile: no P&ID, unknown sheets.
        let mut e = Doc::new(Project::new());
        assert!(e.apply(Op::MakePfd { sheets: vec![999] }).is_err());
    }
}
