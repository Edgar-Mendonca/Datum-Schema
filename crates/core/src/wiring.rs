//! Instrument wiring: junction boxes, cables and I/O cards are plant objects (not drawn); each
//! field device names its junction box and terminals, its cable, and its I/O channel as
//! "<card>/<channel>" (AI-01/3). From these: the cable schedule, the I/O allocation of every card
//! channel, and check 25 (what does not add up).

use crate::checks::Issue;
use crate::model::{Item, Obj, Project};
use std::collections::{BTreeMap, HashMap};

pub const JB: &str = "JunctionBox";
pub const CABLE: &str = "Cable";
pub const CARD: &str = "IOCard";

fn txt(o: &Obj, k: &str) -> String {
    o.data.get(k).map(|v| v.text().trim().to_string()).unwrap_or_default()
}

/// A device's I/O channel as (card tag, channel), when it is written "<card>/<channel>".
pub fn channel(io: &str) -> Option<(String, u32)> {
    let (card, ch) = io.trim().rsplit_once('/')?;
    let n: u32 = ch.trim().parse().ok()?;
    (!card.trim().is_empty() && n > 0).then(|| (card.trim().to_string(), n))
}

/// How many terminals a device uses at its junction box ("1, 2" → 2; "1-4" → 4).
fn terminals(s: &str) -> u32 {
    let parts: Vec<&str> = s.split([',', ';', ' ']).map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
    parts
        .iter()
        .map(|t| match t.split_once('-') {
            Some((a, b)) => match (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                (Ok(a), Ok(b)) if b >= a && b - a < 100 => b - a + 1,
                _ => 1,
            },
            None => 1,
        })
        .sum()
}

/// The wired devices: each field instrument and control valve once (at its first symbol).
fn devices(p: &Project) -> Vec<(&Item, &Obj)> {
    let mut seen = std::collections::HashSet::new();
    p.items
        .iter()
        .filter(|i| crate::symbols::get(&i.sym).is_some_and(|s| s.cat.instrument() || s.cat == crate::symbols::Cat::Control))
        .filter(|i| !crate::loops::in_system(p, i) && seen.insert(i.obj))
        .filter_map(|i| p.obj(i.obj).map(|o| (i, o)))
        .collect()
}

fn by_tag<'a>(p: &'a Project, class: &str) -> HashMap<String, &'a Obj> {
    p.objects.iter().filter(|o| o.class == class && !o.tag.trim().is_empty()).map(|o| (o.tag.trim().to_uppercase(), o)).collect()
}

/// One row of the cable schedule.
pub struct CableRow<'a> {
    pub cable: &'a Obj,
    pub devices: Vec<(&'a Item, &'a Obj)>,
}

pub fn cables(p: &Project) -> Vec<CableRow<'_>> {
    let devs = devices(p);
    let mut v: Vec<CableRow> = p.objects.iter().filter(|o| o.class == CABLE).map(|c| CableRow { cable: c, devices: devs.iter().filter(|(_, o)| txt(o, "cable").eq_ignore_ascii_case(&c.tag)).copied().collect() }).collect();
    v.sort_by_key(|r| { let (a, n, _) = crate::tags::split(&r.cable.tag); (a, n, r.cable.tag.clone()) });
    v
}

/// One channel of an I/O card: the devices on it (none: spare; more than one: a clash).
pub struct ChannelRow<'a> {
    pub card: &'a Obj,
    pub ch: u32,
    pub devices: Vec<(&'a Item, &'a Obj)>,
}

pub fn allocation(p: &Project) -> Vec<ChannelRow<'_>> {
    let devs = devices(p);
    let mut on: HashMap<(String, u32), Vec<(&Item, &Obj)>> = HashMap::new();
    for d in &devs {
        if let Some((card, ch)) = channel(&txt(d.1, "io")) {
            on.entry((card.to_uppercase(), ch)).or_default().push(*d);
        }
    }
    let mut cards: Vec<&Obj> = p.objects.iter().filter(|o| o.class == CARD).collect();
    cards.sort_by_key(|c| { let (a, n, _) = crate::tags::split(&c.tag); (a, n, c.tag.clone()) });
    let mut v = vec![];
    for c in cards {
        let n = c.data.get("channels").and_then(|x| x.num()).map_or(0, |x| x.clamp(0.0, 256.0) as u32);
        for ch in 1..=n {
            v.push(ChannelRow { card: c, ch, devices: on.remove(&(c.tag.trim().to_uppercase(), ch)).unwrap_or_default() });
        }
    }
    v
}

/// Check 25: wiring that does not add up. Junction boxes, cables and cards are checked against
/// only once the project has some of that kind (a project that types wiring as free text is let be).
pub fn checks(p: &Project) -> Vec<Issue> {
    let mut v = vec![];
    let (jbs, cables, cards) = (by_tag(p, JB), by_tag(p, CABLE), by_tag(p, CARD));
    if jbs.is_empty() && cables.is_empty() && cards.is_empty() {
        return v;
    }
    let devs = devices(p);
    let issue = |it: &Item, level: &'static str, msg: String| Issue { rule: 25, level, msg, ids: vec![it.id], sheet: it.sheet, at: [it.x, it.y] };
    let mut jb_used: BTreeMap<String, u32> = BTreeMap::new();
    let mut cable_used: BTreeMap<String, (u32, &Item)> = BTreeMap::new();
    let mut ch_used: HashMap<(String, u32), Vec<&Item>> = HashMap::new();
    for (it, o) in &devs {
        let tag = if o.tag.trim().is_empty() { "A device".to_string() } else { o.tag.clone() };
        let (jb, cable, io) = (txt(o, "jb"), txt(o, "cable"), txt(o, "io"));
        if !jb.is_empty() && !jbs.is_empty() {
            if jbs.contains_key(&jb.to_uppercase()) {
                *jb_used.entry(jb.to_uppercase()).or_default() += terminals(&txt(o, "jb_terminals"));
            } else {
                v.push(issue(it, "warning", format!("{tag} is wired to junction box {jb}, which is not in the project's wiring.")));
            }
        }
        if !cable.is_empty() && !cables.is_empty() {
            if cables.contains_key(&cable.to_uppercase()) {
                cable_used.entry(cable.to_uppercase()).or_insert((0, it)).0 += 1;
            } else {
                v.push(issue(it, "warning", format!("{tag} is on cable {cable}, which is not in the cable schedule.")));
            }
        }
        if io.is_empty() || cards.is_empty() {
            continue;
        }
        let Some((card, ch)) = channel(&io) else {
            v.push(issue(it, "note", format!("{tag}'s I/O channel “{io}” is not written as <card>/<channel> (such as AI-01/3).")));
            continue;
        };
        let Some(c) = cards.get(&card.to_uppercase()) else {
            v.push(issue(it, "warning", format!("{tag} is on I/O card {card}, which is not in the project's wiring.")));
            continue;
        };
        let n = c.data.get("channels").and_then(|x| x.num()).unwrap_or(0.0);
        if ch as f64 > n {
            v.push(issue(it, "error", format!("{tag} is on channel {ch} of {card}, which has {} channels.", crate::fmt_num(n))));
        }
        let kind = txt(c, "io_kind");
        if let Some(want) = crate::instruments::io_of(p, it).filter(|k| !kind.is_empty() && !k.eq_ignore_ascii_case(&kind)) {
            v.push(issue(it, "warning", format!("{tag} needs an {want} channel, but {card} is an {kind} card.")));
        }
        ch_used.entry((card.to_uppercase(), ch)).or_default().push(it);
    }
    for ((card, ch), its) in &ch_used {
        if its.len() > 1 {
            let tags: Vec<&str> = its.iter().map(|i| p.tag_of(i)).collect();
            let mut i = issue(its[0], "error", format!("{} share channel {ch} of {card}.", tags.join(" and ")));
            i.ids = its.iter().map(|x| x.id).collect();
            v.push(i);
        }
    }
    for (c, (used, it)) in cable_used {
        let pairs = cables[&c].data.get("pairs").and_then(|x| x.num()).unwrap_or(0.0);
        if pairs > 0.0 && used as f64 > pairs {
            v.push(issue(it, "error", format!("Cable {} carries {used} devices but has {} pairs.", cables[&c].tag, crate::fmt_num(pairs))));
        }
    }
    for (j, used) in jb_used {
        let n = jbs[&j].data.get("terminals").and_then(|x| x.num()).unwrap_or(0.0);
        if n > 0.0 && used as f64 > n {
            if let Some((it, _)) = devs.iter().find(|(_, o)| txt(o, "jb").eq_ignore_ascii_case(&j)) {
                v.push(issue(it, "error", format!("Junction box {} has {} terminals but its devices use {used}.", jbs[&j].tag, crate::fmt_num(n))));
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

    fn add(d: &mut Doc, class: &str, tag: &str, kv: &[(&str, Val)]) {
        d.apply(Op::AddObject { class: class.into(), tag: tag.into(), data: kv.iter().map(|(k, v)| (k.to_string(), v.clone())).collect() }).unwrap();
    }

    #[test]
    fn channels_and_terminals_are_read() {
        assert_eq!(channel("AI-01/3"), Some(("AI-01".into(), 3)));
        assert_eq!(channel("Rack 1/Slot 2/7"), Some(("Rack 1/Slot 2".into(), 7)));
        assert_eq!(channel("AI 01-03"), None);
        assert_eq!(channel("AI-01/0"), None);
        assert_eq!((terminals("1, 2"), terminals("1-4"), terminals("7"), terminals("")), (2, 4, 1, 0));
    }

    #[test]
    fn wiring_adds_up_or_says_why() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        // Free-text wiring and no wiring objects: nothing to say.
        assert!(checks(&d.p).is_empty());
        add(&mut d, JB, "JB-101", &[("terminals", Val::Num(4.0))]);
        add(&mut d, CABLE, "C-101", &[("cable_type", Val::Text("2-pair 1.5 mm², armoured".into())), ("pairs", Val::Num(2.0))]);
        add(&mut d, CARD, "AI-01", &[("io_kind", Val::Text("AI".into())), ("channels", Val::Num(8.0)), ("system", Val::Text("DCS 1".into()))]);
        add(&mut d, CARD, "AO-01", &[("io_kind", Val::Text("AO".into())), ("channels", Val::Num(8.0))]);
        assert!(d.apply(Op::AddObject { class: CABLE.into(), tag: "c-101".into(), data: Default::default() }).is_err(), "tags are unique");
        assert!(d.apply(Op::AddObject { class: "Teapot".into(), tag: "T".into(), data: Default::default() }).is_err());
        let item = |d: &Doc, t: &str| d.p.items.iter().find(|i| d.p.tag_of(i) == t).unwrap().id;
        let set = |d: &mut Doc, t: &str, kv: &[(&str, &str)]| {
            let id = item(d, t);
            for (k, v) in kv {
                d.apply(Op::SetData { ids: vec![id], key: k.to_string(), val: Some(Val::Text(v.to_string())) }).unwrap();
            }
        };
        set(&mut d, "LT-101", &[("jb", "JB-101"), ("jb_terminals", "1, 2"), ("cable", "C-101"), ("io", "AI-01/1")]);
        set(&mut d, "LV-101", &[("jb", "JB-101"), ("jb_terminals", "3, 4"), ("cable", "C-101"), ("io", "AO-01/1")]);
        assert!(checks(&d.p).is_empty(), "{:#?}", checks(&d.p));
        // The schedule and the allocation.
        let c = cables(&d.p);
        assert_eq!(c[0].devices.len(), 2);
        let a = allocation(&d.p);
        assert_eq!(a.len(), 16);
        assert_eq!(a.iter().filter(|r| !r.devices.is_empty()).count(), 2);
        // Wrong: the valve on the AI card's channel 1 (clash, kind), the cable and box overfull,
        // a channel beyond the card, an unknown cable.
        set(&mut d, "LV-101", &[("io", "AI-01/1"), ("jb_terminals", "3-6")]);
        set(&mut d, "PI-102", &[("cable", "C-101"), ("io", "AI-01/9")]);
        set(&mut d, "LIC-101", &[("cable", "C-999")]);
        let w: Vec<String> = checks(&d.p).into_iter().map(|i| i.msg).collect();
        for want in ["share channel 1 of AI-01", "needs an AO channel, but AI-01 is an AI card", "carries 3 devices but has 2 pairs", "has 4 terminals but its devices use 6", "channel 9 of AI-01, which has 8 channels"] {
            assert!(w.iter().any(|m| m.contains(want)), "{want}: {w:#?}");
        }
        // LIC-101 is in the control system: not a wired field device.
        assert!(!w.iter().any(|m| m.contains("C-999")), "{w:#?}");
        set(&mut d, "PI-102", &[("cable", "C-998")]);
        assert!(checks(&d.p).iter().any(|i| i.msg.contains("C-998, which is not in the cable schedule")));
    }
}
