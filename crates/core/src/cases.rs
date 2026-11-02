//! Process cases (normal, maximum, minimum, start-up…): one set of process values per case on every
//! stream, line and piece of equipment. The values on the drawing are always the active case's, so
//! the hydraulics, sizing, lists, checks and datasheets follow the case chosen; the other cases'
//! values are kept beside them ("~case:<case>:<key>") and swapped in when the case changes. A case
//! never set on an object starts as a copy of the case before it.

use crate::model::{Project, Val};

/// The process values that differ between cases.
pub const KEYS: [&str; 16] = ["flow", "mflow", "oper_p", "oper_t", "density", "viscosity", "press", "temp", "vfrac", "hflow", "head", "power", "duty", "dp_design", "relieving_rate", "relieving_t"];

pub fn default_cases() -> Vec<String> {
    ["Normal", "Maximum", "Minimum"].iter().map(|s| s.to_string()).collect()
}

fn key(case: &str, k: &str) -> String {
    format!("~case:{case}:{k}")
}

pub fn normalize(cases: &mut Vec<String>, active: &mut String) {
    cases.truncate(12);
    let mut seen = std::collections::HashSet::new();
    cases.retain(|c| !c.trim().is_empty() && !c.contains(':') && seen.insert(c.trim().to_lowercase()));
    for c in cases.iter_mut() {
        *c = crate::clip(c.trim(), 30);
    }
    if cases.is_empty() {
        *cases = default_cases();
    }
    if !cases.contains(active) {
        *active = cases[0].clone();
    }
}

/// Makes `case` the active one: every object's process values for the case now active are kept
/// aside, and those of `case` come in (where it has none yet, the values stay as they are).
pub fn switch(p: &mut Project, case: &str) -> Result<(), String> {
    let case = p.settings.cases.iter().find(|c| c.eq_ignore_ascii_case(case.trim())).cloned().ok_or_else(|| format!("There is no case called {case}."))?;
    let old = p.settings.case.clone();
    if case == old {
        return Ok(());
    }
    for o in p.objects.iter_mut() {
        let had_new = KEYS.iter().any(|k| o.data.contains_key(&key(&case, k)));
        for k in KEYS {
            // Keep the active case's value (or that it had none, when the object has any of the keys).
            match o.data.get(k).cloned() {
                Some(v) => {
                    o.data.insert(key(&old, k), v);
                }
                None => {
                    o.data.remove(&key(&old, k));
                }
            }
            if had_new {
                match o.data.remove(&key(&case, k)) {
                    Some(v) => {
                        o.data.insert(k.to_string(), v);
                    }
                    None => {
                        o.data.remove(k);
                    }
                }
            }
        }
    }
    p.settings.case = case;
    Ok(())
}

/// An object's value of `k` in another case (None: not set there).
pub fn value<'a>(p: &'a Project, obj: u32, case: &str, k: &str) -> Option<&'a Val> {
    if case == p.settings.case {
        return p.data_of(obj).get(k);
    }
    p.data_of(obj).get(&key(case, k))
}

/// Renames or removes cases (the values kept for a removed case go with it).
pub fn set_cases(p: &mut Project, cases: Vec<String>) -> Result<(), String> {
    let mut cases = cases;
    let mut active = p.settings.case.clone();
    normalize(&mut cases, &mut active);
    if active != p.settings.case {
        // The active case was removed: its values stay as the new active case's.
        let gone = p.settings.case.clone();
        p.settings.case = active.clone();
        for o in p.objects.iter_mut() {
            for k in KEYS {
                o.data.remove(&key(&active, k));
                o.data.remove(&key(&gone, k));
            }
        }
    }
    let keep: std::collections::HashSet<String> = cases.iter().cloned().collect();
    for o in p.objects.iter_mut() {
        o.data.retain(|k, _| !k.starts_with("~case:") || k.split(':').nth(1).is_some_and(|c| keep.contains(c)));
    }
    p.settings.cases = cases;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    #[test]
    fn each_case_keeps_its_own_values_and_everything_follows_the_active_one() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        assert_eq!(d.p.settings.case, "Normal");
        let line = d.p.objects.iter().find(|o| o.class == crate::model::LINE_CLASS && o.data.get("seq").map(|v| v.text()) == Some("1003".into())).unwrap().id;
        let flow = |d: &Doc| d.p.data_of(line).get("flow").and_then(|v| v.num()).unwrap();
        let normal = flow(&d);
        let v_normal = crate::hydraulics::all(&d.p, &crate::graph::Net::new(&d.p)).into_iter().flatten().map(|h| h.v).fold(0.0, f64::max);
        // The maximum case starts as a copy, then gets 50 % more flow.
        d.apply(Op::SetCase { case: "Maximum".into() }).unwrap();
        assert_eq!(flow(&d), normal);
        d.apply(Op::SetData { ids: vec![line], key: "flow".into(), val: Some(Val::Num(normal * 1.5)) }).unwrap();
        let v_max = crate::hydraulics::all(&d.p, &crate::graph::Net::new(&d.p)).into_iter().flatten().map(|h| h.v).fold(0.0, f64::max);
        assert!(v_max > v_normal * 1.4, "the hydraulics follow the case: {v_normal} → {v_max}");
        // Back to normal: its own flow; the maximum kept aside.
        d.apply(Op::SetCase { case: "Normal".into() }).unwrap();
        assert_eq!(flow(&d), normal);
        assert_eq!(value(&d.p, line, "Maximum", "flow").and_then(|v| v.num()), Some(normal * 1.5));
        // And forward again.
        d.apply(Op::SetCase { case: "maximum".into() }).unwrap();
        assert_eq!(flow(&d), normal * 1.5);
        // The case is one entry in the history, and one undo.
        d.undo();
        assert_eq!((d.p.settings.case.as_str(), flow(&d)), ("Normal", normal));
        assert!(d.apply(Op::SetCase { case: "Nonsense".into() }).is_err());
        // Cases removed take their values with them; hostile names are cleaned.
        d.apply(Op::SetCases { cases: vec!["Normal".into(), " ".into(), "a:b".into(), "Start-up".into(), "normal".into()] }).unwrap();
        assert_eq!(d.p.settings.cases, ["Normal", "Start-up"]);
        assert!(value(&d.p, line, "Maximum", "flow").is_none());
    }

    #[test]
    fn the_datasheet_shows_the_other_cases() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let ft = d.p.objects.iter().find(|o| o.tag == "LV-101").unwrap().id;
        let line = crate::datasheets::process_source(&d.p, &crate::graph::Net::new(&d.p), d.p.items.iter().find(|i| i.obj == ft).unwrap()).unwrap().0;
        d.apply(Op::SetCase { case: "Maximum".into() }).unwrap();
        d.apply(Op::SetData { ids: vec![line], key: "flow".into(), val: Some(Val::Num(45.0 / 3600.0)) }).unwrap();
        d.apply(Op::SetCase { case: "Normal".into() }).unwrap();
        let ds = crate::datasheets::build(&d.p, ft).unwrap();
        let pc = ds.sections.iter().find(|s| s.title == "Process conditions").unwrap();
        assert!(pc.rows.iter().any(|r| r.label == "Flow (Maximum)" && r.value == "45"), "{:#?}", pc.rows.iter().map(|r| (&r.label, &r.value)).collect::<Vec<_>>());
    }
}
