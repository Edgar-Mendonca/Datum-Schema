//! A symbol swapped for another, and a spare added. Swapping keeps the item's object (its tag and
//! data) and its lines: each line moves to the new symbol's nozzle of the same name, else the
//! nearest of the same kind; a swap that would leave a line with nowhere to go is refused. The
//! object's class follows the new symbol when it was the old symbol's own class. A spare is a copy
//! beside the original with the next suffix: P-101 becomes P-101A and its spare P-101B.

use crate::doc::{exec_op, Op};
use crate::model::{End, Item, Project};
use crate::symbols::{self, Sym};

/// The ports of `new` that the used ports of `old` move to, or why not.
fn remap(old: &Sym, new: &Sym, used: &[u8]) -> Result<std::collections::HashMap<u8, u8>, String> {
    let mut out = std::collections::HashMap::new();
    let mut taken = std::collections::HashSet::new();
    for &k in used {
        let op = old.ports.get(k as usize).ok_or("lost a port")?;
        let same = new.ports.iter().position(|q| q.name == op.name && q.signal == op.signal).filter(|i| !taken.contains(i));
        let near = || {
            let (ox, oy) = (op.x / old.w.max(1e-9), op.y / old.h.max(1e-9));
            new.ports
                .iter()
                .enumerate()
                .filter(|(i, q)| q.signal == op.signal && !taken.contains(i))
                .min_by(|a, b| {
                    let d = |q: &symbols::Port| (q.x / new.w.max(1e-9) - ox).powi(2) + (q.y / new.h.max(1e-9) - oy).powi(2);
                    d(a.1).total_cmp(&d(b.1))
                })
                .map(|x| x.0)
        };
        let Some(i) = same.or_else(near) else {
            return Err(format!("A {} has no {} for the line on “{}”: take that line off first.", new.name.to_lowercase(), if op.signal { "signal connection" } else { "nozzle" }, op.name));
        };
        taken.insert(i);
        out.insert(k, i as u8);
    }
    Ok(out)
}

/// Swaps an item's symbol for `sym`, keeping its object and its lines.
pub fn swap(p: &mut Project, item: u32, sym: &str) -> Result<(), String> {
    let it = p.item(item).cloned().ok_or("That symbol no longer exists.")?;
    let old = symbols::get(&it.sym).ok_or("There is no such symbol.")?;
    let new = symbols::get(sym).ok_or("There is no such symbol.")?;
    if new.key == old.key {
        return Ok(());
    }
    let kinds = |c: symbols::Cat| if c.equipment() { 0 } else if c.valve() { 1 } else if c.instrument() { 2 } else { 3 + c as u8 };
    if kinds(old.cat) != kinds(new.cat) {
        return Err(format!("A {} can be swapped for another of its kind, not for a {}.", old.name.to_lowercase(), new.name.to_lowercase()));
    }
    let used: Vec<u8> = {
        let mut v: Vec<u8> = p.lines.iter().flat_map(|l| [&l.a, &l.b]).filter_map(|e| match e {
            End::Port { item: i, port } if *i == item => Some(*port),
            _ => None,
        }).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let map = remap(old, new, &used)?;
    for l in p.lines.iter_mut() {
        for e in [&mut l.a, &mut l.b] {
            if let End::Port { item: i, port } = e {
                if *i == item {
                    *port = map[port];
                }
            }
        }
    }
    // The class follows the symbol when it was the old symbol's own.
    let old_class = crate::classes::class_name_for(&it.base_key());
    let new_class = crate::classes::class_name_for(new.key);
    if let Some(o) = p.obj_mut(it.obj) {
        if o.class == old_class && crate::classes::get(&new_class).is_some() {
            o.class = new_class;
        }
    }
    let x = p.items.iter_mut().find(|i| i.id == item).ok_or("lost")?;
    x.sym = new.key.to_string();
    x.base = String::new();
    Ok(())
}

/// The next suffix letter free for a tag's number (A, B, …).
fn with_suffix(p: &Project, pre: &str, sep: &str, n: u64, from: u8) -> Option<String> {
    (from..=b'Z').map(|c| format!("{pre}{sep}{n}{}", c as char)).find(|t| !p.objects.iter().any(|o| o.tag.eq_ignore_ascii_case(t)))
}

/// Adds a spare beside a piece of equipment (or any tagged symbol): P-101 becomes P-101A and the
/// spare P-101B, with the original's data; P-101A gives P-101B (or the next free letter).
pub fn spare(p: &mut Project, item: u32, created: &mut Vec<u32>) -> Result<String, String> {
    let it = p.item(item).cloned().ok_or("That symbol no longer exists.")?;
    let tag = p.obj(it.obj).map(|o| o.tag.clone()).unwrap_or_default();
    let (pre, num, suf) = crate::tags::split(&tag);
    let n = num.ok_or("Give it a numbered tag first (such as P-101).")?;
    let sep = if tag.contains('-') { "-" } else { "" };
    if !suf.is_empty() && !(suf.len() == 1 && suf.chars().all(|c| c.is_ascii_alphabetic())) {
        return Err(format!("{tag} has a suffix that is not a letter."));
    }
    if suf.is_empty() {
        let a = format!("{pre}{sep}{n}A");
        if p.objects.iter().any(|o| o.tag.eq_ignore_ascii_case(&a)) {
            return Err(format!("{a} is already a tag."));
        }
        if let Some(o) = p.obj_mut(it.obj) {
            o.tag = a;
        }
    }
    let from = suf.chars().next().map_or(b'B', |c| c.to_ascii_uppercase() as u8 + 1);
    let next = with_suffix(p, &pre, sep, n, from).ok_or("Every suffix letter is taken.")?;
    // The copy below the original, with its data.
    let h = symbols::get(&it.sym).map_or(20.0, |s| s.h * it.scale);
    let src = p.obj(it.obj).cloned().ok_or("That symbol has no object.")?;
    let mut out = vec![];
    exec_op(p, Op::AddItem { item: Item { sheet: it.sheet, sym: it.sym.clone(), x: it.x, y: it.y + h + 15.0, rot: it.rot, flip: it.flip, scale: it.scale, ..Item::default() }, on: None, tag: next.clone(), obj: None }, &mut out)?;
    let copy = p.item(out[0]).map(|i| i.obj).ok_or("lost the copy")?;
    if let Some(o) = p.obj_mut(copy) {
        o.data = src.data.into_iter().filter(|(k, _)| !k.starts_with('~')).collect();
        o.node = src.node;
        o.class = src.class.clone();
    }
    created.extend(out);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;

    #[test]
    fn a_gate_valve_becomes_a_ball_valve_with_its_tag_data_and_lines() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let hv = d.p.items.iter().find(|i| d.p.tag_of(i) == "HV-104").unwrap().clone();
        let lines: Vec<u32> = d.p.lines.iter().filter(|l| [&l.a, &l.b].iter().any(|e| e.item().is_some_and(|x| x.0 == hv.id))).map(|l| l.id).collect();
        let size = d.p.data_of(hv.obj).get("size").cloned();
        d.apply(Op::SwapSymbol { item: hv.id, sym: "ball".into() }).unwrap();
        let it = d.p.item(hv.id).unwrap();
        assert_eq!((it.sym.as_str(), d.p.tag_of(it)), ("ball", "HV-104"));
        assert_eq!(d.p.data_of(hv.obj).get("size").cloned(), size);
        let now: Vec<u32> = d.p.lines.iter().filter(|l| [&l.a, &l.b].iter().any(|e| e.item().is_some_and(|x| x.0 == hv.id))).map(|l| l.id).collect();
        assert_eq!(now, lines);
        assert!(crate::api::derived(&d.p, 0).issues.iter().all(|i| i.level != "error"), "no new errors");
        // A pump for another pump (its class follows), not a valve for a vessel.
        let pa = d.p.items.iter().find(|i| d.p.tag_of(i) == "P-101A").unwrap().clone();
        d.apply(Op::SwapSymbol { item: pa.id, sym: "pump_vil".into() }).unwrap();
        assert!(d.p.obj(pa.obj).unwrap().class.contains("Pump"));
        assert!(d.apply(Op::SwapSymbol { item: hv.id, sym: "vvessel".into() }).is_err());
        // Undo puts the gate valve back.
        d.undo();
        d.undo();
        assert_eq!(d.p.item(hv.id).unwrap().sym, "gate");
    }

    #[test]
    fn a_spare_takes_the_next_suffix_and_copies_paste_on_it() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let s = d.p.sheets[0].id;
        let p = d.apply(Op::AddItem { item: Item { sheet: s, sym: "pump_cent".into(), x: 300.0, y: 230.0, ..Item::default() }, on: None, tag: "P-201".into(), obj: None }).unwrap()[0];
        let o = d.p.item(p).unwrap().obj;
        d.apply(Op::SetData { ids: vec![o], key: "power".into(), val: Some(crate::model::Val::Num(11e3)) }).unwrap();
        d.apply(Op::AddSpare { item: p }).unwrap();
        let tags: Vec<String> = d.p.objects.iter().map(|o| o.tag.clone()).collect();
        assert!(tags.contains(&"P-201A".to_string()) && tags.contains(&"P-201B".to_string()), "{tags:?}");
        let b = d.p.objects.iter().find(|o| o.tag == "P-201B").unwrap();
        assert!(matches!(b.data.get("power"), Some(crate::model::Val::Num(x)) if *x == 11e3));
        // P-101A's spare is P-101C here (B is taken), and a copy of P-101A pastes as the next free letter.
        let pa = d.p.items.iter().find(|i| d.p.tag_of(i) == "P-101A").unwrap().id;
        d.apply(Op::AddSpare { item: pa }).unwrap();
        assert!(d.p.objects.iter().any(|o| o.tag == "P-101C"));
        let it = d.p.item(pa).unwrap().clone();
        d.apply(Op::Paste { sheet: s, items: vec![it], lines: vec![], notes: vec![], dx: 0.0, dy: 60.0 }).unwrap();
        assert!(d.p.objects.iter().any(|o| o.tag == "P-101D"));
        // No number, no spare.
        let v = d.apply(Op::AddItem { item: Item { sheet: s, sym: "vvessel".into(), x: 100.0, y: 260.0, ..Item::default() }, on: None, tag: "DRUM".into(), obj: None }).unwrap()[0];
        d.apply(Op::SetTag { id: v, tag: "DRUM".into() }).unwrap();
        assert!(d.apply(Op::AddSpare { item: v }).is_err());
    }
}
