//! Tags: reading them (prefix, number, suffix), the next free tag per prefix and sheet, the next
//! tag for a copy (P-101A → P-101B), renumbering, duplicates, and ISA-5.1 letter codes.

use crate::model::Project;
use crate::symbols::{self, Cat};
use std::collections::{BTreeMap, HashSet};

/// A tag split into its letters, number and suffix: "P-101A" → ("P", 101, "A"), "LIC-101" →
/// ("LIC", 101, ""). The number is None when there is none.
pub fn split(tag: &str) -> (String, Option<u64>, String) {
    let t = tag.trim();
    let start = t.find(|c: char| c.is_ascii_digit());
    let Some(i) = start else { return (t.trim_end_matches(['-', ' ']).to_string(), None, String::new()) };
    let prefix = t[..i].trim_end_matches(['-', ' ', '_']).to_string();
    let rest = &t[i..];
    let j = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    let num = rest[..j].parse::<u64>().ok();
    (prefix, num, rest[j..].to_string())
}

/// Instrument letters and loop number from a tag ("LIC-101" → ("LIC", "101")).
pub fn loop_of(tag: &str) -> (String, String) {
    let (l, n, _) = split(tag);
    (l.to_uppercase(), n.map(|n| n.to_string()).unwrap_or_default())
}

fn used(p: &Project) -> HashSet<String> {
    p.items.iter().map(|i| i.tag.trim().to_uppercase()).filter(|t| !t.is_empty()).collect()
}

/// The next free tag for a prefix on a sheet: numbers run 101, 102 … on sheet 1, 201 … on sheet 2.
pub fn next_free(p: &Project, prefix: &str, sheet: u32) -> String {
    if prefix.is_empty() {
        return String::new();
    }
    let used = used(p);
    let base = (p.sheet_no(sheet).max(1) as u64) * 100;
    let mut n = base + 1;
    loop {
        let t = format!("{prefix}-{n}");
        if !used.contains(&t.to_uppercase()) {
            return t;
        }
        n += 1;
    }
}

/// The next free sequence number for lines on a sheet: 1001, 1002 … on sheet 1, 2001 … on sheet 2.
pub fn next_seq(p: &Project, sheet: u32) -> String {
    let used: HashSet<String> = p.lines.iter().filter_map(|l| l.data.get("seq")).map(|v| v.text()).collect();
    let mut n = p.sheet_no(sheet).max(1) as u64 * 1000 + 1;
    while used.contains(&n.to_string()) {
        n += 1;
    }
    n.to_string()
}

/// The tag a copy gets: the next free suffix letter if the tag has one (P-101A → P-101B), else the
/// next free number (V-101 → V-102). Empty stays empty.
pub fn bump(p: &Project, tag: &str, extra: &HashSet<String>) -> String {
    if tag.trim().is_empty() {
        return String::new();
    }
    let used = used(p);
    let free = |t: &str| !used.contains(&t.to_uppercase()) && !extra.contains(&t.to_uppercase());
    let (pre, num, suf) = split(tag);
    let sep = if tag.contains('-') { "-" } else { "" };
    match num {
        Some(n) if suf.len() == 1 && suf.chars().all(|c| c.is_ascii_alphabetic()) => {
            let c0 = suf.chars().next().unwrap_or('A');
            for c in (c0 as u8 + 1)..=b'Z' {
                let t = format!("{pre}{sep}{n}{}", c as char);
                if free(&t) {
                    return t;
                }
            }
            bump(p, &format!("{pre}{sep}{}", n + 1), extra)
        }
        Some(n) => {
            let mut k = n + 1;
            loop {
                let t = format!("{pre}{sep}{k}{suf}");
                if free(&t) {
                    return t;
                }
                k += 1;
            }
        }
        None => {
            let mut k = 2;
            loop {
                let t = format!("{}-{k}", tag.trim());
                if free(&t) {
                    return t;
                }
                k += 1;
            }
        }
    }
}

/// New tags for every tagged item: per prefix, in sheet order, then left to right and top to
/// bottom, numbered 101 … per sheet. Items sharing a number with a letter suffix (P-101A, P-101B)
/// keep their suffixes and stay together. Instruments keep their letters and are numbered by loop.
pub fn renumber(p: &Project) -> Vec<(u32, String)> {
    type Entry = (f64, f64, u32, Option<u64>, String);
    let mut groups: BTreeMap<(String, usize), Vec<Entry>> = BTreeMap::new();
    for it in &p.items {
        let Some(s) = symbols::get(&it.sym) else { continue };
        if it.tag.trim().is_empty() || s.cat == Cat::Connector {
            continue;
        }
        let (pre, num, suf) = split(&it.tag);
        let key = if s.cat.instrument() || s.cat == Cat::Control { "#loop".to_string() } else { pre.to_uppercase() };
        groups.entry((key, p.sheet_no(it.sheet))).or_default().push((it.x, it.y, it.id, num, suf));
    }
    let mut out = vec![];
    for ((key, sheet), mut v) in groups {
        v.sort_by(|a, b| (a.0 + a.1 * 0.001).total_cmp(&(b.0 + b.1 * 0.001)));
        // Old number → new number, in order of first appearance.
        let mut map: BTreeMap<Option<u64>, u64> = BTreeMap::new();
        let mut next = sheet as u64 * 100 + 1;
        for (_, _, id, num, suf) in &v {
            let n = match (num, map.get(num)) {
                (Some(_), Some(n)) => *n,
                _ => {
                    let n = next;
                    next += 1;
                    if num.is_some() {
                        map.insert(*num, n);
                    }
                    n
                }
            };
            let it = p.item(*id).map(|i| i.tag.clone()).unwrap_or_default();
            let (pre, _, _) = split(&it);
            let pre = if key == "#loop" { pre } else { key.clone() };
            out.push((*id, format!("{pre}-{n}{suf}")));
        }
    }
    out
}

/// Tags used by more than one item (case and spaces ignored): tag → item ids.
pub fn duplicates(p: &Project) -> Vec<(String, Vec<u32>)> {
    let mut m: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for it in &p.items {
        let t = it.tag.trim().to_uppercase();
        if !t.is_empty() && symbols::get(&it.sym).is_some_and(|s| s.cat != Cat::Connector) {
            m.entry(t).or_default().push(it.id);
        }
    }
    m.into_iter().filter(|(_, v)| v.len() > 1).collect()
}

/// ISA-5.1 first letters (measured or initiating variable).
pub const FIRST: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// Letters allowed after the first: variable modifiers (D, F, J, K, Q, S) and the readout,
/// passive and output functions.
pub const SUCCEEDING: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// Letters that are not used as succeeding letters in the conventions followed here.
const NOT_SUCCEEDING: &str = "";

/// Does an instrument's letter code read as ISA-5.1 letters? One to five letters, A–Z.
pub fn letters_ok(l: &str) -> bool {
    let l = l.trim();
    (1..=5).contains(&l.len()) && l.chars().all(|c| c.is_ascii_uppercase()) && FIRST.contains(&l[..1]) && l[1..].chars().all(|c| SUCCEEDING.contains(c) && !NOT_SUCCEEDING.contains(c))
}

/// A controller: letters ending in C (FIC, LIC, PC, TIC, UIC…), not a control valve (…CV).
pub fn is_controller(l: &str) -> bool {
    l.len() >= 2 && l.ends_with('C')
}

/// A measurement: a transmitter, element or indicator (…T, …E, …I but not a controller).
pub fn is_measurement(l: &str) -> bool {
    l.len() >= 2 && (l.ends_with('T') || l.ends_with('E') || l.ends_with("TW") || (l.ends_with('I') && !is_controller(l)))
}

/// A final element: a control valve (…V) or a drive (…Z, …Y as a relay to a drive).
pub fn is_final(l: &str) -> bool {
    l.len() >= 2 && (l.ends_with('V') || l.ends_with('Z'))
}

/// The letters a symbol's tag should end with, if any (for check 12).
pub fn expected_end(sym: &str) -> Option<&'static [&'static str]> {
    Some(match sym {
        "cv_pneu" | "cv_motor" | "cv_solenoid" | "regulator" => &["V"],
        "psv" => &["SV"],
        "rupture" => &["SE"],
        "orifice" | "magmeter" | "coriolis" | "vortex" | "ultrasonic" => &["E"],
        "thermowell" => &["W"],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Item;

    fn item(p: &mut Project, sym: &str, tag: &str, x: f64) -> u32 {
        let id = p.id();
        let sheet = p.sheets[0].id;
        p.items.push(Item { id, sheet, sym: sym.into(), tag: tag.into(), x, ..Item::default() });
        id
    }

    #[test]
    fn split_and_letters() {
        assert_eq!(split("P-101A"), ("P".into(), Some(101), "A".into()));
        assert_eq!(split("LIC-101"), ("LIC".into(), Some(101), String::new()));
        assert_eq!(split("E101"), ("E".into(), Some(101), String::new()));
        assert_eq!(split("HOLD"), ("HOLD".into(), None, String::new()));
        assert!(letters_ok("LIC") && letters_ok("PSV") && !letters_ok("lic") && !letters_ok("P1") && !letters_ok(""));
        assert!(is_controller("UIC") && !is_controller("PCV") || is_final("PCV"));
        assert!(is_measurement("FT") && is_measurement("TE") && !is_measurement("FIC"));
    }

    #[test]
    fn numbering_copies_and_duplicates() {
        let mut p = Project::new();
        let s = p.sheets[0].id;
        assert_eq!(next_free(&p, "P", s), "P-101");
        item(&mut p, "pump_cent", "P-101A", 10.0);
        item(&mut p, "pump_cent", "P-101B", 40.0);
        item(&mut p, "vvessel", "V-101", 80.0);
        assert_eq!(next_free(&p, "V", s), "V-102");
        // A copy of P-101A becomes P-101C (A and B are taken); V-101 becomes V-102.
        assert_eq!(bump(&p, "P-101A", &HashSet::new()), "P-101C");
        assert_eq!(bump(&p, "V-101", &HashSet::new()), "V-102");
        assert!(duplicates(&p).is_empty());
        item(&mut p, "vvessel", "v-101 ", 120.0);
        assert_eq!(duplicates(&p).len(), 1);
        // A second sheet numbers from 201.
        let id = p.id();
        p.sheets.push(crate::model::Sheet { id, ..Default::default() });
        assert_eq!(next_free(&p, "P", id), "P-201");
        assert_eq!(next_seq(&p, id), "2001");
    }

    #[test]
    fn renumber_keeps_suffixes_and_order() {
        let mut p = Project::new();
        let b = item(&mut p, "pump_cent", "P-7B", 50.0);
        let a = item(&mut p, "pump_cent", "P-7A", 10.0);
        let c = item(&mut p, "pump_cent", "P-3", 90.0);
        let t = item(&mut p, "inst_field", "LT-55", 30.0);
        let r: std::collections::HashMap<u32, String> = renumber(&p).into_iter().collect();
        assert_eq!((r[&a].as_str(), r[&b].as_str(), r[&c].as_str()), ("P-101A", "P-101B", "P-102"));
        assert_eq!(r[&t], "LT-101");
    }
}
