//! The company's own rules (check 24): a condition on one attribute of every object of a class
//! (and its subclasses), with a level and a message of the company's choosing.

use crate::checks::Issue;
use crate::model::Project;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UserRule {
    pub on: bool,
    pub class: String,
    pub key: String,
    /// "required", "equals", "not equals", "one of", "contains", "at least", "at most".
    pub test: String,
    /// The text to compare with ("one of": separated by commas).
    pub value: String,
    /// The number to compare with ("at least", "at most"), in SI.
    pub num: f64,
    /// "error", "warning" or "note".
    pub level: String,
    pub message: String,
}

impl Default for UserRule {
    fn default() -> Self {
        Self { on: true, class: String::new(), key: String::new(), test: "required".into(), value: String::new(), num: 0.0, level: "warning".into(), message: String::new() }
    }
}

pub const TESTS: [&str; 7] = ["required", "equals", "not equals", "one of", "contains", "at least", "at most"];

pub fn normalize(v: &mut Vec<UserRule>) {
    v.truncate(200);
    v.retain(|r| !r.class.trim().is_empty() && !r.key.trim().is_empty());
    for r in v.iter_mut() {
        r.class = crate::clip(r.class.trim(), 80);
        r.key = crate::clip(r.key.trim(), 64);
        if !TESTS.contains(&r.test.as_str()) {
            r.test = "required".into();
        }
        if !["error", "warning", "note"].contains(&r.level.as_str()) {
            r.level = "warning".into();
        }
        r.value = crate::clip(&r.value, 400);
        r.message = crate::clip(&r.message, 300);
        if !r.num.is_finite() {
            r.num = 0.0;
        }
    }
}

/// Whether a value passes a rule (None: the attribute is not set).
fn passes(r: &UserRule, v: Option<&crate::model::Val>) -> bool {
    let text = v.map(|x| x.text().trim().to_string()).unwrap_or_default();
    match r.test.as_str() {
        "required" => !text.is_empty(),
        // The other tests say nothing about an attribute that is not set ("required" does).
        _ if text.is_empty() => true,
        "equals" => text.eq_ignore_ascii_case(r.value.trim()),
        "not equals" => !text.eq_ignore_ascii_case(r.value.trim()),
        "one of" => r.value.split(',').any(|x| x.trim().eq_ignore_ascii_case(&text)),
        "contains" => text.to_lowercase().contains(&r.value.trim().to_lowercase()),
        "at least" => v.and_then(|x| x.num()).is_none_or(|x| x >= r.num - 1e-9 * r.num.abs().max(1.0)),
        "at most" => v.and_then(|x| x.num()).is_none_or(|x| x <= r.num + 1e-9 * r.num.abs().max(1.0)),
        _ => true,
    }
}

/// The words of a rule, for its message when the company wrote none.
fn says(p: &Project, r: &UserRule, label: &str, q: &str) -> String {
    let n = || if q.is_empty() { crate::fmt_num(r.num) } else { crate::units::show(r.num, q, p.units == "US") };
    match r.test.as_str() {
        "required" => format!("needs its {label}"),
        "equals" => format!("{label} must be {}", r.value.trim()),
        "not equals" => format!("{label} must not be {}", r.value.trim()),
        "one of" => format!("{label} must be one of {}", r.value.trim()),
        "contains" => format!("{label} must contain {}", r.value.trim()),
        "at least" => format!("{label} must be at least {}", n()),
        _ => format!("{label} must be at most {}", n()),
    }
}

/// Check 24: every rule that is on, on every object of its class that is drawn.
pub fn checks(p: &Project, net: &crate::graph::Net) -> Vec<Issue> {
    let mut v = vec![];
    if p.settings.rules.iter().all(|r| !r.on) {
        return v;
    }
    let level = |l: &str| -> &'static str { match l { "error" => "error", "note" => "note", _ => "warning" } };
    // Where each object is drawn: its first symbol, or its line's first piece.
    let mut at: std::collections::HashMap<u32, (Vec<u32>, u32, [f64; 2])> = std::collections::HashMap::new();
    for it in &p.items {
        at.entry(it.obj).or_insert((vec![it.id], it.sheet, [it.x, it.y]));
    }
    for (r, lines) in net.runs.iter().enumerate() {
        let l = net.lines[&lines[0]];
        at.entry(net.run_obj[r]).or_insert((lines.clone(), l.sheet, crate::graph::end_pt(p, &l.a)));
    }
    for o in &p.objects {
        let Some((ids, sheet, pt)) = at.get(&o.id) else { continue };
        let chain: Vec<String> = crate::classes::chain(&o.class).iter().map(|c| c.name.clone()).collect();
        let attrs = crate::classes::attrs(Some(p), &o.class);
        for r in p.settings.rules.iter().filter(|r| r.on && (r.class == o.class || chain.contains(&r.class))) {
            if passes(r, o.data.get(&r.key)) {
                continue;
            }
            let a = attrs.iter().find(|a| a.key == r.key);
            let label = a.map_or(r.key.clone(), |a| a.label.to_lowercase());
            let who = if o.tag.trim().is_empty() { crate::graph::obj_number(p, o.id) } else { o.tag.clone() };
            let who = if who.trim().is_empty() { crate::classes::get(&o.class).map_or(o.class.clone(), |c| format!("A {}", c.label.to_lowercase())) } else { who };
            let msg = if r.message.trim().is_empty() { format!("{who}: {}.", says(p, r, &label, a.map_or("", |a| a.q.as_str()))) } else { format!("{who}: {}", r.message.trim()) };
            v.push(Issue { rule: 24, level: level(&r.level), msg, ids: ids.clone(), sheet: *sheet, at: *pt });
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::Val;

    fn r24(p: &Project) -> Vec<Issue> {
        checks(p, &crate::graph::Net::new(p))
    }

    #[test]
    fn company_rules_pass_and_fail() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let mut st = d.p.settings.clone();
        st.rules = vec![
            UserRule { class: "Equipment".into(), key: "material".into(), ..UserRule::default() },
            UserRule { class: "Pump".into(), key: "head".into(), test: "at most".into(), num: 50.0, level: "error".into(), ..UserRule::default() },
            UserRule { class: "ControlValve".into(), key: "fail".into(), test: "one of".into(), value: "FC, FO".into(), message: "control valves fail open or closed here".into(), ..UserRule::default() },
            UserRule { on: false, class: "PlantItem".into(), key: "service".into(), ..UserRule::default() },
        ];
        d.apply(Op::SetSettings { settings: Box::new(st) }).unwrap();
        let w = r24(&d.p);
        // Pumps are equipment: the ones without a material are listed; P-101A's head is fine.
        assert!(!w.iter().any(|i| i.msg.contains("head")), "{w:#?}");
        let pa = d.p.items.iter().find(|i| d.p.tag_of(i) == "P-101A").unwrap().id;
        d.apply(Op::SetData { ids: vec![pa], key: "head".into(), val: Some(Val::Num(65.0)) }).unwrap();
        let w = r24(&d.p);
        let h = w.iter().find(|i| i.msg.starts_with("P-101A") && i.msg.contains("head")).unwrap();
        assert_eq!(h.level, "error");
        assert!(h.msg.contains("at most 50 m"), "{}", h.msg);
        // A fail position outside the list, with the company's own words.
        let lv = d.p.items.iter().find(|i| d.p.tag_of(i) == "LV-101").unwrap().id;
        d.apply(Op::SetData { ids: vec![lv], key: "fail".into(), val: Some(Val::Text("FL".into())) }).unwrap();
        assert!(r24(&d.p).iter().any(|i| i.msg == "LV-101: control valves fail open or closed here"));
        // A rule switched off says nothing; with no rules, nothing at all.
        assert!(!r24(&d.p).iter().any(|i| i.msg.contains("service")));
        let mut st = d.p.settings.clone();
        st.rules.clear();
        d.apply(Op::SetSettings { settings: Box::new(st) }).unwrap();
        assert!(r24(&d.p).is_empty());
    }

    #[test]
    fn tags_follow_the_company_format() {
        let mut d = Doc::new(Project::new());
        let mut st = d.p.settings.clone();
        st.tag_format = crate::tags::TagFormat { sep: "-".into(), unit: "10".into(), numbering: "project".into(), digits: 3 };
        d.apply(Op::SetSettings { settings: Box::new(st) }).unwrap();
        let s = d.p.sheets[0].id;
        let put = |d: &mut Doc, sym: &str, tag: &str| { let id = d.apply(Op::AddItem { item: crate::model::Item { sheet: s, sym: sym.into(), x: 50.0, y: 50.0, ..Default::default() }, on: None, tag: tag.into(), obj: None }).unwrap()[0]; d.p.tag_of(d.p.item(id).unwrap()).to_string() };
        assert_eq!(put(&mut d, "pump_cent", ""), "P-10001");
        assert_eq!(put(&mut d, "pump_cent", ""), "P-10002");
        // Instruments take the next loop number in the same form, and loops still group.
        assert_eq!(put(&mut d, "inst_field", "LT"), "LT-10001");
        assert_eq!(crate::tags::loop_of("LT-10001").1, crate::tags::loop_of("LIC-10001").1);
        // A copy keeps the width.
        assert_eq!(crate::tags::bump(&d.p, "V-007", &Default::default()), "V-008");
        // Renumbering follows the format.
        let r = crate::tags::renumber(&d.p);
        assert!(r.iter().all(|(_, t)| t.contains("-10")), "{r:?}");
        // Hostile settings are made sound.
        let mut f = crate::tags::TagFormat { sep: "<b>".into(), unit: "1a2b3c4d5".into(), numbering: "x".into(), digits: 99 };
        f.normalize();
        assert_eq!(f, crate::tags::TagFormat { sep: "-".into(), unit: "1234".into(), numbering: "sheet".into(), digits: 6 });
    }
}
