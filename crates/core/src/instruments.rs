//! Instrument checks and reports: a range that covers the process (check 18), control valves
//! with a fail position (19), loop wiring finished once started (20), and the I/O summary.

use crate::checks::Issue;
use crate::graph::Net;
use crate::model::{Item, Project};
use crate::symbols::{self, Cat};
use crate::{tags, units};

/// A range typed as text ("0–10 bar", "-50 to 150 °C", "0-60 m3/h") in SI for a quantity.
/// A range without a unit is in the project's display unit. None if it cannot be read or its
/// unit is not of that quantity.
pub fn parse_range(text: &str, q: &str, us: bool) -> Option<(f64, f64)> {
    let t = text.trim().replace(['–', '—'], "~").replace(" to ", "~").replace("...", "~");
    // The unit starts at the first letter, degree or percent sign (an exponent's e excepted).
    let cs: Vec<(usize, char)> = t.char_indices().collect();
    let start = cs.iter().enumerate().find(|(k, (_, c))| (c.is_alphabetic() && !(matches!(c, 'e' | 'E') && *k > 0 && cs[k - 1].1.is_ascii_digit())) || matches!(c, '°' | '%')).map_or(t.len(), |(_, (i, _))| *i);
    let (nums, unit) = (t[..start].trim(), t[start..].trim());
    let (a, b) = match nums.split_once('~') {
        Some((a, b)) => (a.trim(), b.trim()),
        None => {
            // "0-10": the dash after the first number (a leading minus belongs to it).
            let i = nums.char_indices().skip(1).find(|(i, c)| *c == '-' && !nums[..*i].ends_with(['e', 'E'])).map(|x| x.0)?;
            (nums[..i].trim(), nums[i + 1..].trim())
        }
    };
    let (lo, hi): (f64, f64) = (a.parse().ok()?, b.parse().ok()?);
    let shown = units::shown(us).into_iter().find(|s| s.q == q)?;
    let to_si = |v: f64| -> Option<f64> {
        if unit.is_empty() {
            return Some((v + shown.offset) * shown.factor);
        }
        let norm = |u: &str| u.replace("m3", "m^3").replace('³', "^3").replace('²', "^2");
        let (f, dim, off) = units::expr(&norm(unit)).ok()?;
        let (_, want, _) = units::expr(&norm(&shown.label)).ok()?;
        units::same(&dim, &want).then_some(f * v + off)
    };
    let (lo, hi) = (to_si(lo)?, to_si(hi)?);
    (lo.is_finite() && hi.is_finite() && hi > lo).then_some((lo, hi))
}

/// The process value an instrument measures, from its line: (quantity, SI value, the line).
fn process_value(p: &Project, net: &Net, it: &Item, letters: &str) -> Option<(&'static str, f64, String)> {
    let (q, keys): (&str, &[&str]) = match letters.chars().next()? {
        'P' => ("press", &["oper_p"]),
        'T' => ("temp", &["oper_t"]),
        'F' => ("flow", &["flow"]),
        _ => return None,
    };
    let (o, what) = crate::datasheets::process_source(p, net, it)?;
    let d = p.data_of(o);
    let v = keys.iter().find_map(|k| d.get(*k).and_then(|v| v.num()))?;
    Some((q, v, what))
}

pub fn checks(p: &Project, net: &Net, on: &dyn Fn(u8) -> bool) -> Vec<Issue> {
    let us = p.units == "US";
    let pfd: std::collections::HashSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    let mut v = vec![];
    let issue = |rule: u8, level: &'static str, it: &Item, msg: String| Issue { rule, level, msg, ids: vec![it.id], sheet: it.sheet, at: [it.x, it.y] };
    let mut seen = std::collections::HashSet::new();
    for it in p.items.iter().filter(|i| !pfd.contains(&i.sheet)) {
        if it.obj != 0 && !seen.insert(it.obj) {
            continue;
        }
        let Some(s) = symbols::get(&it.sym) else { continue };
        let tag = p.tag_of(it);
        // 18. The range covers the operating value.
        if on(18) && s.cat == Cat::Instrument && !crate::loops::in_system(p, it) {
            let (letters, _) = tags::loop_of(tag);
            let range = p.data_of(it.obj).get("range").map(|r| r.text()).unwrap_or_default();
            if tags::is_measurement(&letters) && !range.trim().is_empty() {
                if let Some((q, x, what)) = process_value(p, net, it, &letters) {
                    if let Some((lo, hi)) = parse_range(&range, q, us) {
                        if x < lo || x > hi {
                            v.push(issue(18, "warning", it, format!("The range of {tag} ({}) does not cover the operating {} of {what}, {}.", range.trim(), match q { "press" => "pressure", "temp" => "temperature", _ => "flow" }, units::show(x, q, us))));
                        }
                    }
                }
            }
        }
        // 19. A control valve has a fail position.
        if on(19) && matches!(it.base_key().as_str(), "cv_pneu" | "cv_motor") && p.data_of(it.obj).get("fail").is_none_or(|f| f.text().trim().is_empty()) {
            v.push(issue(19, "warning", it, format!("Control valve {} has no fail position (FO, FC or FL).", if tag.trim().is_empty() { "without a tag" } else { tag })));
        }
    }
    // 20. Once a loop's wiring is started, every device in it is wired.
    if on(20) {
        let wired = |it: &Item, k: &str| p.data_of(it.obj).get(k).is_some_and(|v| !v.text().trim().is_empty());
        // Only the loops where some wiring has been typed.
        let mut keys: Vec<String> = p.objects.iter().filter(|o| crate::loops::WIRING.iter().any(|(k, _)| o.data.get(*k).is_some_and(|v| !v.text().trim().is_empty()))).filter_map(|o| crate::loops::key_of(&o.tag)).collect();
        keys.sort();
        keys.dedup();
        for key in keys {
            let ms = crate::loops::members(p, &key);
            if ms.len() < 2 {
                continue;
            }
            let l = crate::loops::LoopInfo { key: key.clone(), tags: vec![] };
            for it in ms.iter().filter(|i| !pfd.contains(&i.sheet)) {
                let need: Vec<&str> = if crate::loops::in_system(p, it) { vec!["system"] } else { crate::loops::WIRING.iter().map(|x| x.0).collect() };
                let missing: Vec<String> = crate::loops::WIRING.iter().filter(|(k, _)| need.contains(k) && !wired(it, k)).map(|(_, l)| l.to_lowercase()).collect();
                if !missing.is_empty() {
                    v.push(issue(20, "note", it, format!("Loop {}: {} has no {}.", l.key, p.tag_of(it), missing.join(", "))));
                }
            }
        }
    }
    v
}

/// The I/O kinds of the summary.
pub const IO: [&str; 5] = ["AI", "AO", "DI", "DO", "Fieldbus"];

/// The I/O an instrument takes in its control system, if any: from its I/O channel ("AI 01-03"),
/// else its signal, else its letters and symbol.
pub fn io_of(p: &Project, it: &Item) -> Option<&'static str> {
    let s = symbols::get(&it.sym)?;
    if !(s.cat.instrument() || s.cat == Cat::Control) || crate::loops::in_system(p, it) {
        return None;
    }
    let d = p.data_of(it.obj);
    let get = |k: &str| d.get(k).map(|v| v.text().trim().to_uppercase()).unwrap_or_default();
    let ch = get("io");
    if let Some(k) = IO.iter().take(4).find(|k| ch.starts_with(*k)) {
        return Some(k);
    }
    let base = it.base_key();
    let (letters, _) = tags::loop_of(p.tag_of(it));
    let output = s.cat == Cat::Control || tags::is_final(&letters);
    let switch = letters.chars().nth(1) == Some('S');
    let signal = get("signal");
    if signal.starts_with("FIELDBUS") {
        return Some("Fieldbus");
    }
    if signal.starts_with("PNEUMATIC") || signal.starts_with("CAPILLARY") || signal.starts_with("SOFTWARE") {
        return None;
    }
    let discrete = signal.starts_with("DISCRETE") || switch || base == "cv_solenoid";
    if signal.starts_with("4") || discrete {
        return Some(match (output, discrete) { (false, false) => "AI", (true, false) => "AO", (false, true) => "DI", (true, true) => "DO" });
    }
    // No signal typed: transmitters are analogue inputs, control valves analogue outputs.
    if matches!(base.as_str(), "cv_pneu" | "cv_motor") {
        return Some("AO");
    }
    (letters.ends_with('T') && !switch).then_some("AI")
}

/// The I/O summary: per control system, the count of each I/O kind, the total and the total with
/// 20% spare. Returns (system, counts, items).
pub fn io_summary(p: &Project) -> Vec<(String, [usize; 5], Vec<u32>)> {
    let mut m: std::collections::BTreeMap<String, ([usize; 5], Vec<u32>)> = std::collections::BTreeMap::new();
    let mut seen = std::collections::HashSet::new();
    for it in &p.items {
        if it.obj != 0 && !seen.insert(it.obj) {
            continue;
        }
        let Some(k) = io_of(p, it) else { continue };
        let sys = p.data_of(it.obj).get("system").map(|v| v.text().trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "(not assigned)".into());
        let e = m.entry(sys).or_default();
        e.0[IO.iter().position(|x| *x == k).unwrap_or(0)] += 1;
        e.1.push(it.id);
    }
    m.into_iter().map(|(k, (c, ids))| (k, c, ids)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::Val;

    #[test]
    fn ranges_are_read_in_their_units() {
        let near = |a: Option<(f64, f64)>, b: (f64, f64)| a.is_some_and(|a| (a.0 - b.0).abs() < 1e-6 * b.1.abs().max(1.0) && (a.1 - b.1).abs() < 1e-6 * b.1.abs().max(1.0));
        assert!(near(parse_range("0–10 bar", "press", false), (0.0, 1e6)));
        assert!(near(parse_range("0 to 150 psi", "press", false), (0.0, 150.0 * 6894.757293168)));
        assert!(near(parse_range("-50 to 150 °C", "temp", false), (223.15, 423.15)));
        assert!(near(parse_range("0-60 m3/h", "flow", false), (0.0, 60.0 / 3600.0)));
        assert!(near(parse_range("-1-5", "press", false), (-1e5, 5e5)));
        assert!(near(parse_range("0–200", "temp", true), ((0.0 + 459.67) * 5.0 / 9.0, (200.0 + 459.67) * 5.0 / 9.0)));
        // Not a pressure, not a range, or the wrong way round.
        assert!(parse_range("0–10 m", "press", false).is_none());
        assert!(parse_range("full scale", "press", false).is_none());
        assert!(parse_range("10–0 bar", "press", false).is_none());
        assert!(parse_range("0–100 %", "temp", false).is_none());
    }

    #[test]
    fn instrument_checks_pass_and_fail() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let all = |d: &Doc| {
            let net = Net::new(&d.p);
            checks(&d.p, &net, &|_| true)
        };
        assert!(all(&d).iter().all(|i| i.level == "note" || !matches!(i.rule, 18 | 19)), "{:?}", all(&d));
        // 18: a pressure gauge or transmitter on a line operating above its range.
        let pi = d.p.items.iter().find(|i| crate::tags::loop_of(d.p.tag_of(i)).0.starts_with('P') && crate::tags::is_measurement(&crate::tags::loop_of(d.p.tag_of(i)).0)).expect("a pressure instrument").id;
        d.apply(Op::SetData { ids: vec![pi], key: "range".into(), val: Some(Val::Text("0–2 bar".into())) }).unwrap();
        assert!(all(&d).iter().any(|i| i.rule == 18 && i.msg.contains("does not cover the operating pressure")), "{:?}", all(&d));
        d.apply(Op::SetData { ids: vec![pi], key: "range".into(), val: Some(Val::Text("0–16 bar".into())) }).unwrap();
        assert!(!all(&d).iter().any(|i| i.rule == 18));
        // 19: a control valve without a fail position.
        let lv = d.p.items.iter().find(|i| d.p.tag_of(i) == "LV-101").unwrap().id;
        d.apply(Op::SetData { ids: vec![lv], key: "fail".into(), val: None }).unwrap();
        assert!(all(&d).iter().any(|i| i.rule == 19 && i.msg.contains("LV-101")));
        d.apply(Op::SetData { ids: vec![lv], key: "fail".into(), val: Some(Val::Text("FC".into())) }).unwrap();
        assert!(!all(&d).iter().any(|i| i.rule == 19));
        // 20: wiring started on the transmitter only.
        let lt = d.p.items.iter().find(|i| d.p.tag_of(i) == "LT-101").unwrap().id;
        for (k, val) in [("jb", "JB-1"), ("jb_terminals", "1, 2"), ("cable", "C-1"), ("system", "DCS 1"), ("io", "AI 01-01")] {
            d.apply(Op::SetData { ids: vec![lt], key: k.into(), val: Some(Val::Text(val.into())) }).unwrap();
        }
        let w: Vec<String> = all(&d).into_iter().filter(|i| i.rule == 20).map(|i| i.msg).collect();
        assert!(w.iter().any(|m| m.contains("LV-101") && m.contains("cable")), "{w:?}");
        assert!(w.iter().any(|m| m.contains("LIC-101") && m.contains("control system")), "{w:?}");
        assert!(!w.iter().any(|m| m.contains("LT-101")));
        // The I/O summary: the transmitter in DCS 1, the valve not assigned yet.
        let io = io_summary(&d.p);
        let dcs = io.iter().find(|x| x.0 == "DCS 1").unwrap();
        assert_eq!(dcs.1[0], 1);
        assert!(io.iter().any(|x| x.0 == "(not assigned)" && x.1[1] >= 1), "{io:?}");
    }
}
