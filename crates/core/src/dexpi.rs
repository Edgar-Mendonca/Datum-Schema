//! Export of a sheet in the DEXPI P&ID exchange format (Proteus XML, following the structure of
//! DEXPI 1.3): equipment with its nozzles, piping network systems (one per line number) with their
//! segments, centre lines, piping components and connections, instrumentation functions, and the
//! signal and measuring lines between them as information flows. Attributes go out as generic
//! attributes: those with a DEXPI name in the class library in the "DexpiAttributes" set, in SI
//! units; the rest in a "DatumSchemaAttributes" set. Coordinates are millimetres with y up, as the
//! format expects. This is a first, partial mapping: it is checked here for well-formed XML and
//! consistent references, not yet against other tools.

use crate::classes;
use crate::graph::{instrument_connection, obj_number};
use crate::model::{End, Item, Project, Sheet, Val};
use crate::symbols::{self, Place};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

/// XML text: the five special characters escaped, control characters dropped.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' => {}
            c => o.push(c),
        }
    }
    o
}

fn num(v: f64) -> String {
    crate::fmt_num((v * 1e4).round() / 1e4)
}

/// SI base unit of a quantity, as the generic attribute's Units.
fn si_unit(q: &str) -> &'static str {
    match q {
        "press" | "dp" => "Pa",
        "temp" => "K",
        "flow" => "m3/s",
        "mflow" => "kg/s",
        "density" => "kg/m3",
        "visc" => "Pa.s",
        "length" | "head" | "thk" => "m",
        "velocity" => "m/s",
        "power" => "W",
        "volume" => "m3",
        "area" => "m2",
        _ => "",
    }
}

/// What a symbol is in the exchange model.
#[derive(PartialEq, Clone, Copy)]
enum Role {
    Equipment,
    Component,
    Instrument,
    OffPage,
    Skip,
}

fn role(p: &Project, it: &Item) -> Role {
    let Some(s) = symbols::get(&it.sym) else { return Role::Skip };
    let class = p.obj(it.obj).map(|o| o.class.clone()).unwrap_or_default();
    let chain: Vec<&str> = classes::chain(&class).iter().map(|c| c.name.as_str()).collect();
    if it.sym == "offpage" {
        Role::OffPage
    } else if chain.contains(&"Equipment") {
        Role::Equipment
    } else if s.inline || chain.contains(&"PipingItem") || chain.contains(&"PipingComponent") || chain.contains(&"Valve") {
        Role::Component
    } else if chain.contains(&"Instrument") {
        Role::Instrument
    } else if s.terminal && !s.ports.is_empty() {
        Role::Component
    } else {
        Role::Skip
    }
}

struct X {
    s: String,
    h: f64,
}

impl X {
    fn pos(&mut self, ind: &str, x: f64, y: f64) {
        let _ = writeln!(self.s, "{ind}<Position><Location X=\"{}\" Y=\"{}\" Z=\"0\"/><Axis X=\"0\" Y=\"0\" Z=\"1\"/><Reference X=\"1\" Y=\"0\" Z=\"0\"/></Position>", num(x), num(self.h - y));
    }
    /// The object's attributes: DEXPI-named ones in SI units, the rest in our own set.
    fn attrs(&mut self, p: &Project, ind: &str, obj: u32, extra: &[(&str, String)]) {
        let Some(o) = p.obj(obj) else { return };
        let defs = classes::attrs(Some(p), &o.class);
        let mut dexpi: Vec<(String, String, &str, &str)> = extra.iter().map(|(k, v)| (k.to_string(), v.clone(), "string", "")).collect();
        let mut own: Vec<(String, String, &str, &str)> = vec![];
        for (k, v) in &o.data {
            let def = defs.iter().find(|a| a.key == *k);
            let (val, fmt, unit) = match v {
                Val::Num(x) => (num(*x), "double", def.map_or("", |a| si_unit(&a.q))),
                Val::Text(t) => (t.clone(), "string", ""),
            };
            match def.filter(|a| !a.dexpi.is_empty()) {
                Some(a) => dexpi.push((a.dexpi.clone(), val, fmt, unit)),
                None => own.push((k.clone(), val, fmt, unit)),
            }
        }
        for (set, list) in [("DexpiAttributes", dexpi), ("DatumSchemaAttributes", own)] {
            if list.is_empty() {
                continue;
            }
            let _ = writeln!(self.s, "{ind}<GenericAttributes Set=\"{set}\" Number=\"{}\">", list.len());
            for (n, v, f, u) in list {
                let units = if u.is_empty() { String::new() } else { format!(" Units=\"{u}\"") };
                let _ = writeln!(self.s, "{ind}  <GenericAttribute Name=\"{}\" Value=\"{}\" Format=\"{f}\"{units}/>", esc(&n), esc(&v));
            }
            let _ = writeln!(self.s, "{ind}</GenericAttributes>");
        }
    }
}

fn eq_id(it: &Item) -> String {
    format!("EQ-{}", it.id)
}
fn nozzle_id(it: &Item, k: usize) -> String {
    format!("NZ-{}-{}", it.id, k + 1)
}
fn comp_id(it: &Item) -> String {
    format!("PC-{}", it.id)
}
fn pif_id(it: &Item) -> String {
    format!("PIF-{}", it.id)
}
fn seg_id(l: u32) -> String {
    format!("SEG-{l}")
}

/// One sheet as a DEXPI (Proteus XML) document.
pub fn write(p: &Project, sheet: &Sheet) -> String {
    let (w, h) = sheet.dims();
    let mut x = X { s: String::with_capacity(1 << 16), h };
    let items: Vec<&Item> = p.items.iter().filter(|i| i.sheet == sheet.id).collect();
    let by_id: HashMap<u32, &Item> = items.iter().map(|i| (i.id, *i)).collect();
    let roles: HashMap<u32, Role> = items.iter().map(|i| (i.id, role(p, i))).collect();
    let routes = crate::view::routes(p, sheet.id, None);
    let place = |it: &Item, k: usize| -> [f64; 2] {
        let s = symbols::get(&it.sym).expect("a known symbol");
        Place::of(it).pt([s.ports[k].x, s.ports[k].y])
    };
    // The ID an end of a line connects to.
    let end_id = |e: &End| -> Option<String> {
        match e {
            End::Port { item, port } => {
                let it = by_id.get(item)?;
                match roles.get(item)? {
                    // A signal (a drive's power, a trip) goes to the equipment itself, not a nozzle.
                    Role::Equipment if symbols::get(&it.sym).and_then(|s| s.ports.get(*port as usize)).is_some_and(|q| q.signal) => Some(eq_id(it)),
                    Role::Equipment => Some(nozzle_id(it, *port as usize)),
                    Role::Component | Role::OffPage => Some(comp_id(it)),
                    Role::Instrument => Some(pif_id(it)),
                    Role::Skip => None,
                }
            }
            // A branch: onto a pipe segment, or onto a signal or impulse line (an information flow).
            End::Line { line, .. } => {
                let t = p.line(*line).filter(|t| t.sheet == sheet.id)?;
                Some(if t.kind.process() && !instrument_connection(p, t) { seg_id(t.id) } else { format!("IF-{}", t.id) })
            }
            End::Free { .. } => None,
        }
    };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, secs) = (now / 86_400, now % 86_400);
    // Civil date from days since 1970 (H. Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let y = yoe + era * 400 + i64::from(m <= 2);
    let s = &mut x.s;
    let _ = writeln!(s, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(s, "<PlantModel xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"ProteusPIDSchema_4.1.1.xsd\">");
    let _ = writeln!(s, "  <PlantInformation SchemaVersion=\"4.1.1\" OriginatingSystem=\"Datum Schema\" OriginatingSystemVendor=\"Datum\" OriginatingSystemVersion=\"{}\" Date=\"{y:04}-{m:02}-{d:02}\" Time=\"{:02}:{:02}:{:02}\" Is3D=\"no\" Units=\"mm\" Discipline=\"PID\">", env!("CARGO_PKG_VERSION"), secs / 3600, secs / 60 % 60, secs % 60);
    let _ = writeln!(s, "    <UnitsOfMeasure Distance=\"Millimetre\"/>");
    let _ = writeln!(s, "  </PlantInformation>");
    let _ = writeln!(s, "  <Drawing Name=\"{}\" Type=\"PID\" Title=\"{}\" Size=\"{}\">", esc(&sheet.number), esc(&sheet.title), esc(&sheet.size));
    let _ = writeln!(s, "    <Extent><Min X=\"0\" Y=\"0\"/><Max X=\"{}\" Y=\"{}\"/></Extent>", num(w), num(h));
    let _ = writeln!(s, "  </Drawing>");
    // Equipment, with a nozzle at each process port.
    for it in items.iter().filter(|i| roles[&i.id] == Role::Equipment) {
        let Some(o) = p.obj(it.obj) else { continue };
        let _ = writeln!(x.s, "  <Equipment ID=\"{}\" TagName=\"{}\" ComponentClass=\"{}\">", eq_id(it), esc(&o.tag), esc(&o.class));
        x.pos("    ", it.x, it.y);
        x.attrs(p, "    ", it.obj, &[]);
        let sym = symbols::get(&it.sym).expect("a known symbol");
        for (k, port) in sym.ports.iter().enumerate().filter(|(_, q)| !q.signal) {
            let q = place(it, k);
            let _ = writeln!(x.s, "    <Nozzle ID=\"{}\" TagName=\"{}\" ComponentClass=\"Nozzle\">", nozzle_id(it, k), esc(&format!("N{} {}", k + 1, port.name)));
            x.pos("      ", q[0], q[1]);
            let _ = writeln!(x.s, "    </Nozzle>");
        }
        let _ = writeln!(x.s, "  </Equipment>");
    }
    // Piping: one system per line-number object, one segment per drawn piece. A component sits
    // in the first segment that ends at it (so each component appears once).
    let lines: Vec<_> = p.lines.iter().filter(|l| l.sheet == sheet.id && l.kind.process() && !instrument_connection(p, l)).collect();
    let mut placed: HashSet<u32> = HashSet::new();
    let mut systems: Vec<u32> = vec![];
    for l in &lines {
        if !systems.contains(&l.obj) {
            systems.push(l.obj);
        }
    }
    for so in systems {
        let number = obj_number(p, so);
        let _ = writeln!(x.s, "  <PipingNetworkSystem ID=\"PNS-{so}\" TagName=\"{}\" ComponentClass=\"PipingNetworkSystem\">", esc(&number));
        x.attrs(p, "    ", so, &[("LineNumberAssignmentClass", number.clone())]);
        for l in lines.iter().filter(|l| l.obj == so) {
            let _ = writeln!(x.s, "    <PipingNetworkSegment ID=\"{}\" ComponentClass=\"PipingNetworkSegment\">", seg_id(l.id));
            let path = routes.get(&l.id).cloned().unwrap_or_default();
            if path.len() >= 2 {
                let _ = write!(x.s, "      <CenterLine NumPoints=\"{}\">", path.len());
                for q in &path {
                    let _ = write!(x.s, "<Coordinate X=\"{}\" Y=\"{}\" Z=\"0\"/>", num(q[0]), num(h - q[1]));
                }
                let _ = writeln!(x.s, "</CenterLine>");
            }
            for e in [&l.a, &l.b] {
                let End::Port { item, .. } = e else { continue };
                let Some(it) = by_id.get(item) else { continue };
                if !matches!(roles[item], Role::Component | Role::OffPage) || !placed.insert(it.id) {
                    continue;
                }
                let Some(o) = p.obj(it.obj) else { continue };
                let el = if roles[item] == Role::OffPage { "PipeOffPageConnector" } else { "PipingComponent" };
                let class = if roles[item] == Role::OffPage { "PipeOffPageConnector".to_string() } else { o.class.clone() };
                let _ = writeln!(x.s, "      <{el} ID=\"{}\" TagName=\"{}\" ComponentClass=\"{}\">", comp_id(it), esc(&o.tag), esc(&class));
                x.pos("        ", it.x, it.y);
                x.attrs(p, "        ", it.obj, &[]);
                let _ = writeln!(x.s, "      </{el}>");
            }
            let mut c = String::from("      <Connection");
            if let Some(a) = end_id(&l.a) {
                let _ = write!(c, " FromID=\"{a}\"");
            }
            if let Some(b) = end_id(&l.b) {
                let _ = write!(c, " ToID=\"{b}\"");
            }
            let _ = writeln!(x.s, "{c}/>");
            let _ = writeln!(x.s, "    </PipingNetworkSegment>");
        }
        let _ = writeln!(x.s, "  </PipingNetworkSystem>");
    }
    // Components no line reaches: a piping network system of their own, so they are not lost.
    let loose: Vec<&&Item> = items.iter().filter(|i| matches!(roles[&i.id], Role::Component | Role::OffPage) && !placed.contains(&i.id)).collect();
    if !loose.is_empty() {
        let _ = writeln!(x.s, "  <PipingNetworkSystem ID=\"PNS-UNCONNECTED\" ComponentClass=\"PipingNetworkSystem\">");
        let _ = writeln!(x.s, "    <PipingNetworkSegment ID=\"SEG-UNCONNECTED\" ComponentClass=\"PipingNetworkSegment\">");
        for it in loose {
            let Some(o) = p.obj(it.obj) else { continue };
            let class = if roles[&it.id] == Role::OffPage { "PipeOffPageConnector".to_string() } else { o.class.clone() };
            let _ = writeln!(x.s, "      <PipingComponent ID=\"{}\" TagName=\"{}\" ComponentClass=\"{}\">", comp_id(it), esc(&o.tag), esc(&class));
            x.pos("        ", it.x, it.y);
            let _ = writeln!(x.s, "      </PipingComponent>");
        }
        let _ = writeln!(x.s, "    </PipingNetworkSegment>");
        let _ = writeln!(x.s, "  </PipingNetworkSystem>");
    }
    // Instrumentation functions: letters and loop number from the tag.
    for it in items.iter().filter(|i| roles[&i.id] == Role::Instrument) {
        let Some(o) = p.obj(it.obj) else { continue };
        let (letters, n) = crate::tags::loop_of(&o.tag);
        let _ = writeln!(x.s, "  <ProcessInstrumentationFunction ID=\"{}\" TagName=\"{}\" ComponentClass=\"ProcessInstrumentationFunction\">", pif_id(it), esc(&o.tag));
        x.pos("    ", it.x, it.y);
        let cat: String = letters.chars().take(1).collect();
        let funcs: String = letters.chars().skip(1).collect();
        x.attrs(p, "    ", it.obj, &[("ProcessInstrumentationFunctionCategory", cat), ("ProcessInstrumentationFunctions", funcs), ("ProcessInstrumentationFunctionNumber", n)]);
        let _ = writeln!(x.s, "  </ProcessInstrumentationFunction>");
    }
    // Signal lines, and impulse lines to instruments, as information flows.
    for l in p.lines.iter().filter(|l| l.sheet == sheet.id && (!l.kind.process() || instrument_connection(p, l))) {
        let class = if l.kind.process() { "MeasuringLineFunction" } else { "SignalLineFunction" };
        let mut c = format!("  <InformationFlow ID=\"IF-{}\" ComponentClass=\"{class}\">", l.id);
        let path = routes.get(&l.id).cloned().unwrap_or_default();
        if path.len() >= 2 {
            let _ = write!(c, "<CenterLine NumPoints=\"{}\">", path.len());
            for q in &path {
                let _ = write!(c, "<Coordinate X=\"{}\" Y=\"{}\" Z=\"0\"/>", num(q[0]), num(h - q[1]));
            }
            c.push_str("</CenterLine>");
        }
        c.push_str("<Connection");
        if let Some(a) = end_id(&l.a) {
            let _ = write!(c, " FromID=\"{a}\"");
        }
        if let Some(b) = end_id(&l.b) {
            let _ = write!(c, " ToID=\"{b}\"");
        }
        c.push_str("/></InformationFlow>");
        let _ = writeln!(x.s, "{c}");
    }
    let _ = writeln!(x.s, "</PlantModel>");
    x.s
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use quick_xml::events::Event;

    /// Parses the document; returns (element name → count, every ID, every referenced ID).
    pub(crate) fn parse(text: &str) -> (HashMap<String, usize>, Vec<String>, Vec<String>) {
        let mut r = quick_xml::Reader::from_str(text);
        let (mut count, mut ids, mut refs) = (HashMap::new(), vec![], vec![]);
        loop {
            match r.read_event().expect("well-formed XML") {
                Event::Start(e) | Event::Empty(e) => {
                    *count.entry(e.name().0.to_string()).or_insert(0) += 1;
                    for a in e.attributes() {
                        let a = a.expect("a well-formed attribute");
                        let v = a.normalized_value(quick_xml::XmlVersion::Implicit1_0).expect("escaped").to_string();
                        match a.key.0 {
                            "ID" => ids.push(v),
                            "FromID" | "ToID" => refs.push(v),
                            _ => {}
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        (count, ids, refs)
    }

    #[test]
    fn every_example_exports_well_formed_and_consistent() {
        for id in crate::examples::list().iter().map(|e| e.id) {
            let p = crate::examples::get(id).unwrap();
            for s in &p.sheets {
                let text = write(&p, s);
                let (count, ids, refs) = parse(&text);
                let unique: HashSet<&String> = ids.iter().collect();
                assert_eq!(unique.len(), ids.len(), "{id}: duplicate IDs");
                for r in &refs {
                    assert!(unique.contains(r), "{id}: {r} is referenced but not defined");
                }
                if !s.legend {
                    assert!(count.get("Equipment").copied().unwrap_or(0) >= 1, "{id}: {count:?}");
                    assert!(count.get("PipingNetworkSegment").copied().unwrap_or(0) >= 3, "{id}: {count:?}");
                }
            }
        }
    }

    #[test]
    fn the_pump_transfer_maps_to_dexpi_classes_and_attributes() {
        let p = crate::examples::get("pump").unwrap();
        let text = write(&p, &p.sheets[0]);
        for s in ["ComponentClass=\"CentrifugalPump\"", "TagName=\"P-101A\"", "ComponentClass=\"GateValve\"", "ComponentClass=\"CheckValve\"", "ComponentClass=\"ProcessInstrumentationFunction\"", "Name=\"ProcessInstrumentationFunctionCategory\" Value=\"L\"", "ComponentClass=\"SignalLineFunction\"", "Name=\"DesignPressure\"", "Units=\"Pa\"", "Name=\"LineNumberAssignmentClass\" Value=\"3&quot;-P-1003-A1A\""] {
            assert!(text.contains(s), "missing {s}");
        }
        // Hostile text stays text.
        let mut q = p.clone();
        q.objects[0].tag = "<x & \"y\">".into();
        let (_, _, _) = parse(&write(&q, &q.sheets[0]));
    }
}
