//! Control loops placed in one step: on a line, a flow (orifice and transmitter), pressure or
//! temperature loop; on a vessel, a level loop. The measurement, the controller (in the control
//! system, joined by a signal line) and, for a control loop, the control valve in the line with its
//! signal, all with one loop number. The result is a draft to tidy, like an arrangement.

use crate::doc::{exec_op, next_loop, Op};
use crate::model::{End, Item, Kind, Line, Project};
use crate::symbols;

/// Measured variables: letter and name.
pub const VARS: [(&str, &str); 4] = [("F", "flow"), ("P", "pressure"), ("T", "temperature"), ("L", "level")];

#[allow(clippy::too_many_arguments)]
fn add_item(p: &mut Project, created: &mut Vec<u32>, sheet: u32, sym: &str, at: [f64; 2], rot: u8, tag: &str, on: Option<u32>) -> Result<u32, String> {
    let mut out = vec![];
    exec_op(p, Op::AddItem { item: Item { sheet, sym: sym.into(), x: at[0], y: at[1], rot, ..Item::default() }, on, tag: tag.into(), obj: None }, &mut out)?;
    let id = out[0];
    created.extend(out);
    Ok(id)
}

fn add_line(p: &mut Project, created: &mut Vec<u32>, sheet: u32, a: End, b: End, kind: Kind) -> Result<u32, String> {
    let mut out = vec![];
    exec_op(p, Op::AddLine { line: Line { sheet, kind, a, b, ..Line::default() } }, &mut out)?;
    let id = out[0];
    created.extend(out);
    Ok(id)
}

/// A point a fraction along a path, and the direction of the segment it is on (0 right … 3 up).
fn at(path: &[[f64; 2]], frac: f64) -> Option<([f64; 2], u8)> {
    let total: f64 = path.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum();
    if total <= 0.0 {
        return None;
    }
    let mut left = total * frac;
    for w in path.windows(2) {
        let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len >= left && len > 0.0 {
            let d = if dx.abs() >= dy.abs() { if dx >= 0.0 { 0 } else { 2 } } else if dy >= 0.0 { 1 } else { 3 };
            return Some(([w[0][0] + dx * left / len, w[0][1] + dy * left / len], d));
        }
        left -= len;
    }
    None
}

/// The bubble port facing a direction.
fn port_facing(sym: &str, d: u8) -> u8 {
    symbols::get(sym).and_then(|s| s.ports.iter().position(|q| q.dir == d % 4)).unwrap_or(0) as u8
}

/// Places a loop of variable `var` ("F", "P", "T", "L") on the line piece or vessel `target`;
/// `control` adds the control valve. Returns the loop number.
pub fn place(p: &mut Project, target: u32, var: &str, control: bool, created: &mut Vec<u32>) -> Result<String, String> {
    let var = var.trim().to_uppercase();
    if !VARS.iter().any(|v| v.0 == var) {
        return Err("Choose flow, pressure, temperature or level.".into());
    }
    if let Some(line) = p.line(target).cloned() {
        if var == "L" {
            return Err("A level loop goes on a vessel: select the vessel.".into());
        }
        if !line.kind.process() {
            return Err("Select a process line.".into());
        }
        return on_line(p, &line, &var, control, created);
    }
    let it = p.item(target).cloned().ok_or("Select a process line, or a vessel for a level loop.")?;
    if var != "L" || !symbols::get(&it.sym).is_some_and(|s| s.cat == symbols::Cat::Vessel) {
        return Err("Select a process line (flow, pressure, temperature) or a vessel (level).".into());
    }
    on_vessel(p, &it, control, created)
}

fn on_line(p: &mut Project, line: &Line, var: &str, control: bool, created: &mut Vec<u32>) -> Result<String, String> {
    let sheet = line.sheet;
    let mut path = crate::view::routes(p, sheet, None).remove(&line.id).unwrap_or_default();
    if line.flip {
        path.reverse();
    }
    let (m, md) = at(&path, 0.3).ok_or("That line is too short for a loop.")?;
    let n = next_loop(p, sheet);
    let sep = p.settings.tag_format.sep.clone();
    let t = |letters: &str| format!("{var}{letters}{sep}{n}");
    let mut target = line.id;
    // Flow: an orifice in the line, the transmitter beside it.
    if var == "F" {
        add_item(p, created, sheet, "orifice", m, md, &t("E"), Some(line.id))?;
        // The orifice split the line: the transmitter and valve go on its downstream piece.
        target = p.lines.iter().filter(|l| l.sheet == sheet && l.obj == line.obj && l.id != line.id).map(|l| l.id).max().unwrap_or(line.id);
    }
    // The transmitter above the line, tapped onto it.
    let up = if md % 2 == 0 { [0.0, -1.0] } else { [1.0, 0.0] };
    let tap = if var == "F" { [m[0] - if md == 0 { 6.0 } else { 0.0 }, m[1]] } else { m };
    let tap_line = if var == "F" { p.lines.iter().find(|l| l.sheet == sheet && l.obj == line.obj && l.id != target).map_or(line.id, |l| l.id) } else { line.id };
    let tx = add_item(p, created, sheet, "inst_field", [tap[0] + up[0] * 16.0, tap[1] + up[1] * 16.0], 0, &t("T"), None)?;
    let down = if md % 2 == 0 { 1 } else { 2 };
    add_line(p, created, sheet, End::Port { item: tx, port: port_facing("inst_field", down) }, End::Line { line: tap_line, x: tap[0], y: tap[1] }, Kind::Minor)?;
    // The controller above the transmitter.
    let c = add_item(p, created, sheet, "inst_dcs", [tap[0] + up[0] * 34.0, tap[1] + up[1] * 34.0], 0, &t("IC"), None)?;
    let away = if md % 2 == 0 { 3 } else { 0 };
    add_line(p, created, sheet, End::Port { item: tx, port: port_facing("inst_field", away) }, End::Port { item: c, port: port_facing("inst_dcs", (away + 2) % 4) }, Kind::Electric)?;
    if control {
        let mut path2 = crate::view::routes(p, sheet, None).remove(&target).unwrap_or_default();
        if p.line(target).is_some_and(|l| l.flip) {
            path2.reverse();
        }
        let (v, vd) = at(&path2, if var == "F" { 0.6 } else { 0.7 }).ok_or("That line is too short for a control valve.")?;
        let cv = add_item(p, created, sheet, "cv_pneu", v, vd, &t("V"), Some(target))?;
        let sig = symbols::get("cv_pneu").and_then(|s| s.ports.iter().position(|q| q.signal)).unwrap_or(2) as u8;
        add_line(p, created, sheet, End::Port { item: c, port: port_facing("inst_dcs", if md % 2 == 0 { 0 } else { 1 }) }, End::Port { item: cv, port: sig }, Kind::Electric)?;
    }
    Ok(n)
}

fn on_vessel(p: &mut Project, it: &Item, control: bool, created: &mut Vec<u32>) -> Result<String, String> {
    let s = symbols::get(&it.sym).ok_or("no symbol")?;
    let sheet = it.sheet;
    let free = |p: &Project, k: usize| !p.lines.iter().any(|l| [&l.a, &l.b].iter().any(|e| matches!(e, End::Port { item, port } if *item == it.id && *port as usize == k)));
    let side = ["right upper", "right", "right end", "right middle", "left upper", "left", "bottom right", "top left"].iter().find_map(|n| s.ports.iter().position(|q| !q.signal && q.name == *n).filter(|k| free(p, *k)))
        .or_else(|| (0..s.ports.len()).find(|k| !s.ports[*k].signal && free(p, *k) && s.ports[*k].dir % 2 == 0))
        .ok_or("The vessel has no free nozzle for the level transmitter: give it one, or draw the transmitter yourself.")?;
    let (pos, d) = crate::graph::port_pos(it, side as u8).ok_or("no nozzle")?;
    let dv = symbols::dir_vec(d);
    let n = next_loop(p, sheet);
    let sep = p.settings.tag_format.sep.clone();
    let t = |letters: &str| format!("L{letters}{sep}{n}");
    let tx_at = [pos[0] + dv[0] * 16.0, pos[1] + dv[1] * 16.0];
    let tx = add_item(p, created, sheet, "inst_field", tx_at, 0, &t("T"), None)?;
    add_line(p, created, sheet, End::Port { item: it.id, port: side as u8 }, End::Port { item: tx, port: port_facing("inst_field", (d + 2) % 4) }, Kind::Minor)?;
    let c = add_item(p, created, sheet, "inst_dcs", [tx_at[0], tx_at[1] - 20.0], 0, &t("IC"), None)?;
    add_line(p, created, sheet, End::Port { item: tx, port: port_facing("inst_field", 3) }, End::Port { item: c, port: port_facing("inst_dcs", 1) }, Kind::Electric)?;
    if control {
        // The level valve in the vessel's outlet line.
        let out = ["bottom", "outlet", "liquid out", "boot", "bottom right", "bottom left", "right lower", "left lower"].iter().find_map(|nm| {
            let k = s.ports.iter().position(|q| !q.signal && q.name == *nm)?;
            p.lines.iter().find(|l| l.kind.process() && [&l.a, &l.b].iter().any(|e| matches!(e, End::Port { item, port } if *item == it.id && *port as usize == k))).map(|l| l.id)
        });
        let Some(out) = out else { return Err("Draw the vessel's outlet line first, for the level valve.".into()) };
        let mut path = crate::view::routes(p, sheet, None).remove(&out).unwrap_or_default();
        if p.line(out).is_some_and(|l| l.flip) {
            path.reverse();
        }
        let (v, vd) = at(&path, 0.5).ok_or("The outlet line is too short for a control valve.")?;
        let cv = add_item(p, created, sheet, "cv_pneu", v, vd, &t("V"), Some(out))?;
        let sig = symbols::get("cv_pneu").and_then(|s| s.ports.iter().position(|q| q.signal)).unwrap_or(2) as u8;
        add_line(p, created, sheet, End::Port { item: c, port: port_facing("inst_dcs", 0) }, End::Port { item: cv, port: sig }, Kind::Electric)?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;

    fn errors_and_loops(d: &Doc) -> (Vec<String>, Vec<String>) {
        let i = crate::api::derived(&d.p, 0).issues;
        (i.iter().filter(|x| x.level == "error").map(|x| x.msg.clone()).collect(), i.iter().filter(|x| x.rule == 11 || x.rule == 12).map(|x| x.msg.clone()).collect())
    }

    #[test]
    fn a_loop_of_each_kind_is_placed_whole() {
        for var in ["F", "P", "T"] {
            let mut d = Doc::new(crate::examples::get("pump").unwrap());
            let line = d.p.lines.iter().find(|l| l.kind.process() && d.p.data_of(l.obj).get("seq").map(|v| v.text()) == Some("1007".into())).unwrap().id;
            let (e0, _) = errors_and_loops(&d);
            let n0 = d.p.items.len();
            d.apply(Op::PlaceLoop { target: line, var: var.into(), control: true }).unwrap_or_else(|e| panic!("{var}: {e}"));
            let tags: Vec<String> = d.p.items.iter().skip(n0).map(|i| d.p.tag_of(i).to_string()).filter(|t| !t.is_empty()).collect();
            let n = tags.iter().find(|t| t.starts_with(&format!("{var}T"))).unwrap_or_else(|| panic!("{tags:?}")).split('-').nth(1).unwrap().to_string();
            for want in [format!("{var}T-{n}"), format!("{var}IC-{n}"), format!("{var}V-{n}")] {
                assert!(tags.contains(&want), "{var}: {want} in {tags:?}");
            }
            let (e1, loops) = errors_and_loops(&d);
            assert!(e1.len() <= e0.len(), "{var}: {e1:?}");
            assert!(!loops.iter().any(|m| m.contains(&format!("{var}IC-{n}"))), "{var}: the loop is whole: {loops:?}");
            // The transmitter reads the line's conditions.
            let tx = d.p.items.iter().find(|i| d.p.tag_of(i) == format!("{var}T-{n}")).unwrap();
            assert!(crate::datasheets::process_source(&d.p, &crate::graph::Net::new(&d.p), tx).is_some());
            d.undo();
            assert_eq!(d.p.items.len(), n0);
        }
    }

    #[test]
    fn a_level_loop_on_a_vessel_and_what_is_refused() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let s = d.p.sheets[0].id;
        let v = d.apply(Op::AddItem { item: Item { sheet: s, sym: "vvessel".into(), x: 300.0, y: 230.0, ..Item::default() }, on: None, tag: "V-501".into(), obj: None }).unwrap()[0];
        let bl = d.apply(Op::AddItem { item: Item { sheet: s, sym: "battery_limit".into(), x: 380.0, y: 270.0, ..Item::default() }, on: None, tag: String::new(), obj: None }).unwrap()[0];
        d.apply(Op::AddLine { line: Line { sheet: s, kind: Kind::Major, a: End::Port { item: v, port: 1 }, b: End::Port { item: bl, port: 0 }, ..Line::default() } }).unwrap();
        let n0 = d.p.items.len();
        let r = d.apply(Op::PlaceLoop { target: v, var: "L".into(), control: true });
        assert!(r.is_ok(), "{r:?}");
        let tags: Vec<String> = d.p.items.iter().skip(n0).map(|i| d.p.tag_of(i).to_string()).collect();
        assert!(tags.iter().any(|t| t.starts_with("LT-")) && tags.iter().any(|t| t.starts_with("LIC-")) && tags.iter().any(|t| t.starts_with("LV-")), "{tags:?}");
        // A vessel with every nozzle taken says so.
        let full = d.p.items.iter().find(|i| d.p.tag_of(i) == "V-101").unwrap().id;
        let _ = d.apply(Op::PlaceLoop { target: full, var: "L".into(), control: false });
        assert!(d.apply(Op::PlaceLoop { target: v, var: "F".into(), control: true }).is_err());
        assert!(d.apply(Op::PlaceLoop { target: 999_999, var: "F".into(), control: true }).is_err());
        assert!(d.apply(Op::PlaceLoop { target: v, var: "X".into(), control: true }).is_err());
    }
}
