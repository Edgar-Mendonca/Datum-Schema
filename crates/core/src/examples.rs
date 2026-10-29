//! The worked examples (File › Examples), built in code: a pump transfer, a compressor package
//! with an anti-surge loop, a steam heater loop, and "Find the faults" (the pump transfer with
//! planted mistakes for the exhibition demo). Every example opens with 0 errors; the tests hold
//! them to it.

use crate::doc::{Doc, Op};
use crate::model::{End, Item, Kind, Line, Project, Sheet, Val};
use crate::symbols;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Example {
    pub id: &'static str,
    pub name: &'static str,
    pub what: &'static str,
}

pub fn list() -> Vec<Example> {
    vec![
        Example { id: "pump", name: "Pump transfer", what: "A tank to duty and standby pumps with strainers and check valves, to a level-controlled vessel with its relief valve; hydraulics on every line." },
        Example { id: "compressor", name: "Compressor package", what: "Knock-out drum with level control, centrifugal compressor and motor, air cooler, and the anti-surge loop." },
        Example { id: "exchanger", name: "Exchanger loop", what: "A steam heater with its temperature loop, steam trap and condensate return, insulation and tracing." },
        Example { id: "faults", name: "Find the faults", what: "The pump transfer with planted mistakes: open the Checks panel and fix them." },
    ]
}

pub fn get(id: &str) -> Option<Project> {
    match id {
        "pump" => Some(pump().0),
        "compressor" => Some(compressor()),
        "exchanger" => Some(exchanger()),
        "faults" => Some(faults()),
        "big" => Some(big(23)),
        _ => None,
    }
}

/// A small builder over the edit operations, so the examples are made the way a user makes them.
struct X {
    d: Doc,
    s: u32,
}

/// A line end: a named port, a point on a line, or a free point.
enum E<'a> {
    P(u32, &'a str),
    L(u32, f64, f64),
}

impl X {
    fn new(name: &str, number: &str, title: &str) -> Self {
        let mut d = Doc::new(Project::new());
        d.p.name = name.into();
        d.p.title.company = "Datum Process Ltd".into();
        d.p.title.project = name.into();
        d.p.title.client = "Exhibition".into();
        d.p.title.unit = "Unit 100".into();
        d.p.title.by = "EM".into();
        d.p.title.checked = "".into();
        let s = d.p.sheets[0].id;
        if let Some(sh) = d.p.sheets.first_mut() {
            *sh = Sheet { id: s, number: number.into(), title: title.into(), rev: "A".into(), date: "29-Oct-26".into(), revs: vec![crate::model::Rev { rev: "A".into(), date: "29-Oct-26".into(), desc: "Issued for the exhibition".into(), by: "EM".into(), ..Default::default() }], ..Sheet::default() };
        }
        Self { d, s }
    }
    fn op(&mut self, op: Op) -> Vec<u32> {
        self.d.apply(op).unwrap_or_else(|e| panic!("example edit failed: {e}"))
    }
    /// Place a symbol (quarter turns `rot`); an empty tag takes the next free one.
    fn it(&mut self, sym: &str, x: f64, y: f64, rot: u8, tag: &str) -> u32 {
        let s = self.s;
        self.op(Op::AddItem { item: Item { sheet: s, sym: sym.into(), x, y, rot, tag: tag.into(), ..Item::default() }, on: None })[0]
    }
    fn data(&mut self, ids: &[u32], kv: &[(&str, Val)]) {
        for (k, v) in kv {
            self.op(Op::SetData { ids: ids.to_vec(), key: (*k).into(), val: Some(v.clone()) });
        }
    }
    fn end(&self, e: &E) -> End {
        match e {
            E::P(item, name) => {
                let it = self.d.p.item(*item).unwrap();
                let port = symbols::get(&it.sym).unwrap().ports.iter().position(|p| p.name == *name).unwrap_or_else(|| panic!("{} has no port {name}", it.sym));
                End::Port { item: *item, port: port as u8 }
            }
            E::L(line, x, y) => End::Line { line: *line, x: *x, y: *y },
        }
    }
    fn ln(&mut self, a: E, b: E, kind: Kind) -> u32 {
        let (a, b, s) = (self.end(&a), self.end(&b), self.s);
        self.op(Op::AddLine { line: Line { sheet: s, kind, a, b, ..Line::default() } })[0]
    }
    /// A chain of lines through items' "in"/"out" ports: from `a` through each in-line item to `b`.
    fn chain(&mut self, a: E, items: &[u32], b: E, kind: Kind) -> Vec<u32> {
        let mut v = vec![];
        let mut from = a;
        for &it in items {
            v.push(self.ln(from, E::P(it, "in"), kind));
            from = E::P(it, "out");
        }
        v.push(self.ln(from, b, kind));
        v
    }
    /// A pipeline's data on every one of its lines.
    #[allow(clippy::too_many_arguments)]
    fn pipe(&mut self, ids: &[u32], size: &str, fluid: &str, seq: &str, class: &str, service: &str, extra: &[(&str, Val)]) {
        let t = |s: &str| Val::Text(s.into());
        self.data(ids, &[("size", t(size)), ("sched", t("40")), ("fluid", t(fluid)), ("seq", t(seq)), ("class", t(class)), ("service", t(service))]);
        self.data(ids, extra);
    }
    fn done(mut self) -> Project {
        self.d.p.normalize();
        self.d.p
    }
}

fn n(v: f64) -> Val {
    Val::Num(v)
}
fn t(s: &str) -> Val {
    Val::Text(s.into())
}
fn bar(v: f64) -> Val {
    Val::Num(v * 1e5)
}
fn degc(v: f64) -> Val {
    Val::Num(v + 273.15)
}
fn m3h(v: f64) -> Val {
    Val::Num(v / 3600.0)
}

/// Water at 20 °C, 30 m³/h, with design and operating conditions.
fn water(len: f64, oper_p: f64) -> Vec<(&'static str, Val)> {
    vec![("flow", m3h(30.0)), ("density", n(998.0)), ("viscosity", n(1.0e-3)), ("length", n(len)), ("fittings", n(30.0)), ("design_p", bar(10.0)), ("design_t", degc(65.0)), ("oper_p", bar(oper_p)), ("oper_t", degc(20.0)), ("test_p", bar(15.0))]
}

/// Example 1: the pump transfer. Also returns the ids "Find the faults" changes.
fn pump() -> (Project, Faults) {
    let mut x = X::new("Pump transfer", "PID-100", "Pump transfer, tank T-101 to vessel V-101");
    let tank = x.it("tank_cone", 55.0, 130.0, 0, "T-101");
    x.data(&[tank], &[("service", t("Feed tank")), ("volume", n(50.0)), ("design_p", bar(0.07)), ("design_t", degc(65.0)), ("material", t("Carbon steel"))]);
    let breather = x.it("breather", 55.0, 101.0, 0, "PVSV-101");
    let br = x.ln(E::P(tank, "roof"), E::P(breather, "inlet"), Kind::Major);
    x.pipe(&[br], "2", "VA", "1009", "A1A", "Tank breathing", &[]);
    // Suction to the duty pump.
    let (hva, sa, pa) = (x.it("gate", 95.0, 140.0, 0, ""), x.it("strainer_y", 120.0, 140.0, 0, "S-101A"), x.it("pump_cent", 160.0, 140.0, 0, "P-101A"));
    let (hvb, sb, pb) = (x.it("gate", 95.0, 190.0, 0, ""), x.it("strainer_y", 120.0, 190.0, 0, "S-101B"), x.it("pump_cent", 160.0, 190.0, 0, "P-101B"));
    for (p, d) in [(pa, "Transfer pump, duty"), (pb, "Transfer pump, standby")] {
        x.data(&[p], &[("service", t(d)), ("flow", m3h(30.0)), ("head", n(45.0)), ("power", n(7500.0)), ("design_p", bar(10.0)), ("design_t", degc(65.0)), ("material", t("Cast iron"))]);
    }
    let suc_a = x.chain(E::P(tank, "right lower"), &[hva, sa], E::P(pa, "suction"), Kind::Major);
    x.pipe(&suc_a, "4", "P", "1001", "A1A", "Pump A suction", &water(15.0, 0.6));
    let suc_b = x.chain(E::L(suc_a[0], 82.5, 140.0), &[hvb, sb], E::P(pb, "suction"), Kind::Major);
    x.pipe(&suc_b, "4", "P", "1002", "A1A", "Pump B suction", &water(18.0, 0.6));
    // Discharge: check valves and isolation, the level control valve with its isolation, the vessel.
    let (cka, ga) = (x.it("check", 185.0, 134.0, 0, ""), x.it("gate", 205.0, 134.0, 0, ""));
    let (ckb, gb) = (x.it("check", 185.0, 184.0, 0, ""), x.it("gate", 205.0, 184.0, 0, ""));
    let (iso1, lv, iso2) = (x.it("gate", 250.0, 134.0, 0, ""), x.it("cv_pneu", 270.0, 134.0, 0, "LV-101"), x.it("gate", 290.0, 134.0, 0, ""));
    x.data(&[lv], &[("fail", t("FC")), ("size", t("3")), ("class", t("300#")), ("operator", t("Pneumatic")), ("loop", t("101"))]);
    let vessel = x.it("vvessel", 340.0, 142.0, 0, "V-101");
    x.data(&[vessel], &[("service", t("Receiver")), ("volume", n(12.0)), ("design_p", bar(10.0)), ("design_t", degc(65.0)), ("oper_p", bar(4.0)), ("material", t("Carbon steel")), ("size", t("2000 × 4000"))]);
    let dis_a = x.chain(E::P(pa, "discharge"), &[cka, ga, iso1, lv, iso2], E::P(vessel, "left upper"), Kind::Major);
    x.pipe(&dis_a, "3", "P", "1003", "A1A", "Pump discharge to V-101", &water(60.0, 4.5));
    let dis_b = x.chain(E::P(pb, "discharge"), &[ckb, gb], E::L(dis_a[2], 227.5, 134.0), Kind::Major);
    x.pipe(&dis_b, "3", "P", "1004", "A1A", "Pump B discharge", &water(12.0, 4.5));
    // Relief: the safety valve on the vessel's top, to a vent.
    let psv = x.it("psv", 340.0, 108.0, 0, "PSV-101");
    x.data(&[psv], &[("set_p", bar(10.0)), ("size", t("2")), ("class", t("300#"))]);
    let vent = x.it("vent", 365.0, 96.0, 0, "");
    let ri = x.ln(E::P(vessel, "top"), E::P(psv, "inlet"), Kind::Major);
    x.pipe(&[ri], "2", "P", "1005", "A1A", "PSV-101 inlet", &[("design_p", bar(10.0))]);
    let ro = x.ln(E::P(psv, "outlet"), E::P(vent, "in"), Kind::Major);
    x.pipe(&[ro], "3", "P", "1006", "A1A", "PSV-101 outlet to vent", &[]);
    // The vessel's outlet to the battery limit.
    let bl = x.it("battery_limit", 385.0, 175.0, 0, "");
    let out = x.ln(E::P(vessel, "bottom"), E::P(bl, "in"), Kind::Major);
    x.pipe(&[out], "3", "P", "1007", "A1A", "V-101 outlet", &water(40.0, 3.8));
    // The level loop: LT-101 on the vessel, LIC-101 in the control system, LV-101.
    let lt = x.it("inst_field", 375.0, 142.0, 0, "LT-101");
    let lic = x.it("inst_dcs", 375.0, 70.0, 0, "LIC-101");
    x.data(&[lt], &[("function", t("Level transmitter")), ("range", t("0–100 %")), ("signal", t("4–20 mA HART")), ("location", t("Field"))]);
    x.data(&[lic], &[("function", t("Level controller")), ("setpoints", t("SP 50 %, LAH 80 %, LAL 20 %")), ("location", t("DCS"))]);
    x.ln(E::P(vessel, "right upper"), E::P(lt, "left"), Kind::Minor);
    x.ln(E::P(lt, "top"), E::P(lic, "bottom"), Kind::Electric);
    x.ln(E::P(lic, "left"), E::P(lv, "signal"), Kind::Electric);
    // A pressure gauge on the discharge.
    let pi = x.it("inst_field", 225.0, 112.0, 0, "PI-102");
    x.data(&[pi], &[("function", t("Discharge pressure gauge")), ("range", t("0–10 bar"))]);
    x.ln(E::P(pi, "bottom"), E::L(dis_a[2], 225.0, 134.0), Kind::Minor);
    let faults = Faults { pb, ckb, dis_a, lv, psv, ri, ro, vent, out };
    (x.done(), faults)
}

/// What "Find the faults" changes in the pump transfer.
struct Faults {
    pb: u32,
    ckb: u32,
    dis_a: Vec<u32>,
    lv: u32,
    psv: u32,
    ri: u32,
    ro: u32,
    vent: u32,
    out: u32,
}

/// The pump transfer with planted mistakes, one for each kind the checks find:
/// 1 a duplicate tag (both pumps are P-101A); 2 an open end (the vessel outlet stops short of the
/// battery limit); 4 a size change without a reducer (a 2" last piece on the 3" discharge, which is also
/// too fast: 13); 7 flow arrows against each other (a reversed piece); 8 no check valve on pump B;
/// 9 no relief on V-101 (its safety valve was left off); 11 LIC-101 without a final element (the
/// control valve is tagged LV-102).
fn faults() -> Project {
    let (mut p, f) = pump();
    let mut d = Doc::new(std::mem::take(&mut p));
    let ok = |d: &mut Doc, op: Op| {
        d.apply(op).unwrap_or_else(|e| panic!("{e}"));
    };
    ok(&mut d, Op::SetTag { id: f.pb, tag: "P-101A".into() });
    ok(&mut d, Op::Delete { ids: vec![f.ckb] });
    ok(&mut d, Op::Delete { ids: vec![f.psv, f.ri, f.ro, f.vent] });
    let out = d.p.line(f.out).unwrap().clone();
    let at = crate::graph::end_pt(&d.p, &out.b);
    ok(&mut d, Op::SetLine { line: Line { b: End::Free { x: at[0] - 12.0, y: at[1] }, ..out } });
    ok(&mut d, Op::SetData { ids: vec![f.dis_a[5]], key: "size".into(), val: Some(t("2")) });
    let rev = d.p.line(f.dis_a[1]).unwrap().clone();
    ok(&mut d, Op::SetLine { line: Line { flip: true, ..rev } });
    ok(&mut d, Op::SetTag { id: f.lv, tag: "LV-102".into() });
    ok(&mut d, Op::SetData { ids: vec![f.lv], key: "loop".into(), val: Some(t("102")) });
    d.p.name = "Find the faults".into();
    d.p.title.project = "Find the faults".into();
    d.p.sheets[0].title = "Pump transfer, with mistakes to find".into();
    d.p.sheets[0].number = "PID-900".into();
    d.p
}

/// A large drawing for speed tests: `n` copies of the pump transfer tiled on one A1 sheet
/// (22 symbols each, so 23 copies make about 500). Not in the Examples list.
pub fn big(n: usize) -> Project {
    let (one, _) = pump();
    let mut d = Doc::new(one.clone());
    let s = d.p.sheets[0].id;
    let mut sh = d.p.sheets[0].clone();
    sh.size = "A1".into();
    d.apply(Op::SetSheet { sheet: sh }).unwrap();
    let (cols, w, h) = (2, 400.0, 115.0);
    for k in 1..n {
        let (dx, dy) = ((k % cols) as f64 * w, (k / cols) as f64 * h);
        d.apply(Op::Paste { sheet: s, items: one.items.clone(), lines: one.lines.clone(), notes: vec![], dx, dy }).unwrap();
    }
    d.p
}

/// Example 2: the compressor package with its anti-surge loop.
fn compressor() -> Project {
    let mut x = X::new("Compressor package", "PID-200", "Gas compressor package K-201");
    let gas = |len: f64, p: f64| -> Vec<(&'static str, Val)> {
        vec![("phase", t("Gas")), ("mflow", n(9000.0 / 3600.0)), ("density", n(if p > 10.0 { 14.0 } else { 6.5 })), ("viscosity", n(1.2e-5)), ("length", n(len)), ("fittings", n(25.0)), ("design_p", bar(if p > 10.0 { 25.0 } else { 12.0 })), ("design_t", degc(if p > 10.0 { 150.0 } else { 60.0 })), ("oper_p", bar(p)), ("oper_t", degc(if p > 10.0 { 45.0 } else { 30.0 }))]
    };
    // Suction: from the battery limit to the knock-out drum, gas out over the top through the flow element.
    let bl_in = x.it("battery_limit", 25.0, 120.0, 0, "");
    let ko = x.it("ko_drum", 85.0, 130.0, 0, "V-201");
    x.data(&[ko], &[("service", t("Suction knock-out drum")), ("design_p", bar(12.0)), ("design_t", degc(60.0)), ("volume", n(3.0)), ("material", t("Carbon steel"))]);
    let hv_in = x.it("gate", 50.0, 122.0, 0, "");
    let inlet = x.chain(E::P(bl_in, "out"), &[hv_in], E::P(ko, "left upper"), Kind::Major);
    x.pipe(&inlet, "10", "G", "2001", "B1A", "Gas from the header", &gas(30.0, 6.0));
    let fe = x.it("orifice", 140.0, 90.0, 0, "FE-201");
    let k = x.it("comp_cent", 200.0, 120.0, 0, "K-201");
    x.data(&[k], &[("service", t("Gas compressor")), ("flow", n(9000.0 / 3600.0 / 6.5)), ("power", n(450_000.0)), ("design_p", bar(25.0)), ("design_t", degc(150.0))]);
    let m = x.it("motor", 200.0, 150.0, 0, "M-201");
    x.data(&[m], &[("service", t("Compressor drive")), ("power", n(500_000.0))]);
    let suc = x.chain(E::P(ko, "top"), &[fe], E::P(k, "suction"), Kind::Major);
    x.pipe(&suc, "10", "G", "2002", "B1A", "Compressor suction", &gas(20.0, 5.9));
    // Discharge: air cooler, then check valve and isolation to the battery limit.
    let ac = x.it("air_cooler", 255.0, 105.0, 0, "E-201");
    x.data(&[ac], &[("service", t("After-cooler")), ("duty", n(380_000.0)), ("design_p", bar(25.0)), ("design_t", degc(150.0))]);
    let dis = x.ln(E::P(k, "discharge"), E::P(ac, "inlet"), Kind::Major);
    x.pipe(&[dis], "8", "G", "2003", "B2A", "Compressor discharge", &gas(10.0, 14.0));
    let (ck, hv_out, bl_out) = (x.it("check", 310.0, 100.0, 0, ""), x.it("gate", 335.0, 100.0, 0, ""), x.it("battery_limit", 385.0, 100.0, 0, ""));
    let after = x.chain(E::P(ac, "outlet"), &[ck, hv_out], E::P(bl_out, "in"), Kind::Major);
    x.pipe(&after, "8", "G", "2004", "B2A", "Cooled gas to the header", &gas(40.0, 13.8));
    // Anti-surge recycle: from before the check valve, through UV-201, back to the drum's inlet.
    let (g1, uv, g2) = (x.it("gate", 300.0, 60.0, 2, ""), x.it("cv_pneu", 250.0, 60.0, 2, "UV-201"), x.it("gate", 200.0, 60.0, 2, ""));
    x.data(&[uv], &[("fail", t("FO")), ("size", t("6")), ("class", t("300#")), ("loop", t("201")), ("service", t("Anti-surge recycle"))]);
    let rec = x.chain(E::L(after[0], 290.0, 100.0), &[g1, uv, g2], E::L(inlet[1], 65.0, 122.0), Kind::Major);
    x.pipe(&rec, "6", "G", "2005", "B2A", "Anti-surge recycle", &[("design_p", bar(25.0)), ("design_t", degc(150.0))]);
    // Drum level: LT-201, LIC-201 and LV-201 on the liquid drain, isolated, to the drain header.
    let (d1, lv, d2, bl_d) = (x.it("gate", 85.0, 185.0, 0, ""), x.it("cv_pneu", 110.0, 185.0, 0, "LV-201"), x.it("gate", 135.0, 185.0, 0, ""), x.it("battery_limit", 165.0, 185.0, 0, ""));
    x.data(&[lv], &[("fail", t("FC")), ("loop", t("201")), ("size", t("1"))]);
    let drain = x.chain(E::P(ko, "bottom"), &[d1, lv, d2], E::P(bl_d, "in"), Kind::Minor);
    x.pipe(&drain, "1", "D", "2006", "B1A", "Drum liquid to drain", &[("design_p", bar(12.0))]);
    let psv = x.it("psv", 120.0, 132.0, 0, "PSV-201");
    x.data(&[psv], &[("set_p", bar(12.0))]);
    let vent = x.it("vent", 140.0, 122.0, 0, "");
    let ri = x.ln(E::P(ko, "right upper"), E::P(psv, "inlet"), Kind::Major);
    x.pipe(&[ri], "2", "G", "2007", "B1A", "PSV-201 inlet", &[]);
    let ro = x.ln(E::P(psv, "outlet"), E::P(vent, "in"), Kind::Major);
    x.pipe(&[ro], "3", "G", "2008", "B1A", "PSV-201 outlet to flare", &[]);
    let lt = x.it("inst_field", 55.0, 160.0, 0, "LT-201");
    let lic = x.it("inst_dcs", 55.0, 200.0, 0, "LIC-201");
    x.ln(E::P(ko, "left lower"), E::P(lt, "right"), Kind::Minor);
    x.ln(E::P(lt, "bottom"), E::P(lic, "top"), Kind::Electric);
    x.ln(E::P(lic, "right"), E::P(lv, "signal"), Kind::Electric);
    // The anti-surge loop: FT on the flow element, PT and TT on the suction, UIC to UV-201.
    let ft = x.it("inst_field", 140.0, 65.0, 0, "FT-201");
    let pt = x.it("inst_field", 165.0, 65.0, 0, "PT-201");
    let tt = x.it("inst_field", 180.0, 140.0, 0, "TT-201");
    let uic = x.it("inst_dcs", 165.0, 30.0, 0, "UIC-201");
    x.data(&[uic], &[("function", t("Anti-surge controller")), ("location", t("DCS"))]);
    x.ln(E::P(fe, "tap"), E::P(ft, "bottom"), Kind::Minor);
    x.ln(E::P(pt, "bottom"), E::L(suc[1], 165.0, 90.0), Kind::Minor);
    x.ln(E::P(tt, "top"), E::L(suc[1], 180.0, 120.0), Kind::Minor);
    for (i, port) in [(ft, "top"), (pt, "top"), (tt, "right")] {
        x.ln(E::P(i, port), E::P(uic, if i == tt { "right" } else { "bottom" }), Kind::Electric);
    }
    x.ln(E::P(uic, "left"), E::P(uv, "signal"), Kind::Electric);
    x.ln(E::P(m, "power"), E::L(dis, 215.0, 114.0), Kind::Data);
    x.done()
}

/// Example 3: the steam heater loop.
fn exchanger() -> Project {
    let mut x = X::new("Exchanger loop", "PID-300", "Process heater E-301 on steam");
    let e = x.it("hx_fixed", 230.0, 140.0, 0, "E-301");
    x.data(&[e], &[("service", t("Process heater")), ("duty", n(620_000.0)), ("area", n(24.0)), ("design_p", bar(12.0)), ("design_t", degc(200.0)), ("material", t("CS shell, SS tubes"))]);
    // Process water through the tubes.
    let (bl1, h1) = (x.it("battery_limit", 25.0, 140.0, 0, ""), x.it("gate", 60.0, 140.0, 0, ""));
    let pin = x.chain(E::P(bl1, "out"), &[h1], E::P(e, "tube in"), Kind::Major);
    let hot = |len: f64, tc: f64| -> Vec<(&'static str, Val)> { vec![("flow", m3h(25.0)), ("density", n(985.0)), ("viscosity", n(0.5e-3)), ("length", n(len)), ("fittings", n(30.0)), ("design_p", bar(10.0)), ("design_t", degc(120.0)), ("oper_p", bar(5.0)), ("oper_t", degc(tc))] };
    x.pipe(&pin, "3", "P", "3001", "A1A", "Process water to E-301", &hot(30.0, 40.0));
    let (h2, bl2) = (x.it("gate", 330.0, 140.0, 0, ""), x.it("battery_limit", 385.0, 140.0, 0, ""));
    let pout = x.chain(E::P(e, "tube out"), &[h2], E::P(bl2, "in"), Kind::Major);
    x.pipe(&pout, "3", "P", "3002", "A1A", "Heated water", &hot(40.0, 90.0));
    x.data(&pout, &[("ins_type", t("Hot (heat conservation)")), ("ins_thk", n(0.05)), ("tracing", t("Steam"))]);
    x.it("insulation", 300.0, 133.0, 0, "");
    x.it("trace_steam", 355.0, 133.0, 0, "");
    // Steam to the shell through TV-301 and its isolation; the shell's safety valve.
    let (bl3, s1, tv, s2) = (x.it("battery_limit", 25.0, 70.0, 0, ""), x.it("gate", 80.0, 70.0, 0, ""), x.it("cv_pneu", 110.0, 70.0, 0, "TV-301"), x.it("gate", 140.0, 70.0, 0, ""));
    x.data(&[tv], &[("fail", t("FC")), ("loop", t("301")), ("size", t("3")), ("class", t("300#"))]);
    let steam = x.chain(E::P(bl3, "out"), &[s1, tv, s2], E::P(e, "shell in"), Kind::Utility);
    let st = |len: f64| -> Vec<(&'static str, Val)> { vec![("phase", t("Steam")), ("mflow", n(1100.0 / 3600.0)), ("density", n(5.1)), ("viscosity", n(1.5e-5)), ("length", n(len)), ("fittings", n(30.0)), ("design_p", bar(12.0)), ("design_t", degc(200.0)), ("oper_p", bar(9.0)), ("oper_t", degc(180.0))] };
    x.pipe(&steam, "3", "LS", "3003", "B1A", "Steam to E-301", &st(35.0));
    let psv = x.it("psv", 185.0, 50.0, 0, "PSV-301");
    x.data(&[psv], &[("set_p", bar(12.0))]);
    let vent = x.it("vent", 205.0, 38.0, 0, "");
    let ri = x.ln(E::L(steam[3], 185.0, 70.0), E::P(psv, "inlet"), Kind::Utility);
    x.pipe(&[ri], "1-1/2", "LS", "3005", "B1A", "PSV-301 inlet", &[]);
    let ro = x.ln(E::P(psv, "outlet"), E::P(vent, "in"), Kind::Utility);
    x.pipe(&[ro], "2", "LS", "3006", "B1A", "PSV-301 outlet to atmosphere", &[]);
    // Condensate through the steam trap to the return header.
    let (trap, g3, bl4) = (x.it("steam_trap", 265.0, 200.0, 0, "TR-301"), x.it("gate", 300.0, 200.0, 0, ""), x.it("battery_limit", 385.0, 200.0, 0, ""));
    let cond = x.chain(E::P(e, "shell out"), &[trap, g3], E::P(bl4, "in"), Kind::Utility);
    x.pipe(&cond, "1", "C", "3004", "B1A", "Condensate return", &[("flow", m3h(1.15)), ("density", n(950.0)), ("viscosity", n(0.28e-3)), ("length", n(25.0)), ("design_p", bar(12.0)), ("design_t", degc(200.0))]);
    // The temperature loop on the outlet.
    let tw = x.it("thermowell", 300.0, 152.0, 2, "TW-301");
    x.ln(E::P(tw, "process"), E::L(pout[1], 300.0, 140.0), Kind::Minor);
    let tt = x.it("inst_field", 300.0, 175.0, 0, "TT-301");
    let tic = x.it("inst_dcs", 110.0, 30.0, 0, "TIC-301");
    x.data(&[tic], &[("function", t("Outlet temperature controller")), ("setpoints", t("SP 90 °C, TAH 105 °C"))]);
    x.ln(E::P(tw, "signal"), E::P(tt, "top"), Kind::Capillary);
    x.ln(E::P(tt, "left"), E::P(tic, "left"), Kind::Electric);
    x.ln(E::P(tic, "bottom"), E::P(tv, "signal"), Kind::Electric);
    let ti = x.it("inst_field", 60.0, 165.0, 0, "TI-302");
    x.ln(E::P(ti, "top"), E::L(pin[0], 45.0, 140.0), Kind::Minor);
    x.done()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{self, Issue};
    use crate::graph::Net;

    fn issues(p: &Project) -> Vec<Issue> {
        let net = Net::new(p);
        let hyd = crate::hydraulics::all(p, &net);
        checks::run(p, &net, &hyd)
    }

    #[test]
    fn every_example_opens_with_no_errors_or_warnings() {
        for ex in list().iter().filter(|e| e.id != "faults") {
            let p = get(ex.id).unwrap();
            let is = issues(&p);
            let bad: Vec<String> = is.iter().filter(|i| i.level != "note").map(|i| format!("{} {}", i.rule, i.msg)).collect();
            assert!(bad.is_empty(), "{}: {bad:#?}", ex.name);
            // Hydraulics on every pipeline that has a flow, all within their limits.
            let net = Net::new(&p);
            let hyd = crate::hydraulics::all(&p, &net);
            assert!(hyd.iter().flatten().count() >= 3, "{}", ex.name);
            assert!(hyd.iter().flatten().all(|h| h.status == 0));
            // It opens again from its file unchanged.
            let (q, w) = crate::model::from_json(&crate::model::to_json(&p)).unwrap();
            assert!(w.is_empty() && q == p);
        }
    }

    #[test]
    fn every_example_prints_and_exports() {
        for ex in list() {
            let p = get(ex.id).unwrap();
            for s in &p.sheets {
                let v = crate::view::sheet(&p, s, &Net::new(&p), &[]);
                assert!(!v.items.is_empty() && !v.frame.is_empty(), "{}", ex.name);
                let (layers, blocks, ents) = crate::dxf::read_back(&crate::dxf::write(&p, s)).unwrap();
                assert!(layers.len() > 10 && !blocks.is_empty() && ents["INSERT"] == v.items.len(), "{}", ex.name);
            }
            for (k, _) in crate::lists::KINDS {
                let l = crate::lists::build(&p, k).unwrap();
                assert!(!crate::lists::csv(&l).is_empty());
                assert_eq!(&crate::lists::xlsx(&p, k).unwrap()[..2], b"PK");
            }
        }
    }

    #[test]
    fn find_the_faults_has_exactly_its_planted_faults() {
        let p = get("faults").unwrap();
        let mut rules: Vec<u8> = issues(&p).iter().filter(|i| i.level != "note").map(|i| i.rule).collect();
        rules.sort_unstable();
        assert_eq!(rules, vec![1, 2, 4, 7, 8, 9, 11, 13], "{:#?}", issues(&p));
    }

    #[test]
    fn pump_transfer_lines_and_lists() {
        let p = get("pump").unwrap();
        let l = crate::lists::build(&p, "lines").unwrap();
        let numbers: Vec<&str> = l.rows.iter().map(|r| r.cells[0].as_str()).collect();
        assert!(numbers.contains(&"4\"-P-1001-A1A") && numbers.contains(&"3\"-P-1003-A1A"), "{numbers:?}");
        let row = l.rows.iter().find(|r| r.cells[0] == "3\"-P-1003-A1A").unwrap();
        let col = |k: &str| l.cols.iter().position(|c| c.key == k).unwrap();
        assert_eq!(row.cells[col("from")], "P-101A discharge");
        assert_eq!(row.cells[col("to")], "V-101 left upper");
        let v = crate::lists::build(&p, "valves").unwrap();
        assert!(v.rows.len() >= 10);
        let i = crate::lists::build(&p, "instruments").unwrap();
        assert!(i.rows.iter().any(|r| r.cells[0] == "LIC-101"));
    }
}
