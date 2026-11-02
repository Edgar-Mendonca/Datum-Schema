//! The PFD and the P&ID kept in step. A P&ID line linked to a stream ("~stream", set when a P&ID is
//! developed or a PFD made) remembers the values the two last agreed on ("~sync:<key>"). When one
//! side changes, the To do list says which, and one click takes the new value to the other side.
//! When both changed differently, both values are offered. Equipment is shared by both sheets, so
//! only what lives on a PFD but on no P&ID (or the other way round) is listed for it.

use crate::model::{Project, Val, LINE_CLASS, STREAM_CLASS};
use crate::propagate::Todo;
use crate::symbols;

/// (stream key, line key, quantity, label).
pub const PAIRS: [(&str, &str, &str, &str); 7] = [("name", "service", "", "service"), ("phase", "phase", "", "phase"), ("mflow", "mflow", "mflow", "mass flow"), ("flow", "flow", "flow", "volume flow"), ("press", "oper_p", "press", "operating pressure"), ("temp", "oper_t", "temp", "operating temperature"), ("density", "density", "density", "density")];

/// Linked (line object, stream object) pairs.
pub fn pairs(p: &Project) -> Vec<(u32, u32)> {
    p.objects.iter().filter(|o| o.class == LINE_CLASS).filter_map(|o| o.data.get("~stream").and_then(|v| v.num()).map(|s| (o.id, s as u32))).filter(|(_, s)| p.obj(*s).is_some_and(|o| o.class == STREAM_CLASS)).collect()
}

fn same(a: Option<&Val>, b: Option<&Val>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => match (x.num(), y.num()) {
            (Some(m), Some(n)) if matches!(x, Val::Num(_)) || matches!(y, Val::Num(_)) => (m - n).abs() <= 1e-6 * m.abs().max(n.abs()).max(1e-9),
            _ => x.text().trim().eq_ignore_ascii_case(y.text().trim()),
        },
        _ => false,
    }
}

/// Where the two agree, remember it (runs after every edit).
pub fn mark(p: &mut Project) {
    for (l, s) in pairs(p) {
        let sd = p.data_of(s).clone();
        let Some(o) = p.obj_mut(l) else { continue };
        for (sk, lk, _, _) in PAIRS {
            let mk = format!("~sync:{lk}");
            if same(sd.get(sk), o.data.get(lk)) {
                match o.data.get(lk).cloned() {
                    Some(v) => {
                        o.data.insert(mk, v);
                    }
                    None => {
                        o.data.remove(&mk);
                    }
                }
            }
        }
    }
}

fn show(v: Option<&Val>, q: &str, us: bool) -> String {
    match v {
        None => "nothing".into(),
        Some(x) if !q.is_empty() => x.num().map_or(x.text(), |n| crate::units::show(n, q, us)),
        Some(x) => x.text(),
    }
}

/// What to look at between the PFD and the P&ID.
pub fn todo(p: &Project) -> Vec<Todo> {
    let us = p.units == "US";
    let mut v = vec![];
    for (l, s) in pairs(p) {
        let (ld, sd) = (p.data_of(l), p.data_of(s));
        let number = sd.get("number").map(|x| x.text()).unwrap_or_default();
        let line_no = crate::graph::obj_number(p, l);
        let line_item = p.lines.iter().find(|x| x.obj == l);
        let (ids, sheet) = line_item.map_or((vec![], 0), |x| (vec![x.id], x.sheet));
        for (sk, lk, q, label) in PAIRS {
            let (sv, lv) = (sd.get(sk), ld.get(lk));
            if same(sv, lv) {
                continue;
            }
            let last = ld.get(&format!("~sync:{lk}"));
            // Which side moved since they last agreed.
            let (kind, what) = match last {
                Some(_) if same(last, lv) => ("stream", format!("Stream {number}'s {label} is now {}; line {line_no} still has {}.", show(sv, q, us), show(lv, q, us))),
                Some(_) if same(last, sv) => ("line", format!("Line {line_no}'s {label} is now {}; stream {number} still has {}.", show(lv, q, us), show(sv, q, us))),
                _ => ("differ", format!("Line {line_no}'s {label} is {}, stream {number}'s is {}.", show(lv, q, us), show(sv, q, us))),
            };
            v.push(Todo { kind, what, ids: ids.clone(), sheet, key: lk.to_string(), value: String::new(), rev: String::new(), obj: l });
        }
    }
    // Equipment on a PFD and on no P&ID, or on a P&ID and on no PFD (only when both kinds exist).
    let pfd: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    let pid: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| !s.pfd && !s.legend).map(|s| s.id).collect();
    if !pfd.is_empty() && !pid.is_empty() && !pairs(p).is_empty() {
        let mut seen = std::collections::HashSet::new();
        for it in p.items.iter().filter(|i| i.obj != 0 && symbols::get(&i.sym).is_some_and(|s| s.cat.equipment())) {
            if !seen.insert(it.obj) {
                continue;
            }
            let on = |set: &std::collections::HashSet<u32>| p.items.iter().any(|i| i.obj == it.obj && set.contains(&i.sheet));
            let tag = p.tag_of(it).to_string();
            if tag.trim().is_empty() {
                continue;
            }
            if on(&pfd) && !on(&pid) {
                v.push(Todo { kind: "pfd", what: format!("{tag} is on the PFD but on no P&ID: develop the P&ID again to add it."), ids: vec![it.id], sheet: it.sheet, key: "develop".into(), value: String::new(), rev: String::new(), obj: it.obj });
            } else if on(&pid) && !on(&pfd) {
                v.push(Todo { kind: "pfd", what: format!("{tag} is on a P&ID but on no PFD: make the PFD again to add it."), ids: vec![it.id], sheet: it.sheet, key: "make".into(), value: String::new(), rev: String::new(), obj: it.obj });
            }
        }
    }
    v
}

/// Takes one value across: from the stream to its line (`from_stream`), or the other way.
pub fn take(p: &mut Project, line: u32, key: &str, from_stream: bool) -> Result<(), String> {
    let (sk, lk) = PAIRS.iter().find(|x| x.1 == key || x.0 == key).map(|x| (x.0, x.1)).ok_or("That value is not kept in step.")?;
    let line = p.obj_id(line).ok_or("That line no longer exists.")?;
    let s = p.data_of(line).get("~stream").and_then(|v| v.num()).map(|x| x as u32).filter(|s| p.obj(*s).is_some()).ok_or("That line has no stream.")?;
    let (from, fk, to, tk) = if from_stream { (s, sk, line, lk) } else { (line, lk, s, sk) };
    let v = p.data_of(from).get(fk).cloned();
    let o = p.obj_mut(to).ok_or("lost")?;
    match v {
        Some(v) => {
            o.data.insert(tk.to_string(), v);
        }
        None => {
            o.data.remove(tk);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    #[test]
    fn a_change_on_either_side_is_listed_and_taken_across() {
        let mut d = Doc::new(crate::examples::get("pfd").unwrap());
        let pfd = d.p.sheets[0].id;
        d.apply(Op::Develop { sheet: pfd }).unwrap();
        assert!(todo(&d.p).iter().all(|t| t.kind == "pfd" || !["stream", "line", "differ"].contains(&t.kind)), "{:#?}", todo(&d.p));
        let (l, s) = pairs(&d.p).into_iter().find(|(_, s)| d.p.data_of(*s).get("number").map(|v| v.text()) == Some("2".into())).unwrap();
        // The simulator says the pump discharge is now at 13 bar: the stream changed.
        d.apply(Op::SetData { ids: vec![s], key: "press".into(), val: Some(Val::Num(13e5)) }).unwrap();
        let t = todo(&d.p);
        let it = t.iter().find(|t| t.kind == "stream" && t.key == "oper_p").unwrap_or_else(|| panic!("{t:#?}"));
        assert!(it.what.contains("Stream 2's operating pressure is now 13 bar"), "{}", it.what);
        d.apply(Op::Sync { line: l, key: "oper_p".into(), from_stream: true }).unwrap();
        assert!(matches!(d.p.data_of(l).get("oper_p"), Some(Val::Num(x)) if (x - 13e5).abs() < 1e-6));
        assert!(!todo(&d.p).iter().any(|t| t.key == "oper_p"));
        // The P&ID's line gets a new temperature: the line changed; taken to the stream.
        d.apply(Op::SetData { ids: vec![l], key: "oper_t".into(), val: Some(Val::Num(310.0)) }).unwrap();
        assert!(todo(&d.p).iter().any(|t| t.kind == "line" && t.key == "oper_t"));
        d.apply(Op::Sync { line: l, key: "oper_t".into(), from_stream: false }).unwrap();
        assert!(matches!(d.p.data_of(s).get("temp"), Some(Val::Num(x)) if (x - 310.0).abs() < 1e-9));
        // Both changed differently: offered both ways.
        d.apply(Op::SetData { ids: vec![l], key: "density".into(), val: Some(Val::Num(800.0)) }).unwrap();
        d.apply(Op::SetData { ids: vec![s], key: "density".into(), val: Some(Val::Num(900.0)) }).unwrap();
        assert!(todo(&d.p).iter().any(|t| t.kind == "differ" && t.key == "density"));
        // Hostile: not a linked line, not a kept value.
        assert!(d.apply(Op::Sync { line: 999_999, key: "oper_t".into(), from_stream: true }).is_err());
        assert!(d.apply(Op::Sync { line: l, key: "colour".into(), from_stream: true }).is_err());
    }

    #[test]
    fn equipment_missing_on_one_side_is_listed() {
        let mut d = Doc::new(crate::examples::get("pfd").unwrap());
        let pfd = d.p.sheets[0].id;
        d.apply(Op::Develop { sheet: pfd }).unwrap();
        d.apply(Op::AddItem { item: crate::model::Item { sheet: pfd, sym: "vvessel".into(), x: 200.0, y: 220.0, ..Default::default() }, on: None, tag: "V-399".into(), obj: None }).unwrap();
        assert!(todo(&d.p).iter().any(|t| t.kind == "pfd" && t.what.starts_with("V-399 is on the PFD")));
    }
}
