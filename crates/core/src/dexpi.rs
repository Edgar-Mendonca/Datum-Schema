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
use crate::model::{End, Item, Kind, Project, Sheet, Val};
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
    if it.base_key() == "offpage" {
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
    /// A position with y up: `rot` quarter turns clockwise on the sheet turn the Reference (the
    /// symbol's x axis); a mirrored symbol has its Axis pointing down (Z = −1).
    fn pos(&mut self, ind: &str, x: f64, y: f64, rot: u8, flip: bool) {
        let (rx, ry) = [(1, 0), (0, -1), (-1, 0), (0, 1)][(rot % 4) as usize];
        let z = if flip { -1 } else { 1 };
        let _ = writeln!(self.s, "{ind}<Position><Location X=\"{}\" Y=\"{}\" Z=\"0\"/><Axis X=\"0\" Y=\"0\" Z=\"{z}\"/><Reference X=\"{rx}\" Y=\"{ry}\" Z=\"0\"/></Position>", num(x), num(self.h - y));
    }
    /// The object's attributes: DEXPI-named ones in SI units, the rest in our own set.
    fn attrs(&mut self, p: &Project, ind: &str, obj: u32, extra: &[(&str, String)], own_extra: &[(&str, String)]) {
        let defs = p.obj(obj).map(|o| classes::attrs(Some(p), &o.class)).unwrap_or_default();
        let empty = crate::model::Data::new();
        let data = p.obj(obj).map_or(&empty, |o| &o.data);
        let mut dexpi: Vec<(String, String, &str, &str)> = extra.iter().map(|(k, v)| (k.to_string(), v.clone(), "string", "")).collect();
        let mut own: Vec<(String, String, &str, &str)> = own_extra.iter().map(|(k, v)| (k.to_string(), v.clone(), "string", "")).collect();
        for (k, v) in data.iter().filter(|(k, _)| !crate::propagate::hidden(k)) {
            let def = defs.iter().find(|a| a.key == *k);
            let (val, fmt, unit) = match v {
                // Full precision: SI values can be small (a gas viscosity is about 0.00001 Pa·s).
                Val::Num(x) => (x.to_string(), "double", def.map_or("", |a| si_unit(&a.q))),
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

/// Our own attributes that make a round trip exact (other programs may ignore them).
const SYMBOL: &str = "DatumSchemaSymbol";
const KIND: &str = "LineKind";
const FLOW: &str = "FlowDirection";

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Major => "major",
        Kind::Minor => "minor",
        Kind::Utility => "utility",
        Kind::Pneumatic => "pneumatic",
        Kind::Electric => "electric",
        Kind::Data => "data",
        Kind::Capillary => "capillary",
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
/// The exchange is for P&ID sheets.
pub const NOT_PFD: &str = "DEXPI is the P&ID exchange: choose a P&ID sheet. A process flow diagram goes out as DXF, SVG or its stream list.";

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
        x.pos("    ", it.x, it.y, it.rot, it.flip);
        x.attrs(p, "    ", it.obj, &[], &[(SYMBOL, it.base_key())]);
        let sym = symbols::get(&it.sym).expect("a known symbol");
        for (k, port) in sym.ports.iter().enumerate().filter(|(_, q)| !q.signal) {
            let q = place(it, k);
            let _ = writeln!(x.s, "    <Nozzle ID=\"{}\" TagName=\"{}\" ComponentClass=\"Nozzle\">", nozzle_id(it, k), esc(&format!("N{} {}", k + 1, port.name)));
            x.pos("      ", q[0], q[1], 0, false);
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
        x.attrs(p, "    ", so, &[("LineNumberAssignmentClass", number.clone())], &[]);
        for l in lines.iter().filter(|l| l.obj == so) {
            let _ = writeln!(x.s, "    <PipingNetworkSegment ID=\"{}\" ComponentClass=\"PipingNetworkSegment\">", seg_id(l.id));
            x.attrs(p, "      ", 0, &[], &[(KIND, kind_name(l.kind).into()), (FLOW, if l.flip { "reversed" } else { "forward" }.into())]);
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
                x.pos("        ", it.x, it.y, it.rot, it.flip);
                x.attrs(p, "        ", it.obj, &[], &[(SYMBOL, it.base_key())]);
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
            x.pos("        ", it.x, it.y, it.rot, it.flip);
            x.attrs(p, "        ", it.obj, &[], &[(SYMBOL, it.base_key())]);
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
        x.pos("    ", it.x, it.y, it.rot, it.flip);
        let cat: String = letters.chars().take(1).collect();
        let funcs: String = letters.chars().skip(1).collect();
        x.attrs(p, "    ", it.obj, &[("ProcessInstrumentationFunctionCategory", cat), ("ProcessInstrumentationFunctions", funcs), ("ProcessInstrumentationFunctionNumber", n)], &[(SYMBOL, it.base_key())]);
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
        let _ = writeln!(x.s, "{c}");
        // An impulse line's own line number data, then the line type.
        x.attrs(p, "    ", l.obj, &[], &[(KIND, kind_name(l.kind).into())]);
        let mut c = String::from("    ");
        c.push_str("<Connection");
        if let Some(a) = end_id(&l.a) {
            let _ = write!(c, " FromID=\"{a}\"");
        }
        if let Some(b) = end_id(&l.b) {
            let _ = write!(c, " ToID=\"{b}\"");
        }
        c.push_str("/>");
        let _ = writeln!(x.s, "{c}");
        let _ = writeln!(x.s, "  </InformationFlow>");
    }
    let _ = writeln!(x.s, "</PlantModel>");
    x.s
}

// ---------------------------------------------------------------- import

/// One XML element: its name, attributes and children (text is not needed).
#[derive(Default, Debug)]
struct El {
    name: String,
    attrs: Vec<(String, String)>,
    kids: Vec<El>,
}

impl El {
    fn a(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|x| x.0 == k).map(|x| x.1.as_str())
    }
    fn kid(&self, n: &str) -> Option<&El> {
        self.kids.iter().find(|k| k.name == n)
    }
    fn f(&self, k: &str) -> f64 {
        self.a(k).and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| v.is_finite()).unwrap_or(0.0)
    }
}

const MAX_ELEMENTS: usize = 400_000;

/// Parses the document into a tree (at most `MAX_ELEMENTS` elements, 200 levels deep).
fn tree(text: &str) -> Result<El, String> {
    use quick_xml::events::Event;
    let bad = |e: String| format!("This is not a DEXPI file Datum Schema can read (the XML is damaged: {e}).");
    let mut r = quick_xml::Reader::from_str(text);
    let mut stack: Vec<El> = vec![El { name: "#doc".into(), ..El::default() }];
    let mut count = 0usize;
    let open = |e: &quick_xml::events::BytesStart| -> Result<El, String> {
        let mut el = El { name: e.name().0.to_string(), ..El::default() };
        for a in e.attributes() {
            let a = a.map_err(|x| bad(x.to_string()))?;
            let v = a.normalized_value(quick_xml::XmlVersion::Implicit1_0).map_err(|x| bad(x.to_string()))?;
            el.attrs.push((a.key.0.to_string(), crate::clip(&v, 2000)));
        }
        Ok(el)
    };
    loop {
        match r.read_event().map_err(|e| bad(e.to_string()))? {
            Event::Start(e) => {
                count += 1;
                if count > MAX_ELEMENTS || stack.len() > 200 {
                    return Err("That DEXPI file is too large to read.".into());
                }
                stack.push(open(&e)?);
            }
            Event::Empty(e) => {
                count += 1;
                if count > MAX_ELEMENTS {
                    return Err("That DEXPI file is too large to read.".into());
                }
                let el = open(&e)?;
                stack.last_mut().expect("the document").kids.push(el);
            }
            Event::End(_) => {
                let el = stack.pop().ok_or_else(|| bad("an unmatched end tag".into()))?;
                stack.last_mut().ok_or_else(|| bad("an unmatched end tag".into()))?.kids.push(el);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        return Err(bad("an element is not closed".into()));
    }
    let doc = stack.pop().expect("the document");
    doc.kids.into_iter().find(|k| k.name == "PlantModel").ok_or_else(|| "This XML file is not a DEXPI P&ID (it has no PlantModel).".to_string())
}

/// The generic attributes of an element: (set, name, value, format, units).
fn generic(el: &El) -> Vec<(String, String, String, String, String)> {
    el.kids
        .iter()
        .filter(|k| k.name == "GenericAttributes")
        .flat_map(|g| {
            let set = g.a("Set").unwrap_or("").to_string();
            g.kids.iter().filter(|k| k.name == "GenericAttribute").map(move |a| (set.clone(), a.a("Name").unwrap_or("").to_string(), a.a("Value").unwrap_or("").to_string(), a.a("Format").unwrap_or("").to_string(), a.a("Units").unwrap_or("").to_string()))
        })
        .collect()
}

/// A value with its units to SI base units (Pa, K, m³/s…); without units it is taken as SI.
fn to_si(v: f64, units: &str) -> f64 {
    if units.is_empty() {
        return v;
    }
    let u = units.replace("m3", "m^3").replace("m2", "m^2").replace("Pa.s", "Pa*s").replace("degC", "°C").replace("degF", "°F");
    crate::units::expr(&u).map_or(v, |(f, _, off)| f * v + off)
}

/// The symbol for an imported element: our own (round trip), else its class's, else by kind.
fn symbol_for(class: &str, own: Option<&str>, fallback: &str) -> (String, bool) {
    if let Some(s) = own.filter(|s| symbols::get(s).is_some()) {
        return (s.to_string(), true);
    }
    if let Some(s) = classes::get(class).and_then(|c| c.symbols.first()) {
        return (s.clone(), true);
    }
    (fallback.to_string(), false)
}

/// Reads a DEXPI (Proteus XML) P&ID into a new project with one sheet. Returns it with what could
/// not be read exactly, in words.
pub fn read(text: &str) -> Result<(Project, Vec<String>), String> {
    let root = tree(text)?;
    let mut warn: Vec<String> = vec![];
    let mut p = Project::new();
    let sheet_id = p.sheets[0].id;
    // The sheet: name, title, size from the drawing's extent.
    let units = root.kid("PlantInformation").and_then(|i| i.a("Units")).unwrap_or("mm").to_lowercase();
    let k = match units.as_str() {
        "m" | "metre" | "meter" => 1000.0,
        "cm" => 10.0,
        "inch" | "in" => 25.4,
        _ => 1.0,
    };
    let drawing = root.kid("Drawing");
    if let Some(d) = drawing {
        let s = &mut p.sheets[0];
        if let Some(n) = d.a("Name").filter(|n| !n.trim().is_empty()) {
            s.number = crate::clip(n, 60);
        }
        if let Some(t) = d.a("Title").filter(|n| !n.trim().is_empty()) {
            s.title = crate::clip(t, 200);
        }
        if let Some(sz) = d.a("Size").filter(|z| crate::model::SIZES.iter().any(|x| x.0 == *z)) {
            s.size = sz.to_string();
        }
    }
    let ext = drawing.and_then(|d| d.kid("Extent")).and_then(|e| e.kid("Max")).map(|m| (m.f("X") * k, m.f("Y") * k)).filter(|e| e.0 > 1.0 && e.1 > 1.0);
    let (sw, sh) = p.sheets[0].dims();
    let (ew, eh) = ext.unwrap_or((sw, sh));
    // Larger drawings are scaled down to fit the sheet.
    let fit = (sw / ew).min(sh / eh).min(1.0);
    let at = |el: &El| -> Option<([f64; 2], u8, bool)> {
        let pos = el.kid("Position")?;
        let l = pos.kid("Location")?;
        let (x, y) = (l.f("X") * k * fit, (eh - l.f("Y") * k) * fit);
        let rot = pos.kid("Reference").map_or(0, |r| match (r.f("X").round() as i32, r.f("Y").round() as i32) {
            (0, -1) => 1,
            (-1, 0) => 2,
            (0, 1) => 3,
            _ => 0,
        });
        let flip = pos.kid("Axis").is_some_and(|a| a.f("Z") < 0.0);
        Some(([x, y], rot, flip))
    };
    let coord = |c: &El| -> [f64; 2] { [c.f("X") * k * fit, (eh - c.f("Y") * k) * fit] };
    // Objects and their attributes.
    let mut unknown_classes: Vec<String> = vec![];
    let mut fill = |p: &mut Project, obj: u32, el: &El| {
        let class = p.obj(obj).map(|o| o.class.clone()).unwrap_or_default();
        let defs = classes::attrs(Some(p), &class);
        let Some(o) = p.obj_mut(obj) else { return };
        for (set, name, value, format, units) in generic(el) {
            if value.trim().is_empty() || [SYMBOL, KIND, FLOW].contains(&name.as_str()) || name.starts_with("ProcessInstrumentationFunction") || name == "LineNumberAssignmentClass" || name.ends_with("TagNameAssignmentClass") {
                continue;
            }
            let def = defs.iter().find(|a| (!a.dexpi.is_empty() && a.dexpi == name) || a.key == name);
            let key = match (def, set.as_str()) {
                (Some(a), _) => a.key.clone(),
                (None, "DatumSchemaAttributes") => crate::clip(&name, 60),
                (None, _) => format!("dexpi_{}", crate::clip(&name, 60)),
            };
            let numeric = format == "double" || def.is_some_and(|a| !a.q.is_empty());
            let val = match value.trim().parse::<f64>() {
                Ok(x) if numeric && x.is_finite() => Val::Num(if def.is_some_and(|a| !a.q.is_empty()) { to_si(x, &units) } else { x }),
                _ => Val::Text(crate::clip(&value, 1000)),
            };
            o.data.insert(key, val);
        }
    };
    let tag_of = |el: &El| -> String {
        el.a("TagName").map(str::to_string).or_else(|| generic(el).into_iter().find(|g| g.1 == "TagName" || g.1.ends_with("TagNameAssignmentClass")).map(|g| g.2)).unwrap_or_default()
    };
    let own_symbol = |el: &El| -> Option<String> { generic(el).into_iter().find(|g| g.1 == SYMBOL).map(|g| g.2) };
    // Where things went: element ID → (item, its position); nozzle ID → (item, nozzle point).
    let mut items: HashMap<String, u32> = HashMap::new();
    let mut nozzles: HashMap<String, (u32, [f64; 2])> = HashMap::new();
    let mut grid = 0usize;
    let mut place = |p: &mut Project, el: &El, sym: &str, class: &str, tag: &str, fill_attrs: &mut dyn FnMut(&mut Project, u32, &El)| -> u32 {
        let (pt, rot, flip) = at(el).unwrap_or_else(|| {
            grid += 1;
            ([30.0 + (grid % 10) as f64 * 35.0, 30.0 + (grid / 10) as f64 * 35.0], 0, false)
        });
        let obj = p.new_obj(class, tag);
        fill_attrs(p, obj, el);
        let id = p.id();
        p.items.push(Item { id, sheet: sheet_id, sym: sym.into(), x: pt[0], y: pt[1], rot, flip, scale: 1.0, obj, link: 0, base: String::new() });
        id
    };
    let mut equipment: Vec<&El> = vec![];
    fn walk<'a>(el: &'a El, name: &str, out: &mut Vec<&'a El>) {
        for c in &el.kids {
            if c.name == name {
                out.push(c);
            }
            walk(c, name, out);
        }
    }
    for c in root.kids.iter().filter(|c| c.name == "Equipment") {
        equipment.push(c);
        walk(c, "Equipment", &mut equipment);
    }
    for el in &equipment {
        let class = el.a("ComponentClass").unwrap_or("Equipment").to_string();
        let (sym, known) = symbol_for(&class, own_symbol(el).as_deref(), "vvessel");
        if !known {
            unknown_classes.push(class.clone());
        }
        let cls = if classes::get(&class).is_some() { class.clone() } else { classes::class_name_for(&sym) };
        let id = place(&mut p, el, &sym, &cls, &tag_of(el), &mut fill);
        if !known {
            if let Some(o) = p.items.last().map(|i| i.obj).and_then(|o| p.obj_mut(o)) {
                o.data.insert("dexpi_class".into(), Val::Text(class.clone()));
            }
        }
        if let Some(eid) = el.a("ID") {
            items.insert(eid.to_string(), id);
        }
        for n in el.kids.iter().filter(|c| c.name == "Nozzle") {
            if let (Some(nid), Some((q, _, _))) = (n.a("ID"), at(n)) {
                nozzles.insert(nid.to_string(), (id, q));
            }
        }
    }
    // Instrumentation functions.
    let mut pifs: Vec<&El> = vec![];
    walk(&root, "ProcessInstrumentationFunction", &mut pifs);
    for el in &pifs {
        let g = generic(el);
        let get = |n: &str| g.iter().find(|x| x.1 == n).map(|x| x.2.clone()).unwrap_or_default();
        let mut tag = tag_of(el);
        if tag.is_empty() {
            let (c, f, n) = (get("ProcessInstrumentationFunctionCategory"), get("ProcessInstrumentationFunctions"), get("ProcessInstrumentationFunctionNumber"));
            if !c.is_empty() {
                tag = format!("{c}{f}-{n}").trim_end_matches('-').to_string();
            }
        }
        let loc = g.iter().find(|x| x.1 == "location").map(|x| x.2.clone()).unwrap_or_default();
        let fallback = match loc.as_str() { "DCS" => "inst_dcs", "Control room panel" => "inst_panel", "Local panel" => "inst_aux", _ => "inst_field" };
        let (sym, _) = symbol_for("", own_symbol(el).as_deref(), fallback);
        let id = place(&mut p, el, &sym, "Instrument", &tag, &mut fill);
        if let Some(o) = p.items.last().map(|i| i.obj) {
            let c = classes::class_name_for(&sym);
            if let Some(ob) = p.obj_mut(o) {
                ob.class = c;
            }
        }
        if let Some(eid) = el.a("ID") {
            items.insert(eid.to_string(), id);
        }
    }
    // Piping: components first (so connections can find them), then the segments.
    let mut systems: Vec<&El> = vec![];
    walk(&root, "PipingNetworkSystem", &mut systems);
    for sys in &systems {
        for seg in sys.kids.iter().filter(|c| c.name == "PipingNetworkSegment") {
            for c in seg.kids.iter().filter(|c| c.name == "PipingComponent" || c.name == "PipeOffPageConnector" || c.name == "PropertyBreak") {
                let class = c.a("ComponentClass").unwrap_or("PipingComponent").to_string();
                let fallback = if c.name == "PipeOffPageConnector" { "offpage" } else if class.contains("Valve") { "gate" } else { "unknown" };
                let (sym, known) = symbol_for(&class, own_symbol(c).as_deref(), fallback);
                if !known {
                    unknown_classes.push(class.clone());
                }
                let cls = if classes::get(&class).is_some() { class.clone() } else { classes::class_name_for(&sym) };
                let id = place(&mut p, c, &sym, &cls, &tag_of(c), &mut fill);
                if let Some(cid) = c.a("ID") {
                    items.insert(cid.to_string(), id);
                }
            }
        }
    }
    // Every segment's pieces get their ids first, so a branch can name the segment it joins.
    let mut seg_lines: HashMap<String, u32> = HashMap::new();
    struct Piece<'a> {
        seg: &'a El,
        sys: Option<u32>,
        kind: Kind,
        flip: bool,
    }
    let mut pieces: Vec<Piece> = vec![];
    for sys in &systems {
        let unconnected = sys.a("ID") == Some("PNS-UNCONNECTED");
        let obj = if unconnected {
            None
        } else {
            let o = p.new_obj(crate::model::LINE_CLASS, "");
            fill(&mut p, o, sys);
            Some(o)
        };
        for seg in sys.kids.iter().filter(|c| c.name == "PipingNetworkSegment") {
            if unconnected {
                continue;
            }
            let g = generic(seg);
            let kind = match g.iter().find(|x| x.1 == KIND).map(|x| x.2.as_str()) {
                Some("minor") => Kind::Minor,
                Some("utility") => Kind::Utility,
                _ => Kind::Major,
            };
            let flip = g.iter().any(|x| x.1 == FLOW && x.2 == "reversed");
            pieces.push(Piece { seg, sys: obj, kind, flip });
        }
    }
    let mut first_line: Vec<u32> = vec![];
    for pc in &pieces {
        let id = p.id();
        first_line.push(id);
        if let Some(sid) = pc.seg.a("ID") {
            seg_lines.insert(sid.to_string(), id);
        }
    }
    // The flows (signal and impulse lines) also take ids now.
    let mut flows: Vec<&El> = vec![];
    walk(&root, "InformationFlow", &mut flows);
    let mut flow_lines: Vec<u32> = vec![];
    for f in &flows {
        let id = p.id();
        flow_lines.push(id);
        if let Some(fid) = f.a("ID") {
            seg_lines.insert(fid.to_string(), id);
        }
    }
    // The end of a line at an element, near a point: a nozzle, the nearest port of an item, a
    // branch onto a segment, or open.
    let snapshot: HashMap<u32, Item> = p.items.iter().map(|i| (i.id, i.clone())).collect();
    let near_port = |item: u32, q: [f64; 2], process_only: bool| -> Option<u8> {
        let it = snapshot.get(&item)?;
        let s = symbols::get(&it.sym)?;
        let pl = Place::of(it);
        s.ports
            .iter()
            .enumerate()
            .filter(|(_, x)| !process_only || !x.signal)
            .map(|(k, x)| (k as u8, { let r = pl.pt([x.x, x.y]); (r[0] - q[0]).hypot(r[1] - q[1]) }))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    };
    let end_at = |id: Option<&str>, q: [f64; 2], process: bool| -> End {
        let Some(id) = id else { return End::Free { x: q[0], y: q[1] } };
        if let Some(&(item, nq)) = nozzles.get(id) {
            if let Some(port) = near_port(item, nq, true) {
                return End::Port { item, port };
            }
        }
        if let Some(&item) = items.get(id) {
            if let Some(port) = near_port(item, q, process) {
                return End::Port { item, port };
            }
        }
        if let Some(&line) = seg_lines.get(id) {
            return End::Line { line, x: q[0], y: q[1] };
        }
        End::Free { x: q[0], y: q[1] }
    };
    let mut lost = 0usize;
    for (pc, &lid) in pieces.iter().zip(&first_line) {
        let pts: Vec<[f64; 2]> = pc.seg.kid("CenterLine").map(|c| c.kids.iter().filter(|k| k.name == "Coordinate").map(coord).collect()).unwrap_or_default();
        let conn = pc.seg.kid("Connection");
        let (from, to) = (conn.and_then(|c| c.a("FromID")), conn.and_then(|c| c.a("ToID")));
        let (qa, qb) = (pts.first().copied().unwrap_or([0.0, 0.0]), pts.last().copied().unwrap_or([0.0, 0.0]));
        // From → the components inside (not already an end) → To.
        let mids: Vec<u32> = pc.seg.kids.iter().filter(|c| c.name == "PipingComponent" || c.name == "PipeOffPageConnector").filter_map(|c| c.a("ID")).filter(|id| Some(*id) != from && Some(*id) != to).filter_map(|id| items.get(id).copied()).collect();
        let mut nodes: Vec<End> = vec![end_at(from, qa, true)];
        for &m in &mids {
            let s = snapshot.get(&m).and_then(|i| symbols::get(&i.sym));
            let process: Vec<u8> = s.map(|s| s.ports.iter().enumerate().filter(|(_, x)| !x.signal).map(|(k, _)| k as u8).collect()).unwrap_or_default();
            if process.len() >= 2 {
                nodes.push(End::Port { item: m, port: process[0] });
                nodes.push(End::Port { item: m, port: process[1] });
            } else {
                lost += 1;
            }
        }
        nodes.push(end_at(to, qb, true));
        for (i, pair) in nodes.chunks(2).enumerate() {
            let [a, b] = pair else { continue };
            let id = if i == 0 { lid } else { p.id() };
            p.lines.push(crate::model::Line { id, sheet: sheet_id, kind: pc.kind, a: a.clone(), b: b.clone(), pts: vec![], flip: pc.flip, obj: pc.sys.unwrap_or(0) });
        }
    }
    for (f, &lid) in flows.iter().zip(&flow_lines) {
        let pts: Vec<[f64; 2]> = f.kid("CenterLine").map(|c| c.kids.iter().filter(|k| k.name == "Coordinate").map(coord).collect()).unwrap_or_default();
        let conn = f.kid("Connection");
        let (qa, qb) = (pts.first().copied().unwrap_or([0.0, 0.0]), pts.last().copied().unwrap_or([0.0, 0.0]));
        let g = generic(f);
        let measuring = f.a("ComponentClass") == Some("MeasuringLineFunction");
        let kind = match g.iter().find(|x| x.1 == KIND).map(|x| x.2.as_str()) {
            Some("pneumatic") => Kind::Pneumatic,
            Some("data") => Kind::Data,
            Some("capillary") => Kind::Capillary,
            Some("minor") => Kind::Minor,
            Some("electric") => Kind::Electric,
            _ if measuring => Kind::Minor,
            _ => Kind::Electric,
        };
        let a = end_at(conn.and_then(|c| c.a("FromID")), qa, false);
        let b = end_at(conn.and_then(|c| c.a("ToID")), qb, false);
        // An impulse line is piping: a line-number object of its own, with its data.
        let obj = if kind.process() {
            let o = p.new_obj(crate::model::LINE_CLASS, "");
            fill(&mut p, o, f);
            o
        } else {
            0
        };
        p.lines.push(crate::model::Line { id: lid, sheet: sheet_id, kind, a, b, pts: vec![], flip: false, obj });
    }
    if lost > 0 {
        warn.push(format!("{lost} piping components without an inlet and an outlet were placed but not joined into their lines."));
    }
    unknown_classes.sort();
    unknown_classes.dedup();
    if !unknown_classes.is_empty() {
        warn.push(format!("Classes Datum Schema does not know were drawn with a stand-in symbol (their class is kept in the dexpi_class attribute): {}.", unknown_classes.join(", ")));
    }
    if fit < 1.0 {
        warn.push(format!("The drawing was larger than an {} sheet and was scaled to {:.0} % to fit.", p.sheets[0].size, fit * 100.0));
    }
    // A line end on a branch that pointed nowhere, a port that does not exist: normalize repairs.
    warn.extend(p.normalize());
    p.name = p.sheets[0].title.clone();
    Ok((p, warn))
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

    /// Each line between two items as an unordered pair of (tag or symbol, port name).
    fn joins(p: &Project) -> Vec<String> {
        let name = |e: &End| match e {
            End::Port { item, port } => p.item(*item).map(|it| {
                let t = p.tag_of(it);
                let pn = symbols::get(&it.sym).and_then(|s| s.ports.get(*port as usize)).map_or("", |q| q.name);
                format!("{}:{pn}", if t.is_empty() { it.sym.clone() } else { t.to_string() })
            }),
            _ => None,
        };
        let mut v: Vec<String> = p.lines.iter().filter_map(|l| {
            let (a, b) = (name(&l.a)?, name(&l.b)?);
            Some(if a < b { format!("{a} — {b}") } else { format!("{b} — {a}") })
        }).collect();
        v.sort();
        v
    }

    #[test]
    fn every_example_comes_back_from_dexpi() {
        for id in crate::examples::list().iter().map(|e| e.id).filter(|id| *id != "pfd") {
            let p = crate::examples::get(id).unwrap();
            let sheet = p.sheets.iter().find(|s| !s.legend).unwrap();
            let (q, warn) = read(&write(&p, sheet)).unwrap();
            assert!(warn.is_empty(), "{id}: {warn:?}");
            let tags = |p: &Project| { let mut t: Vec<String> = p.objects.iter().filter(|o| !o.tag.is_empty()).map(|o| o.tag.clone()).collect(); t.sort(); t };
            assert_eq!(tags(&q), tags(&p), "{id}: the same tags");
            let numbers = |p: &Project| { let mut n: Vec<String> = p.objects.iter().filter(|o| o.class == crate::model::LINE_CLASS).map(|o| obj_number(p, o.id)).filter(|n| !n.is_empty()).collect(); n.sort(); n.dedup(); n };
            assert_eq!(numbers(&q), numbers(&p), "{id}: the same line numbers");
            assert_eq!(joins(&q), joins(&p), "{id}: the same connections");
            // The items sit where they were, turned and mirrored the same.
            // (Matched by tag and place: "Find the faults" has a duplicate tag on purpose.)
            let mut pairs = vec![];
            for it in &p.items {
                let t = p.tag_of(it);
                if t.is_empty() { continue; }
                let o = q.items.iter().find(|x| q.tag_of(x) == t && (x.x - it.x).abs() < 1e-6 && (x.y - it.y).abs() < 1e-6).unwrap_or_else(|| panic!("{id}: {t} moved"));
                assert!(o.rot == it.rot && o.flip == it.flip && o.sym == it.sym, "{id}: {t}");
                pairs.push((it.obj, o.obj));
            }
            // Data comes back in SI.
            for (a, b) in pairs {
                let (o, back) = (p.obj(a).unwrap(), q.obj(b).unwrap());
                for (k, v) in o.data.iter().filter(|(k, _)| !crate::propagate::hidden(k)) {
                    let w = back.data.get(k).unwrap_or_else(|| panic!("{id}: {} lost {k}", o.tag));
                    match (v, w) {
                        (Val::Num(a), Val::Num(b)) => assert!((a - b).abs() <= 1e-6 * a.abs().max(1.0), "{id}: {} {k} {a} {b}", o.tag),
                        _ => assert_eq!(v.text(), w.text(), "{id}: {} {k}", o.tag),
                    }
                }
            }
            // The checks find what they found before.
            let issues = |p: &Project| { let net = crate::graph::Net::new(p); let h = crate::hydraulics::all(p, &net); let mut v: Vec<(u8, String)> = crate::checks::run(p, &net, &h).into_iter().map(|i| (i.rule, i.msg)).collect(); v.sort(); v };
            assert_eq!(issues(&q), issues(&p), "{id}");
        }
    }

    #[test]
    fn a_foreign_file_with_other_units_and_unknown_classes() {
        let xml = r#"<?xml version="1.0"?>
<PlantModel>
  <PlantInformation Units="m"/>
  <Drawing Name="X-1" Title="Test" Size="A1"><Extent><Min X="0" Y="0"/><Max X="2" Y="1"/></Extent></Drawing>
  <Equipment ID="E1" ComponentClass="CentrifugalPump" TagName="P-1"><Position><Location X="0.2" Y="0.5"/></Position>
    <Nozzle ID="N1"><Position><Location X="0.194" Y="0.5"/></Position></Nozzle>
    <GenericAttributes Set="DexpiAttributes"><GenericAttribute Name="DesignPressure" Value="10" Format="double" Units="bar"/></GenericAttributes>
  </Equipment>
  <Equipment ID="E2" ComponentClass="Teapot" TagName="T-1"><Position><Location X="1" Y="0.5"/></Position></Equipment>
  <PipingNetworkSystem ID="S1">
    <GenericAttributes Set="DexpiAttributes"><GenericAttribute Name="NominalDiameter" Value="4"/></GenericAttributes>
    <PipingNetworkSegment ID="G1"><CenterLine><Coordinate X="0.194" Y="0.5"/><Coordinate X="0.8" Y="0.5"/></CenterLine>
      <PipingComponent ID="V1" ComponentClass="BallValve" TagName="HV-1"><Position><Location X="0.5" Y="0.5"/></Position></PipingComponent>
      <Connection FromID="N1" ToID="E2"/>
    </PipingNetworkSegment>
  </PipingNetworkSystem>
</PlantModel>"#;
        let (q, warn) = read(xml).unwrap();
        assert!(warn.iter().any(|w| w.contains("Teapot")), "{warn:?}");
        assert!(warn.iter().any(|w| w.contains("scaled")), "{warn:?}");
        let pump = q.objects.iter().find(|o| o.tag == "P-1").unwrap();
        assert!(matches!(pump.data.get("design_p"), Some(Val::Num(x)) if (x - 1e6).abs() < 1.0), "{:?}", pump.data);
        assert_eq!(q.sheets[0].size, "A1");
        let hv = q.items.iter().find(|i| q.tag_of(i) == "HV-1").unwrap();
        assert_eq!(hv.sym, "ball");
        // The segment runs pump → valve → teapot: two pieces through the valve, one line number.
        let through: Vec<_> = q.lines.iter().filter(|l| [&l.a, &l.b].iter().any(|e| matches!(e, End::Port { item, .. } if *item == hv.id))).collect();
        assert_eq!(through.len(), 2);
        assert_eq!(through[0].obj, through[1].obj);
        assert_eq!(q.data_of(through[0].obj).get("size"), Some(&Val::Text("4".into())));
    }

    #[test]
    fn damaged_or_foreign_xml_is_refused_politely() {
        assert!(read("not xml <<").is_err());
        assert!(read("<a><b></a>").is_err());
        assert!(read("<Other/>").unwrap_err().contains("PlantModel"));
        let mut r = 11u64;
        for n in [10usize, 500, 20_000] {
            let bytes: Vec<u8> = (0..n).map(|_| { r ^= r << 13; r ^= r >> 7; r ^= r << 17; (r % 96 + 32) as u8 }).collect();
            let _ = read(&String::from_utf8_lossy(&bytes));
        }
        let deep = "<PlantModel>".to_string() + &"<X>".repeat(300) + &"</X>".repeat(300) + "</PlantModel>";
        assert!(read(&deep).is_err());
    }
}
