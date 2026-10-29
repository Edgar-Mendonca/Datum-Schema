//! One passing and one failing case for every rule.
use super::*;
use crate::doc::{Doc, Op};
use crate::model::{Item, Line, Val};

pub struct B {
    pub d: Doc,
}

impl B {
    pub fn new() -> Self {
        Self { d: Doc::new(Project::new()) }
    }
    pub fn sheet(&self) -> u32 {
        self.d.p.sheets[0].id
    }
    pub fn item(&mut self, sym: &str, x: f64, y: f64) -> u32 {
        let sheet = self.sheet();
        self.d.apply(Op::AddItem { item: Item { sheet, sym: sym.into(), x, y, ..Item::default() }, on: None, tag: String::new(), obj: None }).unwrap()[0]
    }
    pub fn tag(&mut self, id: u32, t: &str) {
        self.d.apply(Op::SetTag { id, tag: t.into() }).unwrap();
    }
    pub fn line(&mut self, a: (u32, u8), b: (u32, u8)) -> u32 {
        let sheet = self.sheet();
        self.d.apply(Op::AddLine { line: Line { sheet, a: End::Port { item: a.0, port: a.1 }, b: End::Port { item: b.0, port: b.1 }, ..Line::default() } }).unwrap()[0]
    }
    pub fn set(&mut self, ids: &[u32], k: &str, v: Val) {
        self.d.apply(Op::SetData { ids: ids.to_vec(), key: k.into(), val: Some(v) }).unwrap();
    }
    /// Full line data so rule 6 stays quiet.
    pub fn spec(&mut self, ids: &[u32], size: &str, class: &str) {
        for (k, v) in [("size", size), ("fluid", "P"), ("class", class)] {
            self.set(ids, k, Val::Text(v.into()));
        }
    }
    pub fn issues(&self) -> Vec<Issue> {
        let net = Net::new(&self.d.p);
        let hyd = crate::hydraulics::all(&self.d.p, &net);
        run(&self.d.p, &net, &hyd)
    }
    pub fn has(&self, rule: u8) -> bool {
        self.issues().iter().any(|i| i.rule == rule)
    }
}

/// Two vessels joined through an in-line item: (v1, item, v2, line in, line out).
fn through(sym: &str) -> (B, u32, u32, u32) {
    let mut b = B::new();
    let v1 = b.item("vvessel", 50.0, 100.0);
    let it = b.item(sym, 150.0, 108.0);
    let v2 = b.item("vvessel", 250.0, 100.0);
    let l1 = b.line((v1, 5), (it, 0));
    let l2 = b.line((it, 1), (v2, 3));
    b.spec(&[l1], "3", "A1A");
    b.spec(&[l2], "3", "A1A");
    (b, it, l1, l2)
}

#[test]
fn r1_duplicate_tags_and_numbers() {
    let mut b = B::new();
    let a = b.item("vvessel", 50.0, 100.0);
    let c = b.item("vvessel", 150.0, 100.0);
    assert!(!b.has(1));
    b.tag(c, "V-101");
    assert!(b.has(1));
    b.tag(c, "V-102");
    // Two separate lines with the same number.
    let x = b.item("vvessel", 250.0, 100.0);
    let l1 = b.line((a, 5), (c, 3));
    let l2 = b.line((c, 5), (x, 3));
    b.spec(&[l1, l2], "3", "A1A");
    assert!(!b.has(1), "different sequence numbers");
    b.set(&[l2], "seq", Val::Text(crate::model::Val::text(&b.d.p.data_of(b.d.p.line(l1).unwrap().obj)["seq"])));
    assert!(b.has(1));
}

#[test]
fn r2_open_ends() {
    let (mut b, it, l1, _) = through("gate");
    assert!(!b.has(2));
    let sheet = b.sheet();
    b.d.apply(Op::AddLine { line: Line { sheet, a: End::Port { item: it, port: 0 }, b: End::Free { x: 150.0, y: 150.0 }, ..Line::default() } }).unwrap();
    assert!(b.has(2));
    // A valve with one side joined is an open end too; a drain is not.
    let (mut b2, gv, l, _) = through("gate");
    b2.d.apply(Op::Delete { ids: vec![l], keep: false }).unwrap();
    assert!(b2.issues().iter().any(|i| i.rule == 2 && i.ids == vec![gv]));
    let _ = l1;
}

#[test]
fn r3_offpage_partners() {
    let mut b = B::new();
    let a = b.item("offpage", 300.0, 100.0);
    assert!(b.has(3));
    let s2 = b.d.apply(Op::AddSheet { sheet: crate::model::Sheet::default() }).unwrap()[0];
    let c = b.d.apply(Op::AddItem { item: Item { sheet: s2, sym: "offpage".into(), x: 50.0, y: 100.0, ..Item::default() }, on: None, tag: String::new(), obj: None }).unwrap()[0];
    b.d.apply(Op::Link { a, b: c }).unwrap();
    assert!(!b.has(3));
    // Lines on both sides with different numbers.
    let v = b.item("vvessel", 100.0, 100.0);
    let l1 = b.line((v, 5), (a, 0));
    let w = b.d.apply(Op::AddItem { item: Item { sheet: s2, sym: "vvessel".into(), x: 200.0, y: 100.0, ..Item::default() }, on: None, tag: String::new(), obj: None }).unwrap()[0];
    let l2 = b.d.apply(Op::AddLine { line: Line { sheet: s2, a: End::Port { item: c, port: 0 }, b: End::Port { item: w, port: 3 }, ..Line::default() } }).unwrap()[0];
    b.spec(&[l1, l2], "3", "A1A");
    // The line on the other sheet continues the same line number: one object, one run.
    let net = Net::new(&b.d.p);
    assert_eq!(net.run_of[&l1], net.run_of[&l2]);
    assert!(!b.has(3));
    // Given a number of its own, the partners disagree.
    b.d.apply(Op::NewLineObject { line: l2 }).unwrap();
    b.set(&[l2], "seq", Val::Text("9999".into()));
    assert!(b.has(3), "{:?}", b.issues());
}

#[test]
fn r4_size_change_needs_a_reducer() {
    let (mut b, _, _, l2) = through("gate");
    assert!(!b.has(4));
    b.d.apply(Op::NewLineObject { line: l2 }).unwrap();
    b.set(&[l2], "size", Val::Text("4".into()));
    assert!(b.has(4));
    let (mut r, _, _, l2) = through("reducer_con");
    r.d.apply(Op::NewLineObject { line: l2 }).unwrap();
    r.set(&[l2], "size", Val::Text("4".into()));
    assert!(!r.has(4));
}

#[test]
fn r5_class_change_needs_a_spec_break() {
    let (mut b, _, _, l2) = through("gate");
    b.d.apply(Op::NewLineObject { line: l2 }).unwrap();
    b.set(&[l2], "class", Val::Text("B1A".into()));
    assert!(b.has(5));
    let (mut s, _, _, l2) = through("specbreak");
    s.d.apply(Op::NewLineObject { line: l2 }).unwrap();
    s.set(&[l2], "class", Val::Text("B1A".into()));
    assert!(!s.has(5));
}

#[test]
fn r6_missing_tags_and_line_data() {
    let (mut b, gv, l1, _) = through("gate");
    assert!(!b.has(6));
    b.tag(gv, "");
    assert!(b.has(6));
    let (mut c, _, l, _) = through("gate");
    c.d.apply(Op::SetData { ids: vec![l], key: "class".into(), val: None }).unwrap();
    assert!(c.issues().iter().any(|i| i.rule == 6 && i.msg.contains("pipe class")));
    let _ = l1;
}

#[test]
fn r7_flow_arrows_against_each_other() {
    let (mut b, _, _, l2) = through("gate");
    assert!(!b.has(7));
    let l = b.d.p.line(l2).unwrap().clone();
    b.d.apply(Op::SetLine { line: Line { flip: true, ..l } }).unwrap();
    assert!(b.has(7));
}

#[test]
fn r8_pump_discharge_check_valve() {
    let mut b = B::new();
    let pmp = b.item("pump_cent", 50.0, 100.0);
    let v = b.item("vvessel", 250.0, 100.0);
    let l = b.line((pmp, 1), (v, 3));
    assert!(b.has(8));
    let sheet = b.sheet();
    b.d.apply(Op::AddItem { item: Item { sheet, sym: "check".into(), x: 150.0, y: 94.0, ..Item::default() }, on: Some(l), tag: String::new(), obj: None }).unwrap();
    assert!(!b.has(8), "{:?}", b.issues());
}

#[test]
fn r9_relief_path() {
    let mut b = B::new();
    let v = b.item("vvessel", 100.0, 100.0);
    assert!(b.has(9));
    let s = b.item("psv", 100.0, 60.0);
    b.line((v, 0), (s, 0));
    assert!(!b.has(9));
    let mut c = B::new();
    let v = c.item("hdrum", 100.0, 100.0);
    c.set(&[v], "relief", Val::Text("PSV-201 on the overhead line".into()));
    assert!(!c.has(9));
}

#[test]
fn r10_control_valve_isolation() {
    let (mut b, cv, l1, l2) = through("cv_pneu");
    assert!(b.has(10));
    let sheet = b.sheet();
    for (l, x) in [(l1, 110.0), (l2, 190.0)] {
        b.d.apply(Op::AddItem { item: Item { sheet, sym: "gate".into(), x, y: 108.0, ..Item::default() }, on: Some(l), tag: String::new(), obj: None }).unwrap();
    }
    assert!(!b.issues().iter().any(|i| i.rule == 10 && i.ids == vec![cv]), "{:?}", b.issues());
}

#[test]
fn r11_controller_loop() {
    let mut b = B::new();
    let c = b.item("inst_dcs", 100.0, 50.0);
    b.tag(c, "LIC-101");
    assert!(b.has(11));
    let t = b.item("inst_field", 80.0, 50.0);
    b.tag(t, "LT-101");
    assert!(b.has(11), "no final element yet");
    let v = b.item("cv_pneu", 120.0, 100.0);
    b.tag(v, "LV-101");
    assert!(!b.has(11));
}

#[test]
fn r12_letters_match_the_symbol() {
    let mut b = B::new();
    let v = b.item("cv_pneu", 120.0, 100.0);
    b.tag(v, "LV-101");
    assert!(!b.has(12));
    b.tag(v, "LT-101");
    assert!(b.has(12));
    let f = b.item("orifice", 50.0, 50.0);
    b.tag(v, "LV-101");
    b.tag(f, "FE-102");
    b.set(&[f], "loop", Val::Text("103".into()));
    assert!(b.has(12));
}

#[test]
fn r13_hydraulic_limits() {
    let (mut b, _, l1, l2) = through("gate");
    let ids = [l1, l2];
    b.set(&ids, "density", Val::Num(1000.0));
    b.set(&ids, "viscosity", Val::Num(1e-3));
    b.set(&ids, "flow", Val::Num(30.0 / 3600.0));
    assert!(!b.has(13), "1.75 m/s in 3\" is fine");
    b.set(&ids, "flow", Val::Num(80.0 / 3600.0));
    assert!(b.has(13), "4.7 m/s is too fast");
}

#[test]
fn rules_switch_off() {
    let mut b = B::new();
    b.item("vvessel", 100.0, 100.0);
    assert!(b.has(9));
    let mut s = b.d.p.settings.clone();
    s.off.push(9);
    b.d.apply(Op::SetSettings { settings: s }).unwrap();
    assert!(!b.has(9));
}

#[test]
fn r6_required_class_attributes() {
    let mut b = B::new();
    let pm = b.item("pump_cent", 50.0, 100.0);
    assert!(!b.has(6));
    let mut s = b.d.p.settings.clone();
    s.user_attrs.push(crate::classes::UserAttr { class: "Pump".into(), attr: crate::classes::Attr { key: "seal_plan".into(), label: "Seal plan".into(), required: true, ..Default::default() } });
    b.d.apply(Op::SetSettings { settings: s }).unwrap();
    assert!(b.issues().iter().any(|i| i.rule == 6 && i.msg.contains("seal plan")));
    b.set(&[pm], "seal_plan", Val::Text("53B".into()));
    assert!(!b.has(6));
}
