//! Nozzles added to equipment. A piece of equipment's symbol has fixed nozzles; adding one makes a
//! project symbol for that symbol on the drawing ("x.nozzles.<item>"): the same drawing and the same
//! nozzles plus the new ones, "like" the original so its class, checks and lists are unchanged, kept
//! in the project file like a client library's symbols. Existing nozzles keep their numbers, so the
//! lines on them stay where they are. A nozzle's size, rating and service are the object's data
//! ("nz.<name>.size" and so on), shown in the Inspector and on the equipment's datasheet.

use crate::model::{Item, Project, Val};
use crate::symbols;
use crate::symlib::{CPort, CustomSym};

pub const FIELDS: [(&str, &str); 3] = [("size", "Size"), ("rating", "Rating"), ("service", "Service")];

/// The data key of one of a nozzle's fields.
pub fn key(nozzle: &str, field: &str) -> String {
    format!("nz.{}.{field}", nozzle.trim())
}

/// The project symbol an item would carry with its added nozzles.
fn derived(p: &Project, it: &Item) -> Result<CustomSym, String> {
    let key = format!("x.nozzles.{}", it.id);
    if let Some(c) = p.custom_syms.iter().find(|c| c.key == it.sym && c.key == key) {
        return Ok(c.clone());
    }
    let s = symbols::get(&it.sym).ok_or("There is no such symbol.")?;
    if !s.cat.equipment() {
        return Err("Nozzles are added to equipment (vessels, machines, exchangers).".into());
    }
    let base = crate::symlib::definition(&it.sym);
    let like = base.as_ref().map_or_else(|| s.key.to_string(), |d| if d.like.is_empty() { d.key.clone() } else { d.like.clone() });
    Ok(CustomSym {
        key,
        name: s.name.to_string(),
        library: "nozzles".into(),
        block: base.as_ref().map(|d| d.block.clone()).unwrap_or_default(),
        like: like.clone(),
        class: base.as_ref().map_or_else(|| crate::classes::class_name_for(s.key), |d| d.class.clone()),
        cat: s.cat,
        w: s.w,
        h: s.h,
        draw: s.draw.to_string(),
        ports: s.ports.iter().map(|q| CPort { x: q.x, y: q.y, dir: q.dir, name: q.name.to_string(), signal: q.signal }).collect(),
        prefix: s.prefix.to_string(),
        inline: s.inline,
        terminal: s.terminal,
    })
}

/// The next free nozzle name: N1, N2, …
fn next_name(c: &CustomSym) -> String {
    (1..).map(|n| format!("N{n}")).find(|n| !c.ports.iter().any(|q| q.name.eq_ignore_ascii_case(n))).unwrap_or_default()
}

/// Adds a nozzle to an equipment symbol where the sheet point (x, y) is nearest its outline,
/// facing outward. Returns its name.
pub fn add(p: &mut Project, item: u32, x: f64, y: f64, name: &str) -> Result<String, String> {
    let it = p.item(item).cloned().ok_or("That symbol no longer exists.")?;
    let mut c = derived(p, &it)?;
    if c.ports.len() >= 60 {
        return Err("That symbol has as many nozzles as it can take.".into());
    }
    // The point in the symbol's own coordinates (undo the turn, mirror and scale).
    let (mut dx, mut dy) = ((x - it.x) / it.scale.max(0.01), (y - it.y) / it.scale.max(0.01));
    for _ in 0..(4 - it.rot % 4) % 4 {
        (dx, dy) = (-dy, dx);
    }
    if it.flip {
        dx = -dx;
    }
    if !dx.is_finite() || !dy.is_finite() {
        return Err("A position is out of range.".into());
    }
    // Onto the nearest side of its box, facing out; on a grid of 0.5 mm.
    let (hw, hh) = (c.w / 2.0, c.h / 2.0);
    let dir = crate::symlib::side_dir(dx.clamp(-hw, hw), dy.clamp(-hh, hh), c.w, c.h);
    let r = |v: f64| (v * 2.0).round() / 2.0;
    let (px, py) = match dir {
        0 => (hw, r(dy.clamp(-hh, hh))),
        2 => (-hw, r(dy.clamp(-hh, hh))),
        1 => (r(dx.clamp(-hw, hw)), hh),
        _ => (r(dx.clamp(-hw, hw)), -hh),
    };
    if c.ports.iter().any(|q| (q.x - px).abs() < 1.5 && (q.y - py).abs() < 1.5) {
        return Err("There is a nozzle there already.".into());
    }
    let name = crate::clip(name.trim(), 30);
    let name = if name.is_empty() || c.ports.iter().any(|q| q.name.eq_ignore_ascii_case(&name)) { next_name(&c) } else { name };
    c.ports.push(CPort { x: px, y: py, dir, name: name.clone(), signal: false });
    let like = c.like.clone();
    let key = c.key.clone();
    crate::symlib::register(&c);
    p.custom_syms.retain(|x| x.key != key);
    p.custom_syms.push(c);
    let item = p.items.iter_mut().find(|i| i.id == item).ok_or("lost")?;
    // `base` stays as it was: the drawing standard maps it, and the definition's `like` gives the base key.
    item.sym = key;
    let _ = like;
    Ok(name)
}

/// Removes an added nozzle that has no line on it (the symbol's own cannot be removed).
pub fn remove(p: &mut Project, item: u32, name: &str) -> Result<(), String> {
    let it = p.item(item).cloned().ok_or("That symbol no longer exists.")?;
    let mut c = p.custom_syms.iter().find(|c| c.key == it.sym && c.library == "nozzles").cloned().ok_or("Only nozzles added to a symbol can be removed.")?;
    let k = c.ports.iter().position(|q| q.name == name).ok_or("There is no such nozzle.")?;
    let base = symbols::get(&c.like).map_or(0, |s| s.ports.len());
    if k < base {
        return Err("A symbol's own nozzles stay; only added ones can be removed.".into());
    }
    if p.lines.iter().any(|l| [&l.a, &l.b].iter().any(|e| matches!(e, crate::model::End::Port { item: i, port } if *i == item && *port as usize == k))) {
        return Err("Take the line off that nozzle first.".into());
    }
    // Later ports move down one: lines on them follow.
    for l in p.lines.iter_mut() {
        for e in [&mut l.a, &mut l.b] {
            if let crate::model::End::Port { item: i, port } = e {
                if *i == item && *port as usize > k {
                    *port -= 1;
                }
            }
        }
    }
    c.ports.remove(k);
    crate::symlib::register(&c);
    let key = c.key.clone();
    p.custom_syms.retain(|x| x.key != key);
    p.custom_syms.push(c);
    if let Some(o) = p.obj_mut(it.obj) {
        for (f, _) in FIELDS {
            o.data.remove(&key_for(name, f));
        }
    }
    Ok(())
}

fn key_for(name: &str, f: &str) -> String {
    key(name, f)
}

/// Every nozzle of a symbol with its data: (index, name, size, rating, service, added).
pub fn list(p: &Project, item: u32) -> Vec<(usize, String, String, String, String, bool)> {
    let Some(it) = p.item(item) else { return vec![] };
    let Some(s) = symbols::get(&it.sym) else { return vec![] };
    let base = crate::symlib::definition(&it.sym).filter(|d| d.library == "nozzles").and_then(|d| symbols::get(&d.like)).map_or(s.ports.len(), |b| b.ports.len());
    let d = p.data_of(it.obj);
    let get = |n: &str, f: &str| d.get(&key(n, f)).map(|v| v.text()).unwrap_or_default();
    s.ports.iter().enumerate().filter(|(_, q)| !q.signal).map(|(k, q)| (k, q.name.to_string(), get(q.name, "size"), get(q.name, "rating"), get(q.name, "service"), k >= base)).collect()
}

/// Sets one field of a nozzle (empty clears it).
pub fn set(p: &mut Project, item: u32, name: &str, field: &str, value: &str) -> Result<(), String> {
    if !FIELDS.iter().any(|f| f.0 == field) {
        return Err("A nozzle has a size, a rating and a service.".into());
    }
    let it = p.item(item).cloned().ok_or("That symbol no longer exists.")?;
    if !symbols::get(&it.sym).is_some_and(|s| s.ports.iter().any(|q| q.name == name)) {
        return Err("There is no such nozzle.".into());
    }
    let o = p.obj_mut(it.obj).ok_or("That symbol has no object.")?;
    let v = crate::clip(value.trim(), 60);
    if v.is_empty() {
        o.data.remove(&key(name, field));
    } else {
        o.data.insert(key(name, field), Val::Text(v));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::{End, Kind, Line};

    #[test]
    fn a_nozzle_is_added_used_and_removed() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let v = d.p.items.iter().find(|i| d.p.tag_of(i) == "V-101").unwrap().clone();
        let n0 = symbols::get(&v.sym).unwrap().ports.len();
        let lines0: Vec<Line> = d.p.lines.clone();
        // A click left of the vessel: a nozzle on its left side, facing left.
        d.apply(Op::AddNozzle { item: v.id, x: v.x - 30.0, y: v.y + 4.0, name: String::new() }).unwrap();
        let it = d.p.item(v.id).unwrap().clone();
        let s = symbols::get(&it.sym).unwrap();
        assert_eq!(s.ports.len(), n0 + 1);
        let new = s.ports.last().unwrap();
        assert_eq!((new.name, new.dir), ("N1", 2));
        // The same class, base and checks; every line still on its nozzle.
        assert_eq!(it.base_key(), "vvessel");
        assert_eq!(d.p.lines.iter().map(|l| (&l.a, &l.b)).collect::<Vec<_>>(), lines0.iter().map(|l| (&l.a, &l.b)).collect::<Vec<_>>());
        assert!(crate::api::derived(&d.p, 0).issues.iter().all(|i| i.level != "error"));
        // Its data, and a line drawn to it.
        d.apply(Op::SetNozzle { item: v.id, name: "N1".into(), field: "size".into(), value: "2".into() }).unwrap();
        assert_eq!(list(&d.p, v.id).last().unwrap().2, "2");
        let s0 = d.p.sheets[0].id;
        d.apply(Op::AddLine { line: Line { sheet: s0, kind: Kind::Minor, a: End::Port { item: v.id, port: n0 as u8 }, b: End::Free { x: v.x - 60.0, y: v.y + 4.0 }, ..Line::default() } }).unwrap();
        // Not removed while a line is on it; the symbol's own never.
        assert!(d.apply(Op::RemoveNozzle { item: v.id, name: "N1".into() }).is_err());
        assert!(d.apply(Op::RemoveNozzle { item: v.id, name: "top".into() }).is_err());
        // Saved and opened again: the nozzle is still there.
        let path = std::env::temp_dir().join(format!("nozzle-{}.dschema", std::process::id()));
        crate::file::write(&path, &d.p, &d.history).unwrap();
        let (q, _, _) = crate::file::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(symbols::get(&q.item(v.id).unwrap().sym).unwrap().ports.len(), n0 + 1);
        // A second nozzle where the first is is refused; a pump is not a vessel but takes nozzles too.
        assert!(d.apply(Op::AddNozzle { item: v.id, x: v.x - 30.0, y: v.y + 4.0, name: String::new() }).is_err());
        let gate = d.p.items.iter().find(|i| i.sym == "gate").unwrap().id;
        assert!(d.apply(Op::AddNozzle { item: gate, x: 0.0, y: 0.0, name: String::new() }).is_err());
    }

    #[test]
    fn many_nozzles_round_and_hostile_clicks() {
        let mut d = Doc::new(crate::examples::get("compressor").unwrap());
        let eq: Vec<crate::model::Item> = d.p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| s.cat.equipment())).cloned().collect();
        for it in &eq {
            for k in 0..30 {
                let a = k as f64 * 0.7;
                let _ = d.apply(Op::AddNozzle { item: it.id, x: it.x + 40.0 * a.cos(), y: it.y + 40.0 * a.sin(), name: if k % 3 == 0 { "\u{0}<b>".into() } else { String::new() } });
            }
            for v in [f64::NAN, f64::INFINITY, 1e300] {
                assert!(d.apply(Op::AddNozzle { item: it.id, x: v, y: 0.0, name: String::new() }).is_err());
            }
        }
        // Every nozzle survives saving and opening again.
        let path = std::env::temp_dir().join(format!("nozzles-many-{}.dschema", std::process::id()));
        crate::file::write(&path, &d.p, &d.history).unwrap();
        let (q, _, _) = crate::file::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        for it in &eq {
            let a = symbols::get(&d.p.item(it.id).unwrap().sym).unwrap().ports.len();
            let b = symbols::get(&q.item(it.id).unwrap().sym).unwrap().ports.len();
            assert_eq!(a, b);
        }
    }
}
