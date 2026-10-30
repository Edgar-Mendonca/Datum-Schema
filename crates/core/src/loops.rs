//! Loop diagrams: one per instrument loop (the instruments sharing a first letter and a loop
//! number, such as LT-101, LIC-101 and LV-101), drawn from the P&ID on an A3 sheet with the title
//! block. Each field device is wired through its junction box terminals and cable to its I/O
//! channel in the control system, and on to the controller or display there. The wiring (junction
//! box, terminals, cable, system, I/O channel) is data on each instrument, typed on the loop
//! diagram or in the Inspector; what is missing is listed so the diagram can be completed.

use crate::classes;
use crate::datasheets::{has_datasheet, process_source};
use crate::graph::Net;
use crate::model::{Item, Project, Sheet};
use crate::symbols::{self, Cat, Place, Prim};
use crate::tags;
use serde::Serialize;

/// The wiring attributes every loop member can carry (class library group "Wiring").
pub const WIRING: [(&str, &str); 5] = [("jb", "Junction box"), ("jb_terminals", "JB terminals"), ("cable", "Cable"), ("system", "Control system"), ("io", "I/O channel")];

#[derive(Clone, Debug, Serialize)]
pub struct Member {
    pub obj: u32,
    pub item: u32,
    pub sheet: u32,
    pub tag: String,
    /// "field" or "system" (control room, DCS, PLC, panel).
    pub place: String,
    pub what: String,
    /// The wiring values, in `WIRING` order.
    pub wiring: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopInfo {
    /// The loop: first letter and number, such as "L-101".
    pub key: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopView {
    pub key: String,
    pub number: String,
    pub title: String,
    pub w: f64,
    pub h: f64,
    pub prims: Vec<Prim>,
    pub members: Vec<Member>,
    /// What is still to be filled in, in words.
    pub missing: Vec<String>,
}

fn loop_key(tag: &str) -> Option<String> {
    let (letters, n) = tags::loop_of(tag);
    let first = letters.chars().next()?;
    (!n.is_empty()).then(|| format!("{first}-{n}"))
}

/// Relief devices act alone: they are not in loops.
fn relief(class: &str) -> bool {
    classes::chain(class).iter().any(|c| c.name == "ReliefDevice")
}

/// The loop's members (one symbol per object), measurement first, then the control system, then
/// the final elements.
pub(crate) fn members<'a>(p: &'a Project, key: &str) -> Vec<&'a Item> {
    let mut seen = std::collections::HashSet::new();
    let mut v: Vec<&Item> = p
        .items
        .iter()
        .filter(|i| symbols::get(&i.sym).is_some_and(|s| has_datasheet(s.cat)))
        .filter(|i| p.obj(i.obj).is_some_and(|o| !relief(&o.class) && loop_key(&o.tag).as_deref() == Some(key)))
        .filter(|i| seen.insert(i.obj))
        .collect();
    v.sort_by_key(|i| (rank(p, i), p.tag_of(i).to_string()));
    v
}

pub(crate) fn in_system(p: &Project, it: &Item) -> bool {
    let loc = p.data_of(it.obj).get("location").map(|v| v.text()).unwrap_or_default();
    matches!(it.base_key().as_str(), "inst_dcs" | "inst_plc" | "inst_panel" | "inst_interlock") || loc == "DCS" || loc == "Control room panel"
}

fn rank(p: &Project, it: &Item) -> u8 {
    let letters = tags::loop_of(p.tag_of(it)).0;
    let cat = symbols::get(&it.sym).map(|s| s.cat);
    if cat == Some(Cat::Control) || letters.ends_with('V') {
        2
    } else if in_system(p, it) {
        1
    } else {
        0
    }
}

/// Every loop with at least two members, in tag order.
pub fn all(p: &Project) -> Vec<LoopInfo> {
    let mut keys: Vec<String> = p.objects.iter().filter(|o| !relief(&o.class)).filter_map(|o| loop_key(&o.tag)).collect();
    keys.sort_by_key(|k| {
        let (a, n, _) = tags::split(k);
        (a, n, k.clone())
    });
    keys.dedup();
    keys.into_iter()
        .filter_map(|k| {
            let m = members(p, &k);
            (m.len() >= 2).then(|| LoopInfo { tags: m.iter().map(|i| p.tag_of(i).to_string()).collect(), key: k })
        })
        .collect()
}

fn line(a: [f64; 2], b: [f64; 2], w: f64) -> Prim {
    Prim::Poly { pts: vec![a, b], closed: false, fill: false, dash: false, w }
}
fn dashed(a: [f64; 2], b: [f64; 2]) -> Prim {
    Prim::Poly { pts: vec![a, b], closed: false, fill: false, dash: true, w: 0.25 }
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> Prim {
    Prim::Poly { pts: vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]], closed: true, fill: false, dash: false, w: 0.35 }
}
fn text(x: f64, y: f64, h: f64, s: &str, anchor: u8, bold: bool) -> Prim {
    Prim::Text { p: [x, y], h, s: crate::clip(s, 60), anchor, bold }
}

/// A device's own symbol at (x, y), 1.6 times its P&ID size, with an instrument's letters and loop
/// number written in its bubble. Returns the prims and the half width.
fn device(p: &Project, it: &Item, x: f64, y: f64) -> (Vec<Prim>, f64) {
    let Some(s) = symbols::get(&it.sym) else { return (vec![], 0.0) };
    let k = 1.6;
    let mut v = Place { x, y, rot: 0, flip: false, scale: k }.prims(&symbols::parse(s.draw));
    let (letters, n) = tags::loop_of(p.tag_of(it));
    if s.cat.instrument() && s.cat != Cat::Primary {
        v.push(text(x, y - 1.7 * k, 2.4, &letters, 1, true));
        v.push(text(x, y + 1.9 * k, 2.4, &n, 1, false));
    }
    (v, s.w * k / 2.0)
}

/// The loop diagram of a loop ("L-101"), or of the loop of an object or symbol id.
pub fn diagram(p: &Project, key: &str) -> Result<LoopView, String> {
    let key = match key.parse::<u32>() {
        Ok(id) => p.obj_id(id).and_then(|o| p.obj(o)).and_then(|o| loop_key(&o.tag)).ok_or("That object is not in a loop.")?,
        Err(_) => key.trim().to_uppercase(),
    };
    let ms = members(p, &key);
    if ms.len() < 2 {
        return Err(format!("Loop {key} has fewer than two instruments: there is nothing to wire."));
    }
    let net = Net::new(p);
    let sheet = Sheet { number: format!("LD-{key}"), title: format!("Loop diagram {key}"), rev: String::new(), revs: vec![], ..Sheet::default() };
    let (w, h) = sheet.dims();
    let (x0, y0, x1, _) = crate::frame::inner(&sheet);
    let (_, ytb) = crate::frame::reserved(&sheet);
    let mut v = crate::frame::frame(p, &sheet);
    // Zones across: field, junction box, control system.
    let (xa, xb) = (x0 + (x1 - x0) * 0.42, x0 + (x1 - x0) * 0.62);
    let top = y0 + 4.0;
    for (x, label) in [((x0 + xa) / 2.0, "FIELD"), ((xa + xb) / 2.0, "JUNCTION BOX"), ((xb + x1) / 2.0, "CONTROL SYSTEM")] {
        v.push(text(x, top + 3.0, 3.0, label, 1, true));
    }
    v.push(line([x0, top + 7.0], [x1, top + 7.0], 0.35));
    v.push(dashed([xa, top], [xa, ytb - 4.0]));
    v.push(dashed([xb, top], [xb, ytb - 4.0]));
    let field: Vec<&Item> = ms.iter().copied().filter(|i| !in_system(p, i)).collect();
    let system: Vec<&Item> = ms.iter().copied().filter(|i| in_system(p, i)).collect();
    let rows = field.len().max(1) as f64;
    let (ya, yb) = (top + 18.0, ytb - 14.0);
    let step = ((yb - ya) / rows).min(62.0);
    let wiring = |it: &Item| -> Vec<String> { WIRING.iter().map(|(k, _)| p.data_of(it.obj).get(*k).map(|v| v.text()).unwrap_or_default()).collect() };
    let dash = |s: &str| if s.trim().is_empty() { "—".to_string() } else { s.to_string() };
    let mut missing = vec![];
    let mut io_points = vec![];
    for (i, it) in field.iter().enumerate() {
        let y = ya + step * (i as f64 + 0.5);
        let fx = x0 + 22.0;
        let (sym, half) = device(p, it, fx, y);
        v.extend(sym);
        // What the device is and where it is.
        let d = p.data_of(it.obj);
        let name = symbols::get(&it.sym).map_or("", |s| s.name);
        let on = process_source(p, &net, it).map(|(_, s)| format!("on {s}")).unwrap_or_default();
        let lines = [
            p.tag_of(it).to_string(),
            d.get("function").map(|v| v.text()).filter(|s| !s.is_empty()).unwrap_or_else(|| name.to_string()),
            on,
            d.get("range").map(|v| format!("Range {}", v.text())).unwrap_or_default(),
            d.get("fail").map(|v| format!("Fail {}", v.text())).unwrap_or_default(),
        ];
        // Under the symbol, clear of the wiring.
        let below = symbols::get(&it.sym).map_or(8.0, |s| s.h * 0.8) + 4.0;
        let mut ty = y + below;
        for (k, s) in lines.iter().filter(|s| !s.is_empty()).enumerate() {
            v.push(text(fx - 12.0, ty, 2.4, s, 0, k == 0));
            ty += 4.2;
        }
        let wv = wiring(it);
        // A primary element is not wired: it serves the transmitter beside it.
        if symbols::get(&it.sym).is_some_and(|s| s.cat == Cat::Primary) {
            if let Some(t) = field.iter().find(|o| tags::loop_of(p.tag_of(o)).0.ends_with('T')) {
                v.push(text(fx + half + 4.0, y, 2.2, &format!("Process connection to {}", p.tag_of(t)), 0, false));
            }
            continue;
        }
        // The pair to the junction box, with the cable and the signal on it.
        let (jx, jw) = (xa + 6.0, xb - xa - 12.0);
        let start = fx + half + 1.0;
        for dy in [-1.2, 1.2] {
            v.push(line([start, y + dy], [jx, y + dy], 0.3));
        }
        let signal = d.get("signal").map(|v| v.text()).unwrap_or_default();
        let mid = (start.max(fx + 60.0) + jx) / 2.0;
        v.push(text(mid, y - 4.0, 2.2, &format!("Cable {}", dash(&wv[2])), 1, false));
        if !signal.is_empty() {
            v.push(text(mid, y + 4.5, 2.2, &signal, 1, false));
        }
        // The junction box terminals.
        let terms: Vec<String> = wv[1].split([',', ';', '/', ' ']).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).take(4).collect();
        let n = terms.len().max(2);
        let th = 5.0;
        let bh = th * n as f64;
        v.push(rect(jx, y - bh / 2.0, jw, bh));
        v.push(text(jx + jw / 2.0, y - bh / 2.0 - 3.0, 2.4, &dash(&wv[0]), 1, true));
        for t in 0..n {
            let ty = y - bh / 2.0 + th * (t as f64 + 0.5);
            if t > 0 {
                v.push(line([jx, y - bh / 2.0 + th * t as f64], [jx + jw, y - bh / 2.0 + th * t as f64], 0.18));
            }
            v.push(text(jx + jw / 2.0, ty, 2.2, terms.get(t).map_or("—", |s| s.as_str()), 1, false));
        }
        // On to the I/O channel in the control system.
        let (ix, iw) = (xb + 8.0, 34.0);
        for dy in [-1.2, 1.2] {
            v.push(line([jx + jw, y + dy], [ix, y + dy], 0.3));
        }
        v.push(rect(ix, y - 5.0, iw, 10.0));
        v.push(text(ix + iw / 2.0, y - 2.0, 2.2, &dash(&wv[4]), 1, true));
        v.push(text(ix + iw / 2.0, y + 2.3, 2.0, &dash(&wv[3]), 1, false));
        io_points.push([ix + iw, y]);
        let gaps: Vec<&str> = WIRING.iter().zip(&wv).filter(|(_, val)| val.trim().is_empty()).map(|((_, l), _)| *l).collect();
        if !gaps.is_empty() {
            missing.push(format!("{}: {}", p.tag_of(it), gaps.join(", ").to_lowercase()));
        }
    }
    // The control system's functions (controllers, displays), joined to every I/O channel.
    let sx = x1 - 34.0;
    let sy0 = io_points.iter().map(|q| q[1]).sum::<f64>() / io_points.len().max(1) as f64;
    for (k, it) in system.iter().enumerate() {
        let y = sy0 + (k as f64 - (system.len() as f64 - 1.0) / 2.0) * 30.0;
        let (sym, half) = device(p, it, sx, y);
        v.extend(sym);
        v.push(text(sx, y + 14.0, 2.4, p.tag_of(it), 1, true));
        for q in &io_points {
            let xm = (q[0] + sx - half) / 2.0;
            v.push(Prim::Poly { pts: vec![*q, [xm, q[1]], [xm, y], [sx - half - 0.5, y]], closed: false, fill: false, dash: true, w: 0.3 });
        }
        let wv = wiring(it);
        if !wv[3].is_empty() {
            v.push(text(sx, y - 14.0, 2.2, &wv[3], 1, false));
        }
    }
    if system.is_empty() && !io_points.is_empty() {
        missing.push("The loop has no controller or display in the control system.".into());
    }
    // Notes: what is left to fill in.
    if !missing.is_empty() {
        let mut ny = ytb - 1.0 - 4.0 * (missing.len().min(6) as f64);
        v.push(text(x0 + 2.0, ny - 4.0, 2.4, "TO COMPLETE", 0, true));
        for m in missing.iter().take(6) {
            v.push(text(x0 + 2.0, ny, 2.2, m, 0, false));
            ny += 4.0;
        }
    }
    let members = ms.iter().map(|it| Member { obj: it.obj, item: it.id, sheet: it.sheet, tag: p.tag_of(it).to_string(), place: if in_system(p, it) { "system" } else { "field" }.into(), what: symbols::get(&it.sym).map_or(String::new(), |s| s.name.to_string()), wiring: wiring(it) }).collect();
    Ok(LoopView { number: sheet.number.clone(), title: sheet.title.clone(), key, w, h, prims: v, members, missing })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Val;

    fn texts(v: &LoopView) -> Vec<String> {
        v.prims.iter().filter_map(|x| if let Prim::Text { s, .. } = x { Some(s.clone()) } else { None }).collect()
    }

    #[test]
    fn loops_are_found_and_drawn_with_their_wiring() {
        let mut p = crate::examples::get("pump").unwrap();
        let ls = all(&p);
        let l = ls.iter().find(|l| l.key == "L-101").unwrap_or_else(|| panic!("{ls:?}"));
        assert_eq!(l.tags, ["LT-101", "LIC-101", "LV-101"]);
        // Relief valves and a lone gauge are not loops.
        assert!(ls.iter().all(|l| l.key != "P-101" && l.key != "P-102"), "{ls:?}");
        let d = diagram(&p, "L-101").unwrap();
        let t = texts(&d);
        for s in ["FIELD", "JUNCTION BOX", "CONTROL SYSTEM", "LD-L-101", "Loop diagram L-101", "LT", "LIC", "101", "on V-101", "TO COMPLETE"] {
            assert!(t.iter().any(|x| x == s), "missing {s}: {t:?}");
        }
        assert!(d.missing.iter().any(|m| m.starts_with("LT-101: junction box")), "{:?}", d.missing);
        assert_eq!(d.members.iter().map(|m| (m.tag.as_str(), m.place.as_str())).collect::<Vec<_>>(), [("LT-101", "field"), ("LIC-101", "system"), ("LV-101", "field")]);
        // Wiring typed on the instruments shows on the diagram and completes it.
        for tag in ["LT-101", "LV-101"] {
            let o = p.objects.iter().position(|o| o.tag == tag).unwrap();
            for (k, val) in [("jb", "JB-101"), ("jb_terminals", "1, 2"), ("cable", "C-101-01"), ("system", "DCS cabinet 1"), ("io", "AI 01-03")] {
                p.objects[o].data.insert(k.into(), Val::Text(val.into()));
            }
        }
        let d = diagram(&p, "l-101").unwrap();
        let t = texts(&d);
        for s in ["JB-101", "1", "2", "Cable C-101-01", "AI 01-03", "DCS cabinet 1", "4–20 mA HART"] {
            assert!(t.iter().any(|x| x == s), "missing {s}: {t:?}");
        }
        assert!(d.missing.is_empty(), "{:?}", d.missing);
        // By object id too; a loop of one is refused.
        let lt = p.objects.iter().find(|o| o.tag == "LT-101").unwrap().id;
        assert_eq!(diagram(&p, &lt.to_string()).unwrap().key, "L-101");
        assert!(diagram(&p, "P-102").is_err());
        assert!(diagram(&p, "Z-999").is_err());
    }

    #[test]
    fn every_example_loop_draws() {
        for id in crate::examples::list().iter().map(|e| e.id) {
            let p = crate::examples::get(id).unwrap();
            for l in all(&p) {
                let d = diagram(&p, &l.key).unwrap();
                assert!(d.prims.len() > 40, "{id} {}", l.key);
                assert!(d.prims.iter().all(|x| match x {
                    Prim::Text { p, .. } => p[0] >= 0.0 && p[0] <= d.w && p[1] >= 0.0 && p[1] <= d.h,
                    _ => true,
                }));
            }
        }
    }

    #[test]
    fn a_loop_diagram_exports_to_dxf() {
        let p = crate::examples::get("pump").unwrap();
        let d = diagram(&p, "L-101").unwrap();
        let text = crate::dxf::write_prims(&d.prims, d.w, d.h);
        let (layers, _, counts) = crate::dxf::read_back(&text).unwrap();
        assert!(["FRAME", "TEXT", "DRAWING"].iter().all(|l| layers.iter().any(|x| x == l)), "{layers:?}");
        assert!(counts.get("TEXT").copied().unwrap_or(0) > 20 && counts.get("POLYLINE").copied().unwrap_or(0) > 10, "{counts:?}");
        assert!(text.contains("LD-L-101"));
    }
}
