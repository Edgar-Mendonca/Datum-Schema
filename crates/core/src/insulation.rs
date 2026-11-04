//! Insulation and heat tracing on lines: the codes used in line numbers and lists, the insulation
//! a line gives its valves and fittings, and check 26 (a traced line that is not insulated, an
//! insulation with no thickness, a hot or cold line with none).

use crate::checks::Issue;
use crate::graph::Net;
use crate::model::{Data, Project};

/// Insulation types (the code is the first word).
pub const TYPES: [&str; 6] = ["H (heat conservation)", "C (cold)", "PP (personnel protection)", "A (acoustic)", "F (fire protection)", "N (none)"];
/// Tracing kinds and their codes.
pub const TRACING: [(&str, &str); 5] = [("Steam", "ST"), ("Electric", "ET"), ("Hot water", "HW"), ("Hot oil", "HO"), ("Jacketed", "J")];

fn text(d: &Data, k: &str) -> String {
    d.get(k).map(|v| v.text().trim().to_string()).unwrap_or_default()
}

/// The insulation code of a line ("H", "PP"…; "" for none).
pub fn code(d: &Data) -> String {
    let t = text(d, "ins_type");
    let c = t.split_whitespace().next().unwrap_or("").to_uppercase();
    if c == "N" || c == "NONE" { String::new() } else { c }
}

/// The insulation as a line number shows it: the code and the thickness in millimetres ("H50").
pub fn tag(d: &Data) -> String {
    let c = code(d);
    if c.is_empty() {
        return String::new();
    }
    match d.get("ins_thk").and_then(|v| v.num()).filter(|x| x.is_finite() && *x > 0.0) {
        Some(t) => format!("{c}{}", crate::fmt_num((t * 1000.0).round())),
        None => c,
    }
}

/// The tracing code of a line ("ST", "ET"…; "" for none).
pub fn trace(d: &Data) -> String {
    let t = text(d, "tracing");
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return String::new();
    }
    TRACING.iter().find(|x| x.0.eq_ignore_ascii_case(&t) || x.1.eq_ignore_ascii_case(&t)).map_or_else(|| t.chars().take(4).collect::<String>().to_uppercase(), |x| x.1.to_string())
}

/// Check 26: insulation and tracing that do not add up.
pub fn checks(p: &Project, net: &Net) -> Vec<Issue> {
    let mut v = vec![];
    let pfd: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    for (r, lines) in net.runs.iter().enumerate() {
        let Some(&l0) = lines.first() else { continue };
        let Some(line) = net.lines.get(&l0) else { continue };
        if pfd.contains(&line.sheet) || !line.kind.process() || crate::graph::instrument_connection(p, line) {
            continue;
        }
        let obj = net.run_obj[r];
        if obj == 0 {
            continue;
        }
        let d = p.data_of(obj);
        let n = net.numbers.get(&l0).cloned().filter(|x| !x.is_empty()).map_or("A line".to_string(), |x| format!("Line {x}"));
        let at = crate::graph::end_pt(p, &line.a);
        let mut push = |level: &'static str, msg: String| v.push(Issue { rule: 26, level, msg, ids: lines.clone(), sheet: line.sheet, at });
        let (ins, tr) = (code(d), trace(d));
        if !tr.is_empty() && ins.is_empty() {
            push("warning", format!("{n} is traced ({}) but not insulated: tracing needs insulation over it.", text(d, "tracing")));
        }
        if !ins.is_empty() && d.get("ins_thk").and_then(|x| x.num()).is_none_or(|t| t.is_nan() || t <= 0.0) {
            push("note", format!("{n} is insulated ({ins}) but has no insulation thickness."));
        }
        if ins.is_empty() && tr.is_empty() {
            if let Some(t) = d.get("oper_t").and_then(|x| x.num()).filter(|x| x.is_finite()) {
                if t >= 333.15 {
                    push("note", format!("{n} runs at {}: insulate it for heat conservation or personnel protection, or say N (none).", crate::units::show(t, "temp", p.units == "US")));
                } else if t <= 273.15 {
                    push("note", format!("{n} runs at {}: cold insulation against condensation and icing, or say N (none).", crate::units::show(t, "temp", p.units == "US")));
                }
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::Val;

    #[test]
    fn codes_and_checks() {
        let mut d = Data::new();
        assert_eq!(tag(&d), "");
        d.insert("ins_type".into(), Val::Text("H (heat conservation)".into()));
        assert_eq!(tag(&d), "H");
        d.insert("ins_thk".into(), Val::Num(0.05));
        assert_eq!(tag(&d), "H50");
        d.insert("tracing".into(), Val::Text("Steam".into()));
        assert_eq!(trace(&d), "ST");
        d.insert("ins_type".into(), Val::Text("N (none)".into()));
        assert_eq!(code(&d), "");

        let mut doc = Doc::new(crate::examples::get("pump").unwrap());
        let line = doc.p.objects.iter().find(|o| o.class == crate::model::LINE_CLASS && o.data.get("seq").map(|v| v.text()) == Some("1003".into())).unwrap().id;
        let issues = |doc: &Doc| crate::api::derived(&doc.p, 0).issues.into_iter().filter(|i| i.rule == 26).map(|i| (i.level, i.msg)).collect::<Vec<_>>();
        assert!(issues(&doc).is_empty(), "{:?}", issues(&doc));
        // Traced, not insulated: a warning; then insulated without a thickness: a note; then clean.
        doc.apply(Op::SetData { ids: vec![line], key: "tracing".into(), val: Some(Val::Text("Steam".into())) }).unwrap();
        assert!(issues(&doc).iter().any(|i| i.0 == "warning" && i.1.contains("traced (Steam) but not insulated")));
        doc.apply(Op::SetData { ids: vec![line], key: "ins_type".into(), val: Some(Val::Text("H (heat conservation)".into())) }).unwrap();
        assert!(issues(&doc).iter().all(|i| i.0 == "note") && issues(&doc).iter().any(|i| i.1.contains("no insulation thickness")));
        doc.apply(Op::SetData { ids: vec![line], key: "ins_thk".into(), val: Some(Val::Num(0.04)) }).unwrap();
        assert!(issues(&doc).is_empty());
        // The line number can show it, and the valves in the line take it.
        let mut s = doc.p.settings.clone();
        s.line_format = "<size>-<fluid>-<seq>-<class>-<ins>".into();
        doc.apply(Op::SetSettings { settings: Box::new(s) }).unwrap();
        assert_eq!(crate::graph::obj_number(&doc.p, line), "3\"-P-1003-A1A-H40");
        let hv = doc.p.objects.iter().find(|o| o.tag == "HV-104").unwrap();
        assert_eq!(hv.data.get("insulation").map(|v| v.text()), Some("H40".into()));
        // A hot line with no insulation is noted.
        doc.apply(Op::SetData { ids: vec![line], key: "ins_type".into(), val: None }).unwrap();
        doc.apply(Op::SetData { ids: vec![line], key: "tracing".into(), val: None }).unwrap();
        doc.apply(Op::SetData { ids: vec![line], key: "oper_t".into(), val: Some(Val::Num(373.15)) }).unwrap();
        assert!(issues(&doc).iter().any(|i| i.1.contains("runs at 100 °C")), "{:?}", issues(&doc));
    }
}
