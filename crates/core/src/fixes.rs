//! Fixes offered for the checks: for a problem (its rule and the things it is about), the edits
//! that would put it right, each a label the window offers and one undoable edit
//! (`Op::Fix { rule, ids, choice }`). A fix is worked out again when it is applied, from the
//! project as it is then, so it never acts on stale data.

use crate::doc::{exec_op, Op};
use crate::graph::Net;
use crate::model::{End, Item, Project, Val};
use crate::symbols;

/// What a fix does, worked out from the project.
enum Plan {
    Ops(Vec<Op>),
    /// End a line's open end at a new battery limit.
    Limit { line: u32, end_b: bool },
    /// Put a check valve in a pump's discharge line.
    Check { pump: u32 },
}

fn data(p: &Project, id: u32) -> &crate::model::Data {
    p.obj_id(id).map_or_else(|| p.data_of(0), |o| p.data_of(o))
}

fn text(p: &Project, id: u32, k: &str) -> String {
    data(p, id).get(k).map(|v| v.text().trim().to_string()).unwrap_or_default()
}

/// The process lines either side of an in-line item.
fn sides(p: &Project, item: u32) -> Vec<u32> {
    p.lines.iter().filter(|l| l.kind.process() && [&l.a, &l.b].iter().any(|e| e.item().is_some_and(|x| x.0 == item))).map(|l| l.id).collect()
}

fn set(ids: Vec<u32>, key: &str, val: Option<Val>) -> Op {
    Op::SetData { ids, key: key.into(), val }
}

/// The fixes for a problem: (label, plan).
fn plans(p: &Project, rule: u8, ids: &[u32]) -> Vec<(String, Plan)> {
    let mut v: Vec<(String, Plan)> = vec![];
    let Some(&first) = ids.first() else { return v };
    match rule {
        1 => {
            // Duplicate tags: the others take their next free tags.
            let mut taken = std::collections::HashSet::new();
            let mut ops = vec![];
            for &id in ids.iter().skip(1) {
                if let Some(o) = p.obj_id(id).and_then(|o| p.obj(o)) {
                    if o.tag.trim().is_empty() {
                        continue;
                    }
                    let t = crate::tags::bump(p, &o.tag, &taken);
                    taken.insert(t.to_uppercase());
                    ops.push(Op::SetTag { id: o.id, tag: t });
                }
            }
            if !ops.is_empty() {
                v.push((format!("Give the other{} the next free tag{}", if ops.len() > 1 { "s" } else { "" }, if ops.len() > 1 { "s" } else { "" }), Plan::Ops(ops)));
            }
        }
        2 => {
            if let Some(l) = p.line(first) {
                for (end_b, e) in [(false, &l.a), (true, &l.b)] {
                    if matches!(e, End::Free { .. }) {
                        v.push(("End it at a battery limit".into(), Plan::Limit { line: l.id, end_b }));
                        break;
                    }
                }
            }
        }
        4 | 5 => {
            let key = if rule == 4 { "size" } else { "class" };
            let lines = sides(p, first);
            let vals: Vec<(u32, String)> = lines.iter().map(|l| (*l, text(p, *l, key))).filter(|x| !x.1.is_empty()).collect();
            for (src, val) in &vals {
                let others: Vec<u32> = vals.iter().filter(|x| x.0 != *src && x.1 != *val).map(|x| x.0).collect();
                if !others.is_empty() && !v.iter().any(|x| x.0.ends_with(&format!(" {val}{}", if rule == 4 { "\"" } else { "" }))) {
                    let what = if rule == 4 { format!("{val}\"") } else { val.clone() };
                    v.push((format!("Make both sides {what}"), Plan::Ops(vec![set(others, key, Some(Val::Text(val.clone())))])));
                }
            }
        }
        7 => {
            // A piece flowing against the rest: turn it round (a single piece only).
            if ids.len() == 1 && p.line(first).is_some() {
                let mut l = p.line(first).cloned().unwrap_or_default();
                l.flip = !l.flip;
                v.push(("Turn this piece's flow round".into(), Plan::Ops(vec![Op::SetLine { line: l }])));
            }
        }
        8 => v.push(("Put a check valve in the discharge".into(), Plan::Check { pump: first })),
        9 => {
            v.push(("Place the vessel arrangement (relief valve and the rest)".into(), Plan::Ops(vec![Op::Arrange { ids: vec![first], name: "Vessel".into() }])));
            v.push(("Relief is elsewhere: say so in its Relief path".into(), Plan::Ops(vec![set(vec![first], "relief", Some(Val::Text("See relief study".into())))])));
        }
        10 => v.push(("Place the control valve station (block valves, drain, bypass)".into(), Plan::Ops(vec![Op::Arrange { ids: vec![first], name: "Control valve station".into() }]))),
        13 => {
            // The next common size up.
            let size = text(p, first, "size");
            if let Some(now) = crate::pipes::nps_inches(&size) {
                let next = crate::pipes::sizes().into_iter().map(|s| s.nps).filter(|n| ["1/2", "3/4", "1", "1-1/2", "2", "3", "4", "6", "8", "10", "12", "14", "16", "18", "20", "24", "30"].contains(n)).find(|n| crate::pipes::nps_inches(n).is_some_and(|x| x > now + 1e-9));
                if let Some(n) = next {
                    v.push((format!("Make the line {n}\" (one size up)"), Plan::Ops(vec![set(vec![first], "size", Some(Val::Text(n.into())))])));
                }
            }
        }
        19 => {
            for (f, w) in [("FC", "fail closed"), ("FO", "fail open"), ("FL", "fail last")] {
                v.push((format!("{f} ({w})"), Plan::Ops(vec![set(vec![first], "fail", Some(Val::Text(f.into())))])));
            }
        }
        21 => {
            let class = text(p, first, "class");
            for s in p.settings.specs.iter().take(4) {
                v.push((format!("Use pipe class {}", s.name), Plan::Ops(vec![set(vec![first], "class", Some(Val::Text(s.name.clone())))])));
            }
            if !class.is_empty() {
                let mut st = p.settings.clone();
                st.specs.push(crate::specs::starter(&class));
                v.push((format!("Add a pipe class {class} from the starter carbon-steel class"), Plan::Ops(vec![Op::SetSettings { settings: Box::new(st) }])));
            }
        }
        26 => {
            let d = data(p, first);
            let traced = !crate::insulation::trace(d).is_empty();
            let ins = crate::insulation::code(d);
            if traced && ins.is_empty() {
                v.push(("Insulate it: H (heat conservation), 50 mm".into(), Plan::Ops(vec![set(vec![first], "ins_type", Some(Val::Text("H (heat conservation)".into()))), set(vec![first], "ins_thk", Some(Val::Num(0.05)))])));
            } else if !ins.is_empty() {
                for mm in [25.0, 50.0, 75.0] {
                    v.push((format!("{mm} mm thick"), Plan::Ops(vec![set(vec![first], "ins_thk", Some(Val::Num(mm / 1000.0)))])));
                }
            } else {
                let cold = d.get("oper_t").and_then(|x| x.num()).is_some_and(|t| t <= 273.15);
                let (code, w) = if cold { ("C (cold)", "C (cold), 50 mm") } else { ("H (heat conservation)", "H (heat conservation), 50 mm") };
                v.push((format!("Insulate it: {w}"), Plan::Ops(vec![set(vec![first], "ins_type", Some(Val::Text(code.into()))), set(vec![first], "ins_thk", Some(Val::Num(0.05)))])));
                if !cold {
                    v.push(("Insulate it: PP (personnel protection), 25 mm".into(), Plan::Ops(vec![set(vec![first], "ins_type", Some(Val::Text("PP (personnel protection)".into()))), set(vec![first], "ins_thk", Some(Val::Num(0.025)))])));
                }
                v.push(("No insulation: say N (none)".into(), Plan::Ops(vec![set(vec![first], "ins_type", Some(Val::Text("N (none)".into())))])));
            }
        }
        _ => {}
    }
    v
}

/// The labels of the fixes for a problem.
pub fn labels(p: &Project, rule: u8, ids: &[u32]) -> Vec<String> {
    plans(p, rule, ids).into_iter().map(|x| x.0).collect()
}

/// Applies fix `choice` for a problem.
pub fn apply(p: &mut Project, rule: u8, ids: &[u32], choice: usize, created: &mut Vec<u32>) -> Result<(), String> {
    let (_, plan) = plans(p, rule, ids).into_iter().nth(choice).ok_or("That fix no longer applies: the problem has changed.")?;
    match plan {
        Plan::Ops(ops) => {
            for op in ops {
                exec_op(p, op, created)?;
            }
        }
        Plan::Limit { line, end_b } => {
            let l = p.line(line).cloned().ok_or("That line no longer exists.")?;
            let route = crate::view::routes(p, l.sheet, None).remove(&line).unwrap_or_default();
            let (at, before) = if end_b { (route.last().copied(), route.iter().rev().nth(1).copied()) } else { (route.first().copied(), route.get(1).copied()) };
            let at = at.unwrap_or_else(|| crate::graph::end_pt(p, if end_b { &l.b } else { &l.a }));
            // The limit's inlet faces back along the line.
            let back = before.map_or(2, |b| {
                let (dx, dy) = (b[0] - at[0], b[1] - at[1]);
                if dx.abs() >= dy.abs() { if dx >= 0.0 { 0 } else { 2 } } else if dy >= 0.0 { 1 } else { 3 }
            });
            let s = symbols::get("battery_limit").ok_or("no battery limit symbol")?;
            let rot = (back + 4 - s.ports[0].dir % 4) % 4;
            let pl = symbols::Place { x: 0.0, y: 0.0, rot, flip: false, scale: 1.0 };
            let off = pl.pt([s.ports[0].x, s.ports[0].y]);
            let mut out = vec![];
            exec_op(p, Op::AddItem { item: Item { sheet: l.sheet, sym: "battery_limit".into(), x: at[0] - off[0], y: at[1] - off[1], rot, ..Item::default() }, on: None, tag: String::new(), obj: None }, &mut out)?;
            let bl = out[0];
            created.extend(out);
            let e = End::Port { item: bl, port: 0 };
            let mut nl = l.clone();
            if end_b { nl.b = e } else { nl.a = e }
            exec_op(p, Op::SetLine { line: nl }, created)?;
        }
        Plan::Check { pump } => {
            let it = p.item(pump).cloned().ok_or("That pump no longer exists.")?;
            let s = symbols::get(&it.sym).ok_or("no symbol")?;
            let k = s.ports.iter().position(|q| q.name == "discharge").ok_or("That pump has no discharge nozzle.")? as u8;
            let (line, at_a) = {
                let net = Net::new(p);
                let &(l, is_b) = net.on_port(it.id, k).iter().find(|(l, _)| net.lines.get(l).is_some_and(|x| x.kind.process())).ok_or("Draw the discharge line first.")?;
                (l, !is_b)
            };
            let mut route = crate::view::routes(p, it.sheet, None).remove(&line).unwrap_or_default();
            if !at_a {
                route.reverse();
            }
            if route.len() < 2 {
                return Err("Draw the discharge line first.".into());
            }
            // Half way along the first straight run from the pump, flowing away from it.
            let (a, b) = (route[0], route[1]);
            let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            if len < 10.0 {
                return Err("The discharge line's first run is too short for a valve: lengthen it.".into());
            }
            let t = (len / 2.0).min(12.0) / len;
            let at = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            let dir = if (b[0] - a[0]).abs() >= (b[1] - a[1]).abs() { if b[0] >= a[0] { 0 } else { 2 } } else if b[1] >= a[1] { 1 } else { 3 };
            let mut out = vec![];
            exec_op(p, Op::AddItem { item: Item { sheet: it.sheet, sym: "check".into(), x: at[0], y: at[1], rot: dir, ..Item::default() }, on: Some(line), tag: String::new(), obj: None }, &mut out)?;
            created.extend(out);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;

    fn issues(d: &Doc, rule: u8) -> Vec<crate::checks::Issue> {
        crate::api::derived(&d.p, 0).issues.into_iter().filter(|i| i.rule == rule).collect()
    }

    /// Applies the first fix of the first problem of a rule, and says whether that problem went.
    fn fix(d: &mut Doc, rule: u8, choice: usize) -> bool {
        let i = issues(d, rule).into_iter().next().expect("a problem to fix");
        let before = issues(d, rule).len();
        d.apply(Op::Fix { rule, ids: i.ids.clone(), choice }).unwrap_or_else(|e| panic!("rule {rule}: {e}"));
        issues(d, rule).len() < before
    }

    #[test]
    fn the_faults_example_is_put_right_one_fix_at_a_time() {
        let mut d = Doc::new(crate::examples::get("faults").unwrap());
        let mut fixed = vec![];
        for rule in [1u8, 2, 4, 5, 8, 9, 10, 19] {
            if issues(&d, rule).is_empty() {
                continue;
            }
            fixed.push(rule);
            let i = issues(&d, rule).into_iter().next().unwrap();
            let labels = labels(&d.p, rule, &i.ids);
            assert!(!labels.is_empty(), "rule {rule} offers a fix");
            assert!(fix(&mut d, rule, 0), "rule {rule}: the first fix ({}) removes the problem", labels[0]);
        }
        assert!(fixed.len() >= 5, "the planted faults exercise most fixes: {fixed:?}");
        // Each fix is one undo.
        let n = d.p.items.len();
        d.undo();
        assert!(d.p.items.len() <= n);
        // A stale fix is refused politely.
        assert!(d.apply(Op::Fix { rule: 19, ids: vec![999_999], choice: 0 }).is_err());
        assert!(d.apply(Op::Fix { rule: 8, ids: vec![], choice: 3 }).is_err());
    }

    #[test]
    fn velocity_and_insulation_fixes() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let line = d.p.objects.iter().find(|o| o.class == crate::model::LINE_CLASS && o.data.get("seq").map(|v| v.text()) == Some("1003".into())).unwrap().id;
        d.apply(Op::SetData { ids: vec![line], key: "size".into(), val: Some(Val::Text("1".into())) }).unwrap();
        let i = issues(&d, 13).into_iter().find(|i| i.ids.iter().any(|x| d.p.line(*x).is_some_and(|l| l.obj == line))).expect("too fast in 1\"");
        let l = labels(&d.p, 13, &i.ids);
        assert!(l[0].starts_with("Make the line 1-1/2\""), "{l:?}");
        d.apply(Op::SetData { ids: vec![line], key: "tracing".into(), val: Some(Val::Text("Steam".into())) }).unwrap();
        assert!(fix(&mut d, 26, 0));
        assert_eq!(d.p.data_of(line).get("ins_type").map(|v| v.text()), Some("H (heat conservation)".into()));
    }

    #[test]
    fn every_fix_offered_on_every_example_applies_or_says_why() {
        for ex in ["pump", "compressor", "exchanger", "faults", "pfd"] {
            let p = crate::examples::get(ex).unwrap();
            for i in crate::api::derived(&p, 0).issues.iter().take(40) {
                for k in 0..labels(&p, i.rule, &i.ids).len() + 1 {
                    let mut d = Doc::new(p.clone());
                    let _ = d.apply(Op::Fix { rule: i.rule, ids: i.ids.clone(), choice: k });
                }
            }
        }
    }
}
