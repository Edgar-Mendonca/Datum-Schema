//! Find anything: a plain query over the project, the same in the window, the command line and an
//! AI assistant. A query names what to look at ("control valves", "pumps", "lines", "gate valves",
//! a tag pattern like FT-1*), then conditions joined by "and":
//! - missing or present data: "without a size", "with no range", "with service";
//! - comparisons with units: "velocity above 3 m/s", "design pressure > 10 bar", "size < 2";
//! - the drawing: "without a bypass", "without isolation", "without a check valve", "not drawn",
//!   "on PID-100", "in Area 1", "with errors", "failing rule 8", "fail open", "normally closed";
//! - text in quotes: "cooling water".
//!
//! What was understood is said back in words, and any word not understood is listed, so a query
//! never silently means something else.

use crate::graph::Net;
use crate::model::{Item, Project, Val, LINE_CLASS, STREAM_CLASS};
use crate::symbols::{self, Cat, Sym};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct FRow {
    /// The first symbol or line piece that shows it.
    pub id: u32,
    pub sheet: u32,
    pub tag: String,
    pub what: String,
    /// Why it matched (the values the conditions looked at).
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Default)]
pub struct Found {
    /// The query as understood, in words.
    pub understood: Vec<String>,
    /// Words not understood (ignored).
    pub unknown: Vec<String>,
    pub rows: Vec<FRow>,
}

#[derive(Clone, Debug, PartialEq)]
enum Subject {
    All,
    Equipment,
    Vessels,
    Pumps,
    Compressors,
    Exchangers,
    Valves,
    ManualValves,
    ControlValves,
    CheckValves,
    Relief,
    Instruments,
    Transmitters,
    Controllers,
    Lines,
    Strainers,
    Sym(String),
}

const SUBJECTS: &[(&str, Subject)] = &[
    ("control valves", Subject::ControlValves),
    ("control valve", Subject::ControlValves),
    ("check valves", Subject::CheckValves),
    ("check valve", Subject::CheckValves),
    ("manual valves", Subject::ManualValves),
    ("hand valves", Subject::ManualValves),
    ("relief valves", Subject::Relief),
    ("safety valves", Subject::Relief),
    ("relief devices", Subject::Relief),
    ("line numbers", Subject::Lines),
    ("heat exchangers", Subject::Exchangers),
    ("exchangers", Subject::Exchangers),
    ("exchanger", Subject::Exchangers),
    ("psvs", Subject::Relief),
    ("valves", Subject::Valves),
    ("valve", Subject::Valves),
    ("pumps", Subject::Pumps),
    ("pump", Subject::Pumps),
    ("compressors", Subject::Compressors),
    ("compressor", Subject::Compressors),
    ("vessels", Subject::Vessels),
    ("vessel", Subject::Vessels),
    ("tanks", Subject::Vessels),
    ("drums", Subject::Vessels),
    ("equipment", Subject::Equipment),
    ("instruments", Subject::Instruments),
    ("instrument", Subject::Instruments),
    ("transmitters", Subject::Transmitters),
    ("transmitter", Subject::Transmitters),
    ("controllers", Subject::Controllers),
    ("controller", Subject::Controllers),
    ("lines", Subject::Lines),
    ("line", Subject::Lines),
    ("pipes", Subject::Lines),
    ("strainers", Subject::Strainers),
    ("strainer", Subject::Strainers),
    ("tags", Subject::All),
    ("everything", Subject::All),
    ("anything", Subject::All),
    ("objects", Subject::All),
];

const FILLER: &[&str] = &["all", "the", "a", "an", "that", "have", "has", "had", "which", "are", "is", "show", "find", "me", "list", "every", "of", "whose", "where", "there", "any", "and", "with", "without", "no", "on", "in", "its", "their", "than", "it", "they", "them", "please", "what", "who", "do", "does", "to", "for", "by"];

/// One thing a project's query can look at.
struct Cand<'a> {
    obj: u32,
    id: u32,
    sheet: u32,
    item: Option<&'a Item>,
    sym: Option<&'static Sym>,
    line: bool,
    tag: String,
}

/// A data field a query can name: its key, label and quantity.
struct Field {
    key: String,
    label: String,
    q: String,
}

fn fields(p: &Project) -> Vec<Field> {
    let mut v: Vec<Field> = vec![
        Field { key: "~v".into(), label: "velocity".into(), q: "velocity".into() },
        Field { key: "~dp100".into(), label: "pressure drop".into(), q: "dp100".into() },
        Field { key: "size".into(), label: "size".into(), q: String::new() },
        Field { key: "~tag".into(), label: "tag".into(), q: String::new() },
    ];
    let mut seen: HashSet<String> = v.iter().map(|f| f.key.clone()).collect();
    for c in crate::classes::all() {
        for a in crate::classes::attrs(Some(p), &c.name) {
            if seen.insert(a.key.clone()) {
                v.push(Field { key: a.key.clone(), label: a.label.to_lowercase(), q: a.q.clone() });
            }
        }
    }
    // Longer labels first, so "design pressure" wins over "pressure".
    v.sort_by_key(|f| std::cmp::Reverse(f.label.len()));
    v
}

fn wildcard(pat: &str, s: &str) -> bool {
    let (pat, s) = (pat.to_uppercase(), s.to_uppercase());
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == s;
    }
    let mut at = 0;
    for (k, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match s[at..].find(part) {
            Some(i) if k > 0 || i == 0 => at += i + part.len(),
            _ => return false,
        }
    }
    parts.last().is_some_and(|l| l.is_empty()) || at == s.len()
}

/// Finds what the query asks for.
pub fn find(p: &Project, query: &str) -> Result<Found, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("Type what to find: \"control valves without a bypass\", \"lines with velocity above 3 m/s\", \"pumps without a check valve\"…".into());
    }
    let us = p.units == "US";
    let net = Net::new(p);
    let hyd = crate::hydraulics::all(p, &net);
    let mut out = Found::default();

    // Quoted text first.
    let mut texts: Vec<String> = vec![];
    let mut rest = String::new();
    for (k, part) in query.split('"').enumerate() {
        if k % 2 == 1 {
            if !part.trim().is_empty() {
                texts.push(part.trim().to_lowercase());
            }
        } else {
            rest.push_str(part);
            rest.push(' ');
        }
    }
    let rest = rest.replace(',', " ").replace(">=", " >= ").replace("<=", " <= ");
    let orig: Vec<String> = rest.split_whitespace().map(String::from).collect();
    let mut words: Vec<String> = orig.iter().map(|w| w.to_lowercase()).collect();
    let mut used = vec![false; words.len()];
    // Finds a phrase (whole words, not yet used); marks it used; returns its first index.
    let take = |words: &[String], used: &mut Vec<bool>, phrase: &str| -> Option<usize> {
        let ph: Vec<&str> = phrase.split(' ').collect();
        (0..words.len().saturating_sub(ph.len() - 1)).find(|&i| (0..ph.len()).all(|k| !used[i + k] && words[i + k] == ph[k])).inspect(|&i| (0..ph.len()).for_each(|k| used[i + k] = true))
    };

    // Conditions, each a test on a candidate with a detail.
    type Test<'a> = Box<dyn Fn(&Cand) -> Option<String> + 'a>;
    let mut tests: Vec<Test> = vec![];
    let issues = crate::checks::run(p, &net, &hyd);
    let flagged = |levels: &[&str]| -> BTreeMap<u32, Vec<String>> {
        let mut m: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for i in issues.iter().filter(|i| levels.contains(&i.level)) {
            for id in &i.ids {
                m.entry(*id).or_default().push(format!("rule {}", i.rule));
            }
        }
        m
    };
    let flags_any = |c: &Cand, m: &BTreeMap<u32, Vec<String>>| -> Option<String> {
        let ids: Vec<u32> = if c.line { p.lines.iter().filter(|l| l.obj == c.obj).map(|l| l.id).collect() } else { p.items.iter().filter(|i| i.obj == c.obj).map(|i| i.id).collect() };
        let v: Vec<String> = ids.iter().filter_map(|i| m.get(i)).flatten().cloned().collect();
        (!v.is_empty()).then(|| v.join(", "))
    };

    // The drawing's relations.
    for (phrase, want, label) in [("without a bypass", false, "without a bypass"), ("without bypass", false, "without a bypass"), ("with a bypass", true, "with a bypass"), ("with bypass", true, "with a bypass")] {
        if take(&words, &mut used, phrase).is_some() {
            out.understood.push(label.into());
            let n = &net;
            tests.push(Box::new(move |c: &Cand| (c.id != 0 && c.sym.is_some_and(|s| s.cat == Cat::Control) && bypass(n, c) == want).then(String::new)));
        }
    }
    for phrase in ["without a check valve", "without check valve", "with no check valve"] {
        if take(&words, &mut used, phrase).is_some() {
            out.understood.push("without a check valve on the discharge".into());
            let n = &net;
            tests.push(Box::new(move |c: &Cand| (c.id != 0 && c.sym.is_some_and(|s| s.cat == Cat::Rotating) && !has_check(n, c)).then(String::new)));
        }
    }
    for phrase in ["without isolation", "without block valves", "without isolation valves", "with no isolation"] {
        if take(&words, &mut used, phrase).is_some() {
            out.understood.push("without isolation valves on both sides".into());
            let n = &net;
            tests.push(Box::new(move |c: &Cand| (c.id != 0 && c.sym.is_some_and(|s| s.inline) && !isolated(n, c)).then(String::new)));
        }
    }
    for (phrase, levels, label) in [("with problems", &["error", "warning", "note"][..], "with problems"), ("with errors", &["error"][..], "with errors"), ("with warnings", &["warning"][..], "with warnings"), ("with notes", &["note"][..], "with notes"), ("with accepted problems", &["accepted"][..], "with accepted problems")] {
        if take(&words, &mut used, phrase).is_some() {
            out.understood.push(label.into());
            let m = flagged(levels);
            tests.push(Box::new(move |c: &Cand| flags_any(c, &m)));
        }
    }
    for i in 0..words.len() {
        if !used[i] && words[i] == "failing" {
            let k = if words.get(i + 1).is_some_and(|w| w == "rule" || w == "check") { i + 2 } else { i + 1 };
            if let Some(n) = words.get(k).and_then(|w| w.parse::<u8>().ok()) {
                (i..=k).for_each(|x| used[x] = true);
                out.understood.push(format!("failing rule {n}"));
                let ids: HashSet<u32> = issues.iter().filter(|x| x.rule == n && x.level != "accepted").flat_map(|x| x.ids.clone()).collect();
                tests.push(Box::new(move |c: &Cand| {
                    let mine: Vec<u32> = if c.line { p.lines.iter().filter(|l| l.obj == c.obj).map(|l| l.id).collect() } else { p.items.iter().filter(|it| it.obj == c.obj).map(|it| it.id).collect() };
                    mine.iter().any(|x| ids.contains(x)).then(|| format!("rule {n}"))
                }));
            }
        }
    }
    for phrase in ["virtual instruments", "virtual"] {
        if take(&words, &mut used, phrase).is_some() {
            out.understood.push("virtual (not drawn on the sheet)".into());
            tests.push(Box::new(move |c: &Cand| c.item.is_some_and(|i| i.virt).then(String::new)));
        }
    }
    for phrase in ["not drawn", "undrawn", "registered"] {
        if take(&words, &mut used, phrase).is_some() {
            out.understood.push("not drawn yet".into());
            tests.push(Box::new(move |c: &Cand| (c.id == 0).then(String::new)));
        }
    }
    for (phrase, key, val) in [("fail open", "fail", "FO"), ("fail closed", "fail", "FC"), ("fail last", "fail", "FL"), ("fails open", "fail", "FO"), ("fails closed", "fail", "FC"), ("normally closed", "position", "NC"), ("normally open", "position", "NO"), ("locked closed", "position", "LC"), ("locked open", "position", "LO")] {
        if take(&words, &mut used, phrase).is_some() {
            out.understood.push(phrase.into());
            tests.push(Box::new(move |c: &Cand| p.data_of(c.obj).get(key).is_some_and(|v| v.text().trim().eq_ignore_ascii_case(val)).then(|| format!("{key} {val}"))));
        }
    }
    // The subject (after the relation phrases, so "without a check valve" is not taken as one): the longest phrase naming a kind of thing, or a symbol's name.
    let mut subj = Subject::All;
    let mut named: Vec<(String, Subject, String)> = SUBJECTS.iter().map(|(ph, s)| (ph.to_string(), s.clone(), ph.to_string())).collect();
    for s in symbols::all().iter().filter(|s| !s.rare) {
        let n = s.name.to_lowercase();
        named.push((format!("{n}s"), Subject::Sym(s.key.into()), format!("{n}s")));
        named.push((n.clone(), Subject::Sym(s.key.into()), format!("{n}s")));
    }
    named.sort_by_key(|x| std::cmp::Reverse(x.0.len()));
    for (ph, s, label) in &named {
        if take(&words, &mut used, ph).is_some() {
            subj = s.clone();
            out.understood.insert(0, capital(label));
            break;
        }
    }
    if subj == Subject::All && !out.understood.iter().any(|u| u.chars().next().is_some_and(|c| c.is_uppercase())) {
        out.understood.insert(0, "Everything".into());
    }

    // Sheets and areas.
    for s in &p.sheets {
        let n = s.number.to_lowercase();
        if take(&words, &mut used, &n).is_some() {
            let _ = take(&words, &mut used, "sheet");
            out.understood.push(format!("on {}", s.number));
            let sid = s.id;
            tests.push(Box::new(move |c: &Cand| {
                let on = p.items.iter().any(|i| i.obj == c.obj && i.sheet == sid) || p.lines.iter().any(|l| l.obj == c.obj && l.sheet == sid);
                on.then(String::new)
            }));
        }
    }
    for node in &p.nodes {
        let n = node.name.to_lowercase();
        if !n.is_empty() && take(&words, &mut used, &n).is_some() {
            out.understood.push(format!("in {}", node.name));
            let id = node.id;
            let under: HashSet<u32> = {
                let mut s: HashSet<u32> = [id].into();
                loop {
                    let more: Vec<u32> = p.nodes.iter().filter(|x| s.contains(&x.parent) && !s.contains(&x.id)).map(|x| x.id).collect();
                    if more.is_empty() {
                        break;
                    }
                    s.extend(more);
                }
                s
            };
            tests.push(Box::new(move |c: &Cand| p.obj(c.obj).is_some_and(|o| under.contains(&o.node)).then(String::new)));
        }
    }

    // Comparisons: <field> <op> <number> [unit].
    let fs = fields(p);
    let ops: &[(&str, &str)] = &[("more than", ">"), ("less than", "<"), ("at least", ">="), ("at most", "<="), ("above", ">"), ("over", ">"), ("below", "<"), ("under", "<"), (">=", ">="), ("<=", "<="), (">", ">"), ("<", "<"), ("=", "="), ("equals", "="), ("is", "=")];
    let mut i = 0;
    while i < words.len() {
        let Some(&(opw, op)) = ops.iter().find(|(w, _)| { let ph: Vec<&str> = w.split(' ').collect(); i + ph.len() <= words.len() && (0..ph.len()).all(|k| !used[i + k] && words[i + k] == ph[k]) }) else { i += 1; continue };
        let oplen = opw.split(' ').count();
        // The field: the longest label ending just before the operator.
        let field = fs.iter().find(|f| {
            let n = f.label.split(' ').count();
            i >= n && (0..n).all(|k| !used[i - n + k]) && words[i - n..i].join(" ") == f.label || (i >= 1 && !used[i - 1] && words[i - 1] == f.key.to_lowercase())
        });
        let Some(f) = field else { i += oplen; continue };
        let flen = if i >= f.label.split(' ').count() && words[i - f.label.split(' ').count()..i].join(" ") == f.label { f.label.split(' ').count() } else { 1 };
        let at = i + oplen;
        let num = words.get(at).and_then(|w| w.trim_end_matches('"').parse::<f64>().ok());
        let Some(num) = num else { i += oplen; continue };
        // A unit: one or two words that make a unit of the field's kind.
        let mut ulen = 0;
        let mut si = if f.q.is_empty() { num } else { crate::units::from_display(num, &f.q, us) };
        for n in [2usize, 1] {
            if at + n < orig.len() + 1 && at + 1 + n <= orig.len() {
                let u = orig[at + 1..at + 1 + n].join(" ");
                if let Ok((fct, _, off)) = crate::units::expr(&u) {
                    si = fct * num + off;
                    ulen = n;
                    break;
                }
            }
        }
        used[(i - flen)..(at + 1 + ulen)].iter_mut().for_each(|u| *u = true);
        let shown = if ulen > 0 { orig[at..at + 1 + ulen].join(" ") } else { orig[at].clone() };
        out.understood.push(format!("{} {} {}", f.label, opw, shown));
        let (key, q, op) = (f.key.clone(), f.q.clone(), op.to_string());
        let hy = &hyd;
        let n = &net;
        tests.push(Box::new(move |c: &Cand| {
            let v = value(p, n, hy, c, &key)?;
            let ok = match op.as_str() {
                ">" => v > si,
                "<" => v < si,
                ">=" => v >= si,
                "<=" => v <= si,
                _ => (v - si).abs() <= 1e-9 * si.abs().max(1.0),
            };
            ok.then(|| if q.is_empty() { format!("{} {}", key.trim_start_matches('~'), crate::fmt_sig(v, 4)) } else { format!("{} {}", key.trim_start_matches('~'), crate::units::show(v, &q, us)) })
        }));
        i = at + 1 + ulen;
    }

    // Missing or present data: "without a size", "with no range", "missing service", "with service".
    let neg = ["without", "no", "missing", "lacking"];
    for i in 0..words.len() {
        if used[i] {
            continue;
        }
        let (negative, start) = if neg.contains(&words[i].as_str()) {
            (true, i + 1)
        } else if words[i] == "with" && words.get(i + 1).is_some_and(|w| w == "no") {
            (true, i + 2)
        } else if words[i] == "with" {
            (false, i + 1)
        } else {
            continue;
        };
        let start = if words.get(start).is_some_and(|w| w == "a" || w == "an" || w == "any") { start + 1 } else { start };
        let Some(f) = fs.iter().find(|f| {
            let n = f.label.split(' ').count();
            start + n <= words.len() && (0..n).all(|k| !used[start + k]) && words[start..start + n].join(" ") == f.label
        }) else { continue };
        let n = f.label.split(' ').count();
        (i..start + n).for_each(|x| used[x] = true);
        out.understood.push(format!("{} {}", if negative { "without" } else { "with" }, f.label));
        let key = f.key.clone();
        let hy = &hyd;
        let nn = &net;
        tests.push(Box::new(move |c: &Cand| {
            let has = if key == "~tag" { !c.tag.is_empty() } else if key.starts_with('~') { value(p, nn, hy, c, &key).is_some() } else { p.data_of(c.obj).get(&key).is_some_and(|v| !v.text().trim().is_empty()) };
            (has != negative).then(String::new)
        }));
    }

    // Tag patterns (FT-1*, P-101A) and quoted text.
    for (k, w) in orig.iter().enumerate() {
        if used[k] {
            continue;
        }
        // A word after "on" or "sheet" names a sheet; one that is not a sheet is reported, not taken as a tag.
        let after_on = k > 0 && ["on", "sheet"].contains(&words[k - 1].as_str());
        let looks = !after_on && (w.contains('*') || (w.contains('-') && w.chars().any(|c| c.is_ascii_digit()) && w.chars().any(|c| c.is_ascii_alphabetic())));
        if looks {
            used[k] = true;
            out.understood.push(format!("tagged {w}"));
            let pat = w.clone();
            tests.push(Box::new(move |c: &Cand| wildcard(&pat, &c.tag).then(String::new)));
        }
    }
    for t in texts {
        out.understood.push(format!("mentioning \"{t}\""));
        tests.push(Box::new(move |c: &Cand| {
            let hit = c.tag.to_lowercase().contains(&t) || p.data_of(c.obj).iter().any(|(k, v)| !k.starts_with('~') && v.text().to_lowercase().contains(&t));
            hit.then(String::new)
        }));
    }
    // Words left over.
    for (k, w) in words.iter_mut().enumerate() {
        if !used[k] && !FILLER.contains(&w.as_str()) && w.parse::<f64>().is_err() {
            out.unknown.push(orig[k].clone());
        }
    }

    // The candidates: each object once, at its first symbol or line piece (P&ID sheets first).
    let pfd: HashSet<u32> = p.sheets.iter().filter(|s| s.pfd).map(|s| s.id).collect();
    let mut cands: Vec<Cand> = vec![];
    for o in &p.objects {
        if o.class == STREAM_CLASS {
            continue;
        }
        let line = o.class == LINE_CLASS;
        if line {
            let Some(l) = p.lines.iter().filter(|l| l.obj == o.id).min_by_key(|l| (pfd.contains(&l.sheet), l.id)) else { continue };
            if !l.kind.process() || crate::graph::instrument_connection(p, l) {
                continue;
            }
            cands.push(Cand { obj: o.id, id: l.id, sheet: l.sheet, item: None, sym: None, line: true, tag: crate::graph::obj_number(p, o.id) });
        } else {
            let it = p.items.iter().filter(|i| i.obj == o.id).min_by_key(|i| (pfd.contains(&i.sheet), i.id));
            let sym = it.and_then(|i| symbols::get(&i.sym)).or_else(|| crate::classes::get(&o.class).and_then(|c| c.symbols.first()).and_then(|k| symbols::get(k)));
            if it.is_none() && o.tag.is_empty() {
                continue;
            }
            cands.push(Cand { obj: o.id, id: it.map_or(0, |i| i.id), sheet: it.map_or(0, |i| i.sheet), item: it, sym, line: false, tag: o.tag.clone() });
        }
    }
    let base = |c: &Cand| c.item.map(|i| i.base_key()).or_else(|| c.sym.map(|s| s.key.to_string())).unwrap_or_default();
    let is = |c: &Cand| -> bool {
        let cat = c.sym.map(|s| s.cat);
        let b = base(c);
        let letters = crate::tags::loop_of(&c.tag).0;
        match &subj {
            Subject::All => c.line || !c.tag.is_empty(),
            Subject::Lines => c.line,
            Subject::Equipment => cat.is_some_and(|x| x.equipment()),
            Subject::Vessels => cat == Some(Cat::Vessel),
            Subject::Pumps => b.starts_with("pump"),
            Subject::Compressors => b.starts_with("comp") || b == "fan",
            Subject::Exchangers => cat == Some(Cat::Heat),
            Subject::Valves => cat.is_some_and(|x| x.valve()) && !matches!(b.as_str(), "psv" | "rupture" | "breather"),
            Subject::ManualValves => cat == Some(Cat::Valve) && !matches!(b.as_str(), "check" | "nonslam"),
            Subject::ControlValves => cat == Some(Cat::Control) && !matches!(b.as_str(), "psv" | "rupture" | "breather"),
            Subject::CheckValves => matches!(b.as_str(), "check" | "nonslam"),
            Subject::Relief => matches!(b.as_str(), "psv" | "rupture" | "breather"),
            Subject::Instruments => cat.is_some_and(|x| x.instrument()),
            Subject::Transmitters => cat.is_some_and(|x| x.instrument()) && letters.len() >= 2 && letters.ends_with('T'),
            Subject::Controllers => cat.is_some_and(|x| x.instrument()) && letters.len() >= 2 && letters[1..].contains('C'),
            Subject::Strainers => b.starts_with("strainer"),
            Subject::Sym(k) => &b == k,
        }
    };
    for c in cands.iter().filter(|c| is(c)) {
        let mut detail = vec![];
        let mut ok = true;
        for t in &tests {
            match t(c) {
                Some(d) => {
                    if !d.is_empty() {
                        detail.push(d);
                    }
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            let what = if c.line { "Line number".into() } else { c.sym.map_or(String::new(), |s| s.name.to_string()) };
            out.rows.push(FRow { id: c.id, sheet: c.sheet, tag: if c.tag.is_empty() { "(no tag)".into() } else { c.tag.clone() }, what, detail: detail.join("; ") });
        }
    }
    out.rows.sort_by(|a, b| {
        let k = |t: &str| crate::tags::split(t);
        let (x, y) = (k(&a.tag), k(&b.tag));
        (x.0, x.1, a.tag.clone()).cmp(&(y.0, y.1, b.tag.clone()))
    });
    Ok(out)
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

/// A field's number on a candidate (SI), from its data or the hydraulics.
fn value(p: &Project, net: &Net, hyd: &[Option<crate::hydraulics::Hyd>], c: &Cand, key: &str) -> Option<f64> {
    match key {
        "~v" | "~dp100" => {
            let l = p.lines.iter().find(|l| l.obj == c.obj)?;
            let run = *net.run_of.get(&l.id)?;
            let h = hyd.get(run)?.as_ref()?;
            Some(if key == "~v" { h.v } else { h.dp100 })
        }
        "size" => crate::pipes::nps_inches(&p.data_of(c.obj).get("size")?.text()),
        k => match p.data_of(c.obj).get(k)? {
            Val::Num(x) => Some(*x),
            Val::Text(t) => t.trim().parse().ok(),
        },
    }
}

/// The process ports of an in-line item: (inlet, outlet).
fn io(c: &Cand) -> Option<(u8, u8)> {
    let s = c.sym?;
    let ps: Vec<u8> = s.ports.iter().enumerate().filter(|(_, q)| !q.signal).map(|(k, _)| k as u8).collect();
    Some((*ps.first()?, *ps.get(1)?))
}

/// Does a control valve have a bypass: do the two sides meet without going through it?
fn bypass(net: &Net, c: &Cand) -> bool {
    let Some((a, b)) = io(c) else { return false };
    let stop = |_: &Item, s: &Sym| s.cat.equipment();
    let up: HashSet<u32> = net.reach(c.id, a, true, 14, stop).into_iter().map(|x| x.0).collect();
    let down: HashSet<u32> = net.reach(c.id, b, true, 14, stop).into_iter().map(|x| x.0).collect();
    up.iter().any(|x| down.contains(x))
}

/// Is an in-line item isolated by a manual valve on each side?
fn isolated(net: &Net, c: &Cand) -> bool {
    let Some((a, b)) = io(c) else { return false };
    let side = |port: u8| net.reach(c.id, port, false, 6, |_, s| s.cat == Cat::Valve || s.cat.equipment()).iter().any(|(i, _, _)| net.items.get(i).is_some_and(|it| net.sym(*i).is_some_and(|s| s.cat == Cat::Valve) && !matches!(it.base_key().as_str(), "check" | "nonslam")));
    side(a) && side(b)
}

/// Has a pump a check valve on its discharge (before the first other valve)?
fn has_check(net: &Net, c: &Cand) -> bool {
    let Some(s) = c.sym else { return false };
    let Some(k) = s.ports.iter().position(|q| q.name == "discharge" || q.name == "outlet") else { return false };
    net.reach(c.id, k as u8, true, 6, |_, s| s.cat.valve() || s.cat.equipment()).iter().any(|(i, _, _)| net.items.get(i).is_some_and(|it| matches!(it.base_key().as_str(), "check" | "nonslam")))
}

/// The results as a list (kind "find:<query>"): printed, exported and clicked like any list.
pub fn list(p: &Project, query: &str) -> Result<crate::lists::List, String> {
    use crate::lists::{Col, List, Row};
    let f = find(p, query)?;
    let col = |k: &str, l: &str| Col { key: k.into(), label: l.into(), q: "", edit: false, num: false };
    let mut title = format!("Find: {}", f.understood.join(", "));
    if !f.unknown.is_empty() {
        title += &format!(" (not understood: {})", f.unknown.join(" "));
    }
    let rows = f.rows.iter().map(|r| Row { ids: if r.id != 0 { vec![r.id] } else { vec![] }, sheet: r.sheet, cells: vec![r.tag.clone(), r.what.clone(), p.sheet(r.sheet).map(|s| s.number.clone()).unwrap_or_else(|| "not drawn".into()), r.detail.clone()], status: None }).collect();
    Ok(List { kind: format!("find:{query}"), title, cols: vec![col("tag", "Tag"), col("what", "What"), col("sheet", "Sheet"), col("detail", "Why")], rows, heads: vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    fn tags(p: &Project, q: &str) -> Vec<String> {
        find(p, q).unwrap().rows.into_iter().map(|r| r.tag).collect()
    }

    #[test]
    fn plain_queries_find_what_they_say() {
        let p = crate::examples::get("worked").unwrap();
        // Subjects.
        assert_eq!(tags(&p, "pumps"), ["P-101A", "P-101B", "P-102"], "P-102 is registered, not drawn");
        assert!(tags(&p, "control valves").contains(&"LV-101".to_string()));
        assert!(tags(&p, "check valves").len() == 2);
        assert!(tags(&p, "gate valves").contains(&"HV-101".to_string()) && !tags(&p, "gate valves").contains(&"HV-103".to_string()));
        assert!(tags(&p, "transmitters").contains(&"LT-101".to_string()) && !tags(&p, "transmitters").contains(&"LIC-101".to_string()));
        assert_eq!(tags(&p, "controllers"), ["LIC-101"]);
        assert!(tags(&p, "lines").iter().all(|t| t.contains("-P-") || t.contains("-VA-")));
        // The drawing's relations: LV-101 has block valves but no bypass; the pumps have check valves.
        assert_eq!(tags(&p, "control valves without a bypass"), ["LV-101"]);
        assert!(tags(&p, "control valves with a bypass").is_empty());
        assert!(tags(&p, "control valves without isolation").is_empty());
        assert!(tags(&p, "pumps without a check valve").is_empty());
        // Data: missing, present, compared with units.
        let f = find(&p, "lines with velocity above 1 m/s").unwrap();
        assert!(!f.rows.is_empty() && f.rows.iter().all(|r| r.detail.starts_with("v ")), "{f:?}");
        assert!(f.understood.iter().any(|u| u == "velocity above 1 m/s"));
        assert!(tags(&p, "lines with velocity above 50 m/s").is_empty());
        assert_eq!(tags(&p, "pumps with design pressure >= 10 bar"), ["P-101A", "P-101B"]);
        assert!(tags(&p, "pumps with design pressure > 10 bar").is_empty());
        assert!(tags(&p, "lines with size < 2").iter().all(|t| t.starts_with("1\"")));
        assert!(tags(&p, "instruments without a range").contains(&"LIC-101".to_string()) && !tags(&p, "instruments without a range").contains(&"LT-101".to_string()));
        assert!(tags(&p, "instruments with range").contains(&"LT-101".to_string()));
        assert_eq!(tags(&p, "control valves fail closed"), ["LV-101"]);
        assert_eq!(tags(&p, "valves normally closed"), ["HV-109", "HV-110", "HV-111"]);
        // Tags, text, sheets, registered tags.
        assert_eq!(tags(&p, "P-101*"), ["P-101A", "P-101B"]);
        assert_eq!(tags(&p, "equipment \"standby\""), ["P-101B"]);
        assert!(tags(&p, "pumps on PID-110").len() == 2 && tags(&p, "pumps on PID-999").len() == 3, "an unknown sheet is not a condition");
        assert_eq!(find(&p, "pumps on PID-999").unwrap().unknown, ["PID-999"]);
        assert_eq!(tags(&p, "equipment not drawn"), ["P-102"]);
        // Problems: none in the worked example but notes (rule 27).
        assert!(!tags(&p, "equipment with notes").is_empty() && tags(&p, "equipment with errors").is_empty());
        assert!(!tags(&p, "equipment failing rule 27").is_empty());
        // Words not understood are said; an empty query is refused.
        let f = find(&p, "pumps painted purple").unwrap();
        assert_eq!(f.unknown, ["painted", "purple"]);
        assert!(find(&p, "  ").is_err());
        // As a list.
        let l = crate::lists::build(&p, "find:control valves without a bypass").unwrap();
        assert!(l.title.starts_with("Find: Control valves, without a bypass") && l.rows.len() == 1);
    }

    #[test]
    fn a_problem_accepted_with_a_reason_is_kept_for_audit() {
        let mut d = Doc::new(crate::examples::get("worked").unwrap());
        let net = Net::new(&d.p);
        let hyd = crate::hydraulics::all(&d.p, &net);
        let i = crate::checks::run(&d.p, &net, &hyd).into_iter().find(|i| i.rule == 27).unwrap();
        assert!(d.apply(Op::Accept { rule: i.rule, ids: i.ids.clone(), msg: i.msg.clone(), reason: "".into(), by: String::new(), date: String::new() }).is_err(), "a reason is needed");
        d.user = "EM".into();
        d.apply(Op::Accept { rule: i.rule, ids: i.ids.clone(), msg: i.msg.clone(), reason: "High level overflows to the bund; no LSH needed".into(), by: String::new(), date: "2026-11-21".into() }).unwrap();
        let now = crate::checks::run(&d.p, &Net::new(&d.p), &hyd);
        assert_eq!(now.iter().find(|x| x.msg == i.msg).unwrap().level, "accepted");
        let l = crate::lists::build(&d.p, "accepted").unwrap();
        assert_eq!((l.rows[0].cells[3].as_str(), l.rows[0].cells[5].as_str()), ("EM", "Accepted"));
        assert!(find(&d.p, "equipment with accepted problems").unwrap().rows.len() == 1);
        // The drawing changes what the check says: open again, and the audit list says so.
        let t101 = d.p.items.iter().find(|x| d.p.tag_of(x) == "T-101").unwrap().obj;
        d.apply(Op::SetData { ids: vec![t101], key: "safety_na".into(), val: Some(Val::Text("LSH".into())) }).unwrap();
        let after = crate::checks::run(&d.p, &Net::new(&d.p), &crate::hydraulics::all(&d.p, &Net::new(&d.p)));
        assert!(after.iter().filter(|x| x.rule == 27 && x.ids == i.ids).all(|x| x.level != "accepted"));
        assert_eq!(crate::lists::build(&d.p, "accepted").unwrap().rows[0].cells[5], "Changed: check it again");
        // Undo, and open it again explicitly.
        d.undo();
        d.apply(Op::Unaccept { rule: i.rule, ids: i.ids.clone(), msg: i.msg.clone() }).unwrap();
        assert!(d.p.settings.accepted.is_empty());
        assert!(d.apply(Op::Unaccept { rule: i.rule, ids: i.ids.clone(), msg: i.msg.clone() }).is_err());
    }

    #[test]
    fn hostile_queries_never_panic() {
        let p = crate::examples::get("pump").unwrap();
        for q in ["\"", "\"\"\"", "> > >", "velocity above", "velocity above x m/s", "size < ", "*", "**-*", "failing rule 999", "with with with", "without", "on", "in", "lines with velocity above 1e308 m/s", "é ü 中文", "P-1\u{0}01", "a\"b\"c\"d", "design pressure >= -5 bar and size = 3"] {
            let _ = find(&p, q);
        }
    }
}
