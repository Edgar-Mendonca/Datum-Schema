//! The symbol library: Datum Schema's own vector drawings, following the conventions of the public
//! symbol standards (ISO 10628-2 for equipment, valves, fittings and lines; ISA-5.1 for instrument
//! bubbles and letter codes). Each symbol is one definition: name, category, size, ports (the
//! connection points, with the direction a line leaves them) and its drawing in a small language
//! (below), from which the engine makes the window's SVG and the DXF blocks.
//!
//! Drawing language, millimetres at 1:1 round the symbol's centre, y down; commands split by `;`:
//! `l x y x y …` polyline · `p …` closed outline · `f …` filled shape · `d …` dashed polyline ·
//! `c cx cy r` circle · `fc cx cy r` filled circle · `a cx cy r a0 a1` arc (degrees, clockwise
//! from +x) · `e cx cy rx ry a0 a1` elliptical arc · `r x y w h` rectangle · `t x y size text`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
pub enum Cat {
    #[default]
    Vessel,
    Rotating,
    Heat,
    Valve,
    Control,
    Fitting,
    Instrument,
    Primary,
    Mark,
    Connector,
}

impl Cat {
    pub const ALL: [Cat; 10] = [Cat::Vessel, Cat::Rotating, Cat::Heat, Cat::Valve, Cat::Control, Cat::Fitting, Cat::Instrument, Cat::Primary, Cat::Mark, Cat::Connector];
    pub fn name(self) -> &'static str {
        match self {
            Cat::Vessel => "Vessels",
            Cat::Rotating => "Rotating equipment",
            Cat::Heat => "Heat transfer",
            Cat::Valve => "Manual valves",
            Cat::Control => "Control and relief valves",
            Cat::Fitting => "Fittings and specials",
            Cat::Instrument => "Instruments",
            Cat::Primary => "Primary elements",
            Cat::Mark => "Line marks",
            Cat::Connector => "Connectors",
        }
    }
    /// Equipment: listed in the equipment list.
    pub fn equipment(self) -> bool {
        matches!(self, Cat::Vessel | Cat::Rotating | Cat::Heat)
    }
    /// Valves: listed in the valve list.
    pub fn valve(self) -> bool {
        matches!(self, Cat::Valve | Cat::Control)
    }
    /// Instruments: listed in the instrument index.
    pub fn instrument(self) -> bool {
        matches!(self, Cat::Instrument | Cat::Primary)
    }
    /// The layer it is drawn on (View › Layers and the DXF layers).
    pub fn layer(self) -> &'static str {
        match self {
            Cat::Instrument | Cat::Primary => "instrument",
            Cat::Mark => "notes",
            _ => "process",
        }
    }
}

/// A connection point: position, the direction a line leaves it (0 right, 1 down, 2 left, 3 up),
/// its name, and whether it takes signals rather than fluid.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Port {
    pub x: f64,
    pub y: f64,
    pub dir: u8,
    pub name: &'static str,
    pub signal: bool,
}

#[derive(Debug, Serialize)]
pub struct Sym {
    pub key: &'static str,
    pub name: &'static str,
    pub cat: Cat,
    pub w: f64,
    pub h: f64,
    pub ports: Vec<Port>,
    #[serde(skip)]
    pub draw: &'static str,
    /// Prefix of automatic tags ("P" → P-101); empty: no tag.
    pub prefix: &'static str,
    /// Sits in a line: two process ports, dropped onto a line it splits it.
    pub inline: bool,
    /// A line may end here without being an open end (battery limit, drain, vent, connectors…).
    pub terminal: bool,
    /// Rare in practice: shown after the others in the palette.
    pub rare: bool,
}

const fn p(x: f64, y: f64, dir: u8, name: &'static str) -> Port {
    Port { x, y, dir, name, signal: false }
}
const fn s(x: f64, y: f64, dir: u8, name: &'static str) -> Port {
    Port { x, y, dir, name, signal: true }
}

/// In-line ports at ±x.
fn io(x: f64) -> Vec<Port> {
    vec![p(-x, 0.0, 2, "in"), p(x, 0.0, 0, "out")]
}
/// The four signal ports of an instrument bubble of radius r.
fn bubble_ports(r: f64) -> Vec<Port> {
    vec![s(0.0, r, 1, "bottom"), s(0.0, -r, 3, "top"), s(-r, 0.0, 2, "left"), s(r, 0.0, 0, "right")]
}

struct Def(&'static str, &'static str, Cat, f64, f64, &'static str, &'static str, u8);
// Flags: 1 in-line, 2 terminal, 4 rare.

const BOWTIE: &str = "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5";

fn defs() -> Vec<(Def, Vec<Port>)> {
    use Cat::*;
    let vport = vec![p(0.0, -19.0, 3, "top"), p(0.0, 19.0, 1, "bottom"), p(-10.0, -8.0, 2, "left upper"), p(-10.0, 8.0, 2, "left lower"), p(10.0, -8.0, 0, "right upper"), p(10.0, 8.0, 0, "right lower")];
    let hport = vec![p(-21.0, 0.0, 2, "left end"), p(21.0, 0.0, 0, "right end"), p(-8.0, -9.0, 3, "top left"), p(8.0, -9.0, 3, "top right"), p(-8.0, 9.0, 1, "bottom left"), p(8.0, 9.0, 1, "bottom right")];
    let tport = vec![p(0.0, -15.0, 3, "roof"), p(-18.0, 10.0, 2, "left lower"), p(18.0, 10.0, 0, "right lower"), p(-18.0, -4.0, 2, "left upper"), p(18.0, -4.0, 0, "right upper"), p(0.0, 15.0, 1, "bottom")];
    let cport = vec![p(0.0, -36.0, 3, "top"), p(0.0, 36.0, 1, "bottom"), p(-8.0, -20.0, 2, "left upper"), p(-8.0, 0.0, 2, "feed"), p(-8.0, 20.0, 2, "left lower"), p(8.0, -20.0, 0, "right upper"), p(8.0, 0.0, 0, "right middle"), p(8.0, 20.0, 0, "right lower")];
    let pump = vec![p(-6.0, 0.0, 2, "suction"), p(8.0, -6.0, 0, "discharge")];
    let lr = |x: f64| vec![p(-x, 0.0, 2, "suction"), p(x, 0.0, 0, "discharge")];
    let hx = vec![p(-6.0, -6.0, 3, "shell in"), p(6.0, 6.0, 1, "shell out"), p(-20.0, 0.0, 2, "tube in"), p(20.0, 0.0, 0, "tube out")];
    let cvp = |y: f64| { let mut v = io(4.0); v.push(s(0.0, y, 3, "signal")); v };
    vec![
        // ---------- vessels
        (Def("vvessel", "Vertical vessel", Vessel, 20.0, 38.0, "l -10 -14 -10 14; l 10 -14 10 14; e 0 -14 10 5 180 360; e 0 14 10 5 0 180", "V", 0), vport.clone()),
        (Def("hdrum", "Horizontal drum", Vessel, 42.0, 18.0, "l -16 -9 16 -9; l -16 9 16 9; e -16 0 5 9 90 270; e 16 0 5 9 -90 90", "V", 0), hport.clone()),
        (Def("tank_cone", "Tank, cone roof", Vessel, 36.0, 30.0, "l -18 -10 -18 15 18 15 18 -10; l -18 -10 0 -15 18 -10", "T", 0), tport.clone()),
        (Def("tank_flat", "Tank, flat roof", Vessel, 36.0, 30.0, "p -18 -15 18 -15 18 15 -18 15", "T", 0), tport),
        (Def("column_trays", "Column with trays", Vessel, 16.0, 72.0, "l -8 -32 -8 32; l 8 -32 8 32; e 0 -32 8 4 180 360; e 0 32 8 4 0 180; d -8 -24 4 -24; d -4 -16 8 -16; d -8 -8 4 -8; d -4 8 8 8; d -8 16 4 16; d -4 24 8 24", "C", 0), cport.clone()),
        (Def("column_packed", "Column with packing", Vessel, 16.0, 72.0, "l -8 -32 -8 32; l 8 -32 8 32; e 0 -32 8 4 180 360; e 0 32 8 4 0 180; l -8 -26 8 -26; l -8 -6 8 -6; l -8 -26 8 -6; l 8 -26 -8 -6; l -8 6 8 6; l -8 26 8 26; l -8 6 8 26; l 8 6 -8 26", "C", 0), cport),
        (Def("separator", "Separator with boot", Vessel, 42.0, 38.0, "l -16 -9 16 -9; l -16 9 -4 9; l 4 9 16 9; e -16 0 5 9 90 270; e 16 0 5 9 -90 90; l -4 9 -4 16; l 4 9 4 16; e 0 16 4 2.5 0 180; d 6 -9 6 9", "V", 0),
            vec![p(-21.0, 0.0, 2, "inlet"), p(21.0, 0.0, 0, "liquid out"), p(8.0, -9.0, 3, "gas out"), p(-8.0, -9.0, 3, "top left"), p(0.0, 18.5, 1, "boot"), p(12.0, 9.0, 1, "bottom right")]),
        (Def("filter", "Filter vessel", Vessel, 16.0, 30.0, "l -8 -11 -8 11; l 8 -11 8 11; e 0 -11 8 4 180 360; e 0 11 8 4 0 180; d -4 -9 -4 9 4 9 4 -9", "F", 0),
            vec![p(0.0, -15.0, 3, "top"), p(0.0, 15.0, 1, "bottom"), p(-8.0, -6.0, 2, "inlet"), p(8.0, 6.0, 0, "outlet")]),
        (Def("ko_drum", "Knock-out drum", Vessel, 20.0, 38.0, "l -10 -14 -10 14; l 10 -14 10 14; e 0 -14 10 5 180 360; e 0 14 10 5 0 180; l -10 -10 10 -10; l -10 -7 10 -7; l -10 -10 -7 -7 -4 -10 -1 -7 2 -10 5 -7 8 -10 10 -8", "V", 0), vport),
        (Def("silo", "Silo", Vessel, 24.0, 40.0, "l -12 -16 -12 6 -2 20 2 20 12 6 12 -16; e 0 -16 12 4 180 360", "SI", 4),
            vec![p(0.0, -20.0, 3, "top"), p(0.0, 20.0, 1, "outlet"), p(-12.0, -6.0, 2, "left"), p(12.0, -6.0, 0, "right")]),
        // ---------- rotating
        (Def("pump_cent", "Centrifugal pump", Rotating, 16.0, 16.0, "c 0 0 6; l 0 -6 8 -6; l -4.5 4 -6 8 6 8 4.5 4", "P", 0), pump),
        (Def("pump_pd", "Positive-displacement pump", Rotating, 14.0, 16.0, "c 0 0 6; c -2.2 0 1.8; c 2.2 0 1.8; l -4.5 4 -6 8 6 8 4.5 4", "P", 0), lr(6.0)),
        (Def("pump_vil", "Vertical in-line pump", Rotating, 16.0, 22.0, "c 0 0 5; l -8 0 -5 0; l 5 0 8 0; l 0 -5 0 -8; r -4 -16 8 8; t 0 -12 3 M", "P", 0), lr(8.0)),
        (Def("comp_cent", "Centrifugal compressor", Rotating, 16.0, 18.0, "p -8 -9 8 -5 8 5 -8 9", "K", 0), lr(8.0)),
        (Def("comp_recip", "Reciprocating compressor", Rotating, 18.0, 14.0, "r -8 -6 16 12; l -4 -6 -4 6; l 4 -6 4 6; r -2 -3 4 6; l 2 0 6 0", "K", 0), lr(8.0)),
        (Def("comp_screw", "Screw compressor", Rotating, 18.0, 14.0, "r -8 -6 16 12; c -3 0 3; c 3 0 3", "K", 0), lr(8.0)),
        (Def("fan", "Fan or blower", Rotating, 16.0, 16.0, "c 0 0 7; l -5 -5 5 5; l -5 5 5 -5; c 0 0 1.5", "B", 0), vec![p(-7.0, 0.0, 2, "inlet"), p(7.0, 0.0, 0, "outlet")]),
        (Def("turbine", "Steam turbine", Rotating, 16.0, 18.0, "p -8 -5 8 -9 8 9 -8 5", "ST", 0), vec![p(-8.0, 0.0, 2, "steam in"), p(8.0, 0.0, 0, "exhaust")]),
        (Def("motor", "Electric motor", Rotating, 12.0, 12.0, "c 0 0 6; t 0 0 5 M", "M", 0), vec![s(0.0, 6.0, 1, "power")]),
        (Def("agitator", "Agitator", Rotating, 14.0, 28.0, "c 0 -10 4; t 0 -10 4 M; l 0 -6 0 12; l -6 10 -2 14; l -2 10 -6 14; l 6 10 2 14; l 2 10 6 14", "AG", 0), vec![]),
        // ---------- heat transfer
        (Def("hx_fixed", "Shell-and-tube exchanger, fixed tubesheet", Heat, 40.0, 12.0, "r -16 -6 32 12; l -16 -6 -16 6; l -20 -6 -16 -6 -16 6 -20 6 -20 -6; l 16 -6 20 -6 20 6 16 6; l -16 -2 16 -2; l -16 2 16 2", "E", 0), hx.clone()),
        (Def("hx_u", "Shell-and-tube exchanger, U-tube", Heat, 40.0, 12.0, "l -16 -6 12 -6; l -16 6 12 6; e 12 0 4 6 -90 90; l -20 -6 -16 -6 -16 6 -20 6 -20 -6; l -16 -2 10 -2; l -16 2 10 2; a 10 0 2 -90 90", "E", 0),
            vec![p(-6.0, -6.0, 3, "shell in"), p(6.0, 6.0, 1, "shell out"), p(-20.0, -3.0, 2, "tube in"), p(-20.0, 3.0, 2, "tube out")]),
        (Def("hx_kettle", "Kettle reboiler", Heat, 44.0, 22.0, "l -20 -4 -14 -4 -8 -11 18 -11; l -20 4 -14 4 -8 11 18 11; e 18 0 4 11 -90 90; l -20 -4 -20 4; l -14 -2 14 -2; l -14 2 14 2; a 14 0 2 -90 90", "E", 0),
            vec![p(-20.0, -2.0, 2, "tube in"), p(-20.0, 2.0, 2, "tube out"), p(0.0, 11.0, 1, "shell in"), p(4.0, -11.0, 3, "vapour out"), p(16.0, 11.0, 1, "liquid out")]),
        (Def("hx_plate", "Plate exchanger", Heat, 16.0, 24.0, "r -8 -12 16 24; l -8 -6 8 -12; l -8 0 8 -6; l -8 6 8 0; l -8 12 8 6", "E", 0),
            vec![p(-8.0, -8.0, 2, "hot in"), p(8.0, 8.0, 0, "hot out"), p(8.0, -8.0, 0, "cold out"), p(-8.0, 8.0, 2, "cold in")]),
        (Def("air_cooler", "Air cooler", Heat, 40.0, 22.0, "r -18 -8 36 6; l -18 -5 18 -5; c 0 7 5; l -5 7 5 7; l 0 2 0 12; l -8 -2 -5 2; l 8 -2 5 2", "E", 0), vec![p(-20.0, -5.0, 2, "inlet"), p(20.0, -5.0, 0, "outlet")]),
        (Def("fired_heater", "Fired heater", Heat, 30.0, 36.0, "l -14 18 -14 -6 -6 -14 6 -14 14 -6 14 18 -14 18; l -6 -14 -6 -18 6 -18 6 -14; l -10 8 -10 -4 -6 -4 -6 8 -2 8 -2 -4 2 -4 2 8 6 8 6 -4 10 -4 10 8; l -6 16 -4 12 -2 16; l 2 16 4 12 6 16", "H", 0),
            vec![p(-14.0, -2.0, 2, "inlet"), p(14.0, -2.0, 0, "outlet"), p(0.0, 18.0, 1, "fuel")]),
        (Def("elec_heater", "Electric heater", Heat, 24.0, 12.0, "r -10 -6 20 12; l -8 0 -6 -3 -3 3 0 -3 3 3 6 -3 8 0", "EH", 0), vec![p(-12.0, 0.0, 2, "inlet"), p(12.0, 0.0, 0, "outlet"), s(0.0, -6.0, 3, "power")]),
        (Def("cooling_tower", "Cooling tower", Heat, 30.0, 32.0, "l -14 16 -9 -12 9 -12 14 16 -14 16; c 0 -14 4; l -4 -14 4 -14; d -10 4 10 4", "CT", 4),
            vec![p(-12.0, 4.0, 2, "water in"), p(14.0, 12.0, 0, "water out")]),
        (Def("jacketed", "Jacketed vessel", Heat, 26.0, 40.0, "l -10 -14 -10 14; l 10 -14 10 14; e 0 -14 10 5 180 360; e 0 14 10 5 0 180; d -13 -10 -13 15 -6 19 6 19 13 15 13 -10", "R", 0),
            vec![p(0.0, -19.0, 3, "top"), p(0.0, 19.0, 1, "bottom"), p(-13.0, -4.0, 2, "jacket in"), p(13.0, 10.0, 0, "jacket out"), p(-10.0, -10.0, 2, "left upper"), p(10.0, -10.0, 0, "right upper")]),
        // ---------- manual valves
        (Def("gate", "Gate valve", Valve, 8.0, 5.0, BOWTIE, "HV", 1), io(4.0)),
        (Def("globe", "Globe valve", Valve, 8.0, 5.0, "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5; fc 0 0 1.3", "HV", 1), io(4.0)),
        (Def("ball", "Ball valve", Valve, 8.0, 5.0, "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5; c 0 0 1.7", "HV", 1), io(4.0)),
        (Def("butterfly", "Butterfly valve", Valve, 8.0, 6.0, "l -4 0 -2 0; l 2 0 4 0; l -2 -3 -2 3; l 2 -3 2 3; l -1.5 2.5 1.5 -2.5; fc 0 0 0.8", "HV", 1), io(4.0)),
        (Def("plug", "Plug valve", Valve, 8.0, 5.0, "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5; f -0.9 -1.6 0.9 -1.6 0.9 1.6 -0.9 1.6", "HV", 1), io(4.0)),
        (Def("needle", "Needle valve", Valve, 8.0, 7.0, "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5; l 0 0 0 -3.5; f -1.2 -3.5 1.2 -3.5 0 -1.5", "HV", 1), io(4.0)),
        (Def("diaphragm", "Diaphragm valve", Valve, 8.0, 7.0, "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5; a 0 -1.2 2.4 180 360", "HV", 1), io(4.0)),
        (Def("check", "Check valve", Valve, 8.0, 5.0, "p -4 -2.5 -4 2.5 3 0; l 3 -2.5 3 2.5; l 3 0 4 0", "HV", 1), io(4.0)),
        (Def("nonslam", "Non-slam check valve", Valve, 8.0, 6.0, "f -4 -2.5 -4 2.5 3 0; l 3 -2.5 3 2.5; l 3 0 4 0; l -1 -2 -1 -3.5 1 -3.5", "HV", 1), io(4.0)),
        (Def("threeway", "Three-way valve", Valve, 8.0, 8.0, "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5; p -2.5 4 0 0 2.5 4", "HV", 0), vec![p(-4.0, 0.0, 2, "a"), p(4.0, 0.0, 0, "b"), p(0.0, 4.0, 1, "c")]),
        // ---------- control and relief
        (Def("cv_pneu", "Control valve, pneumatic", Control, 8.0, 16.0, "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5; l 0 0 0 -5; l -3 -5 3 -5; a 0 -5 3 180 360", "CV", 1), cvp(-8.0)),
        (Def("cv_motor", "Control valve, motor", Control, 8.0, 19.0, "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5; l 0 0 0 -4.5; c 0 -7 2.5; t 0 -7 2.6 M", "CV", 1), cvp(-9.5)),
        (Def("cv_solenoid", "Control valve, solenoid", Control, 8.0, 18.0, "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5; l 0 0 0 -5; r -2.5 -9 5 4; t 0 -7 2.6 S", "XV", 1), cvp(-9.0)),
        (Def("psv", "Pressure safety valve", Control, 10.0, 14.0, "p -2.5 5 2.5 5 0 0; p 5 -2.5 5 2.5 0 0; l 0 0 0 -1.5 -1.8 -2.3 1.8 -3.6 -1.8 -4.9 1.8 -6.2 0 -7; l -2 -7 2 -7", "PSV", 0), vec![p(0.0, 5.0, 1, "inlet"), p(5.0, 0.0, 0, "outlet")]),
        (Def("regulator", "Pressure regulator", Control, 8.0, 14.0, "p -4 -2.5 0 0 -4 2.5; p 4 -2.5 0 0 4 2.5; l 0 0 0 -4; l -3 -4 3 -4; a 0 -4 3 180 360; l 3 -4 6 -4 6 -1", "PCV", 1), io(4.0)),
        (Def("rupture", "Rupture disc", Control, 8.0, 8.0, "l 0 4 0 1; l -3 1 3 1; l -3 -1 3 -1; a 0 1.5 2.6 205 335; l 0 -1 0 -4", "PSE", 0), vec![p(0.0, 4.0, 1, "inlet"), p(0.0, -4.0, 3, "outlet")]),
        (Def("breather", "Breather valve", Control, 10.0, 10.0, "r -4 -4 8 6; l 0 2 0 4; l -2 -1 -2 -3; l -3 -2 -2 -3 -1 -2; l 2 -3 2 -1; l 1 -2 2 -1 3 -2", "PVSV", 6), vec![p(0.0, 4.0, 1, "inlet")]),
        // ---------- fittings and specials
        (Def("reducer_con", "Concentric reducer", Fitting, 6.0, 6.0, "p -3 -3 3 -1.5 3 1.5 -3 3", "", 1), io(3.0)),
        (Def("reducer_ecc", "Eccentric reducer", Fitting, 6.0, 6.0, "p -3 -3 3 -3 3 0 -3 3", "", 1), io(3.0)),
        (Def("blind", "Blind flange", Fitting, 3.0, 7.0, "l -1.5 0 -0.6 0; f -0.6 -3.5 0.6 -3.5 0.6 3.5 -0.6 3.5", "", 2), vec![p(-1.5, 0.0, 2, "in")]),
        (Def("spectacle", "Spectacle blind", Fitting, 4.0, 14.0, "l -2 0 -0.5 0; l 0.5 0 2 0; f -0.5 -3 0.5 -3 0.5 3 -0.5 3; l 0 -3 0 -4; c 0 -5.5 1.5", "", 1), io(2.0)),
        (Def("spacer", "Ring spacer", Fitting, 4.0, 7.0, "l -2 0 -0.6 0; l 0.6 0 2 0; p -0.6 -3.5 0.6 -3.5 0.6 3.5 -0.6 3.5; l 0 -3.5 0 -1; l 0 1 0 3.5", "", 5), io(2.0)),
        (Def("strainer_y", "Strainer, Y-type", Fitting, 8.0, 7.0, "l -4 0 4 0; l -2 0 1 4; l 0 0 3 4; l 1 4 3 4; d -1 1 1.5 3.5", "S", 1), io(4.0)),
        (Def("strainer_basket", "Strainer, basket", Fitting, 8.0, 8.0, "l -4 0 -3 0; l 3 0 4 0; r -3 -3 6 7; d -2 -2 -2 3 2 3 2 -2", "S", 1), io(4.0)),
        (Def("steam_trap", "Steam trap", Fitting, 6.0, 6.0, "c 0 0 2.6; t 0 0 2.8 T", "TR", 1), io(3.0)),
        (Def("sight_glass", "Sight glass", Fitting, 6.0, 6.0, "c 0 0 2.6; l -1.6 -0.8 1.6 -0.8; l -1.6 0.8 1.6 0.8", "SG", 1), io(3.0)),
        (Def("flame_arrester", "Flame arrester", Fitting, 8.0, 6.0, "l -4 0 -3 0; l 3 0 4 0; r -3 -2.5 6 5; l -3 -2.5 3 2.5; l -3 2.5 3 -2.5", "FA", 1), io(4.0)),
        (Def("flex_hose", "Flexible hose", Fitting, 8.0, 4.0, "l -4 0 -3 -1.5 -2 1.5 -1 -1.5 0 1.5 1 -1.5 2 1.5 3 -1.5 4 0", "", 1), io(4.0)),
        (Def("ro", "Restriction orifice", Fitting, 6.0, 13.0, "l -3 0 -0.5 0; l 0.5 0 3 0; l -0.5 -3.5 -0.5 -0.8; l 0.5 -3.5 0.5 -0.8; l -0.5 0.8 -0.5 3.5; l 0.5 0.8 0.5 3.5; t 0 5.5 2 RO", "RO", 1), io(3.0)),
        (Def("exp_joint", "Expansion joint", Fitting, 8.0, 6.0, "l -4 0 -3 0; l 3 0 4 0; l -3 -3 -2 -2 -1 -3 0 -2 1 -3 2 -2 3 -3; l -3 3 -2 2 -1 3 0 2 1 3 2 2 3 3; l -3 -3 -3 3; l 3 -3 3 3", "EJ", 1), io(4.0)),
        (Def("drain", "Drain", Fitting, 5.0, 9.0, "l 0 -4 0 -2.5; p -1.8 -2.5 0 0.5 1.8 -2.5; p -1.8 3.5 0 0.5 1.8 3.5; l 0 3.5 0 4.5; l -1.6 4.5 1.6 4.5", "", 2), vec![p(0.0, -4.0, 3, "in")]),
        (Def("vent", "Vent", Fitting, 6.0, 8.0, "l 0 4 0 -1; a 1.6 -1 1.6 180 360; l 3.2 -1 3.2 1", "", 2), vec![p(0.0, 4.0, 1, "in")]),
        (Def("sample", "Sample point", Fitting, 6.0, 10.0, "l 0 -4.5 0 -3; p -1.6 -3 0 -0.5 1.6 -3; p -1.6 2 0 -0.5 1.6 2; c 0 3.5 1.4", "SC", 2), vec![p(0.0, -4.5, 3, "in")]),
        // ---------- instruments (ISA-5.1 bubbles; the tag is written inside)
        (Def("inst_field", "Instrument, field-mounted", Instrument, 11.0, 11.0, "c 0 0 5.5", "", 0), bubble_ports(5.5)),
        (Def("inst_panel", "Instrument, control-room panel", Instrument, 11.0, 11.0, "c 0 0 5.5; l -5.5 0 5.5 0", "", 0), bubble_ports(5.5)),
        (Def("inst_aux", "Instrument, local panel", Instrument, 11.0, 11.0, "c 0 0 5.5; d -5.5 0 5.5 0", "", 4), bubble_ports(5.5)),
        (Def("inst_dcs", "Shared display (DCS)", Instrument, 11.0, 11.0, "r -5.5 -5.5 11 11; c 0 0 5.5; l -5.5 0 5.5 0", "", 0), bubble_ports(5.5)),
        (Def("inst_plc", "Programmable logic (PLC)", Instrument, 11.0, 11.0, "r -5.5 -5.5 11 11; p 0 -5.5 5.5 0 0 5.5 -5.5 0; l -5.5 0 5.5 0", "", 0), bubble_ports(5.5)),
        (Def("inst_interlock", "Interlock logic", Instrument, 12.0, 12.0, "p 0 -6 6 0 0 6 -6 0", "", 0), bubble_ports(6.0)),
        // ---------- primary elements
        (Def("orifice", "Orifice plate", Primary, 4.0, 8.0, "l -2 0 -0.7 0; l 0.7 0 2 0; l -0.7 -4 -0.7 4; l 0.7 -4 0.7 4", "FE", 1), { let mut v = io(2.0); v.push(s(0.0, -4.0, 3, "tap")); v }),
        (Def("magmeter", "Magnetic flowmeter", Primary, 10.0, 6.0, "l -5 0 -3.5 0; l 3.5 0 5 0; r -3.5 -2.5 7 5; t 0 0 2.6 M", "FE", 1), { let mut v = io(5.0); v.push(s(0.0, -2.5, 3, "signal")); v }),
        (Def("coriolis", "Coriolis flowmeter", Primary, 10.0, 7.0, "l -5 0 -3.5 0; l 3.5 0 5 0; r -3.5 -2.5 7 5; l -2 1.5 -2 -1 2 -1 2 1.5", "FE", 1), { let mut v = io(5.0); v.push(s(0.0, -2.5, 3, "signal")); v }),
        (Def("vortex", "Vortex flowmeter", Primary, 10.0, 6.0, "l -5 0 -3.5 0; l 3.5 0 5 0; r -3.5 -2.5 7 5; l -2.5 0 -1.6 -1 -0.8 0 0 1 0.8 0 1.6 -1 2.5 0", "FE", 1), { let mut v = io(5.0); v.push(s(0.0, -2.5, 3, "signal")); v }),
        (Def("ultrasonic", "Ultrasonic flowmeter", Primary, 10.0, 6.0, "l -5 0 -3.5 0; l 3.5 0 5 0; r -3.5 -2.5 7 5; l -2.5 1.5 2.5 -1.5; l 1.2 -1.8 2.5 -1.5 2 -0.4", "FE", 1), { let mut v = io(5.0); v.push(s(0.0, -2.5, 3, "signal")); v }),
        (Def("thermowell", "Thermowell", Primary, 4.0, 8.0, "p -1 -2 1 -2 1 3 0 4 -1 3; l 0 -2 0 -4", "TW", 0), vec![p(0.0, 4.0, 1, "process"), s(0.0, -4.0, 3, "signal")]),
        // ---------- line marks
        (Def("insulation", "Insulation", Mark, 12.0, 6.0, "d -6 -2 6 -2; d -6 2 6 2; t 0 -4 2 IN", "", 0), vec![]),
        (Def("trace_steam", "Steam tracing", Mark, 12.0, 6.0, "d -6 -2 6 -2; t 0 -4 2 ST", "", 0), vec![]),
        (Def("trace_elec", "Electric tracing", Mark, 12.0, 6.0, "d -6 -2 6 -2; t 0 -4 2 ET", "", 0), vec![]),
        (Def("jacket", "Jacketing", Mark, 12.0, 6.0, "r -6 -2.5 12 5; t 0 -4.5 2 J", "", 4), vec![]),
        (Def("specbreak", "Spec break", Mark, 4.0, 8.0, "l -2 0 2 0; l 0 -4 0 4; l -1.5 -4 1.5 -4; l -1.5 4 1.5 4", "", 1), io(2.0)),
        (Def("battery_limit", "Battery limit", Mark, 4.0, 16.0, "l -2 0 2 0; d 0 -7 0 7; t 0 -8.5 2 BL", "", 3), io(2.0)),
        (Def("tiein", "Tie-in point", Mark, 7.0, 7.0, "p 0 -3.5 3.5 0 0 3.5 -3.5 0; t 0 0 1.8 TI", "TP", 3), io(3.5)),
        (Def("package", "Package boundary", Mark, 60.0, 40.0, "d -30 -20 30 -20 30 20 -30 20 -30 -20", "", 0), vec![]),
        // ---------- connectors
        (Def("offpage", "Off-page connector", Connector, 14.0, 6.0, "p -7 -3 4 -3 7 0 4 3 -7 3", "", 2), vec![p(-7.0, 0.0, 2, "line")]),
        (Def("unknown", "Unknown symbol", Mark, 10.0, 10.0, "r -5 -5 10 10; l -5 -5 5 5; l -5 5 5 -5", "", 4), vec![]),
    ]
}

static LIB: std::sync::OnceLock<Vec<Sym>> = std::sync::OnceLock::new();

/// Every symbol in the library.
pub fn all() -> &'static [Sym] {
    LIB.get_or_init(|| {
        defs()
            .into_iter()
            .map(|(Def(key, name, cat, w, h, draw, prefix, flags), ports)| Sym { key, name, cat, w, h, ports, draw, prefix, inline: flags & 1 != 0, terminal: flags & 2 != 0, rare: flags & 4 != 0 })
            .collect()
    })
}

/// A symbol: one of ours, or a library symbol registered from a client's legend (`symlib`).
pub fn get(key: &str) -> Option<&'static Sym> {
    builtin(key).or_else(|| crate::symlib::get(key))
}

/// One of Datum Schema's own symbols.
pub fn builtin(key: &str) -> Option<&'static Sym> {
    all().iter().find(|s| s.key == key)
}

// ---------------------------------------------------------------- drawing primitives

/// One piece of a drawing in sheet millimetres (y down). The window draws these as SVG and the
/// DXF writer as entities, so both always agree.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "k", rename_all = "lowercase")]
pub enum Prim {
    /// A polyline; `closed` joins the last point to the first, `fill` fills it, `dash` dashes it.
    Poly { pts: Vec<[f64; 2]>, closed: bool, fill: bool, dash: bool, w: f64 },
    Circle { c: [f64; 2], r: f64, fill: bool, w: f64 },
    /// From a0 to a1 degrees, clockwise from +x (y down).
    Arc { c: [f64; 2], r: f64, a0: f64, a1: f64, w: f64 },
    /// Text centred (anchor 1), left (0) or right (2) on its point, upright, `h` mm high; the point
    /// is the middle of the text's height.
    Text { p: [f64; 2], h: f64, s: String, anchor: u8, bold: bool },
}

pub const STROKE: f64 = 0.25;

/// Parses a symbol's drawing.
pub fn parse(draw: &str) -> Vec<Prim> {
    let mut out = vec![];
    for cmd in draw.split(';') {
        let cmd = cmd.trim();
        let (op, rest) = cmd.split_once(' ').unwrap_or((cmd, ""));
        if op == "t" || op == "tl" || op == "tr" {
            let mut w = rest.splitn(4, ' ');
            let n: Vec<f64> = (0..3).filter_map(|_| w.next()?.parse().ok()).collect();
            if n.len() == 3 {
                out.push(Prim::Text { p: [n[0], n[1]], h: n[2], s: w.next().unwrap_or("").to_string(), anchor: match op { "tl" => 0, "tr" => 2, _ => 1 }, bold: false });
            }
            continue;
        }
        let n: Vec<f64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        let pts = || n.as_chunks::<2>().0.to_vec();
        match op {
            "l" | "p" | "f" | "d" => out.push(Prim::Poly { pts: pts(), closed: op == "p" || op == "f", fill: op == "f", dash: op == "d", w: STROKE }),
            "c" | "fc" if n.len() == 3 => out.push(Prim::Circle { c: [n[0], n[1]], r: n[2], fill: op == "fc", w: STROKE }),
            "a" if n.len() == 5 => out.push(Prim::Arc { c: [n[0], n[1]], r: n[2], a0: n[3], a1: n[4], w: STROKE }),
            "e" if n.len() == 6 => {
                // An elliptical arc as a polyline (DXF R12 has no ellipses).
                let k = 16;
                let pts = (0..=k).map(|i| { let a = (n[4] + (n[5] - n[4]) * i as f64 / k as f64).to_radians(); [n[0] + n[2] * a.cos(), n[1] + n[3] * a.sin()] }).collect();
                out.push(Prim::Poly { pts, closed: false, fill: false, dash: false, w: STROKE });
            }
            "r" if n.len() == 4 => out.push(Prim::Poly { pts: vec![[n[0], n[1]], [n[0] + n[2], n[1]], [n[0] + n[2], n[1] + n[3]], [n[0], n[1] + n[3]]], closed: true, fill: false, dash: false, w: STROKE }),
            _ => {}
        }
    }
    out
}

/// A placement: position, quarter turns clockwise, mirror and scale.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub x: f64,
    pub y: f64,
    pub rot: u8,
    pub flip: bool,
    pub scale: f64,
}

impl Place {
    pub fn of(it: &crate::model::Item) -> Self {
        Self { x: it.x, y: it.y, rot: it.rot % 4, flip: it.flip, scale: it.scale }
    }
    /// A local point on the sheet.
    pub fn pt(&self, p: [f64; 2]) -> [f64; 2] {
        let (mut x, mut y) = (p[0] * self.scale, p[1] * self.scale);
        if self.flip {
            x = -x;
        }
        for _ in 0..self.rot {
            (x, y) = (-y, x);
        }
        [self.x + x, self.y + y]
    }
    /// A local direction (0 right, 1 down, 2 left, 3 up) on the sheet.
    pub fn dir(&self, d: u8) -> u8 {
        let d = if self.flip && d.is_multiple_of(2) { (d + 2) % 4 } else { d % 4 };
        (d + self.rot) % 4
    }
    /// A local angle (degrees, clockwise) on the sheet.
    fn ang(&self, a: f64) -> f64 {
        (if self.flip { 180.0 - a } else { a }) + 90.0 * self.rot as f64
    }
    pub fn prims(&self, ps: &[Prim]) -> Vec<Prim> {
        ps.iter()
            .map(|p| match p {
                Prim::Poly { pts, closed, fill, dash, w } => Prim::Poly { pts: pts.iter().map(|q| self.pt(*q)).collect(), closed: *closed, fill: *fill, dash: *dash, w: *w },
                Prim::Circle { c, r, fill, w } => Prim::Circle { c: self.pt(*c), r: r * self.scale, fill: *fill, w: *w },
                Prim::Arc { c, r, a0, a1, w } => {
                    let (b0, b1) = if self.flip { (self.ang(*a1), self.ang(*a0)) } else { (self.ang(*a0), self.ang(*a1)) };
                    Prim::Arc { c: self.pt(*c), r: r * self.scale, a0: b0, a1: b1, w: *w }
                }
                Prim::Text { p, h, s, anchor, bold } => Prim::Text { p: self.pt(*p), h: h * self.scale, s: s.clone(), anchor: *anchor, bold: *bold },
            })
            .collect()
    }
}

/// Unit direction of 0 right, 1 down, 2 left, 3 up.
pub fn dir_vec(d: u8) -> [f64; 2] {
    [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]][(d % 4) as usize]
}

fn n(v: f64) -> String {
    crate::fmt_num((v * 1000.0).round() / 1000.0)
}

/// SVG for a list of primitives (strokes in `currentColor`; the window sets their width).
pub fn svg(ps: &[Prim]) -> String {
    let mut s = String::new();
    for p in ps {
        match p {
            Prim::Poly { pts, closed, fill, dash, .. } => {
                let d: Vec<String> = pts.iter().map(|q| format!("{},{}", n(q[0]), n(q[1]))).collect();
                let tag = if *closed { "polygon" } else { "polyline" };
                s += &format!("<{tag} points=\"{}\"{}{}/>", d.join(" "), if *fill { " class=\"sf\"" } else { "" }, if *dash { " stroke-dasharray=\"1.2 0.8\"" } else { "" });
            }
            Prim::Circle { c, r, fill, .. } => s += &format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{}/>", n(c[0]), n(c[1]), n(*r), if *fill { " class=\"sf\"" } else { "" }),
            Prim::Arc { c, r, a0, a1, .. } => {
                let pt = |a: f64| [c[0] + r * a.to_radians().cos(), c[1] + r * a.to_radians().sin()];
                let (p0, p1) = (pt(*a0), pt(*a1));
                let large = if (a1 - a0).rem_euclid(360.0) > 180.0 { 1 } else { 0 };
                s += &format!("<path d=\"M{} {}A{} {} 0 {large} 1 {} {}\"/>", n(p0[0]), n(p0[1]), n(*r), n(*r), n(p1[0]), n(p1[1]));
            }
            Prim::Text { p, h, s: t, anchor, .. } => s += &format!("<text x=\"{}\" y=\"{}\" font-size=\"{}\" class=\"st\"{}>{}</text>", n(p[0]), n(p[1]), n(*h), match anchor { 0 => " text-anchor=\"start\"", 2 => " text-anchor=\"end\"", _ => "" }, crate::xml(t)),
        }
    }
    s
}

/// What the window's palette and inspector need about each symbol.
#[derive(Serialize)]
pub struct SymInfo {
    pub key: &'static str,
    pub name: &'static str,
    pub cat: Cat,
    pub cat_name: &'static str,
    pub w: f64,
    pub h: f64,
    pub ports: Vec<Port>,
    pub prefix: &'static str,
    pub inline: bool,
    pub terminal: bool,
    pub rare: bool,
    /// The drawing without its text, for the sprite (`<use>`).
    pub svg: String,
    /// The symbol's own letters (M, S, T…), drawn upright by the window.
    pub texts: Vec<Prim>,
    /// The class it draws by default, and that class's attributes.
    pub class: String,
    pub fields: Vec<crate::classes::Attr>,
}

/// Every symbol: ours, then the library symbols registered (their keys start with "x.").
pub fn info() -> Vec<SymInfo> {
    all().iter().chain(crate::symlib::registered()).map(sym_info).collect()
}

pub fn sym_info(s: &'static Sym) -> SymInfo {
    let ps = parse(s.draw);
    let (texts, shapes): (Vec<Prim>, Vec<Prim>) = ps.into_iter().partition(|p| matches!(p, Prim::Text { .. }));
    let class = crate::classes::class_name_for(s.key);
    SymInfo { key: s.key, name: s.name, cat: s.cat, cat_name: s.cat.name(), w: s.w, h: s.h, ports: s.ports.clone(), prefix: s.prefix, inline: s.inline, terminal: s.terminal, rare: s.rare, svg: svg(&shapes), texts, fields: crate::classes::attrs(None, &class), class }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_is_complete_and_sound() {
        let lib = all();
        assert!(lib.len() >= 80, "{} symbols", lib.len());
        let mut keys = std::collections::HashSet::new();
        for s in lib {
            assert!(keys.insert(s.key), "duplicate key {}", s.key);
            let ps = parse(s.draw);
            assert!(!ps.is_empty(), "{} draws nothing", s.key);
            // Every command parsed: as many primitives as commands.
            assert_eq!(ps.len(), s.draw.split(';').count(), "{}: a drawing command did not parse", s.key);
            if s.inline {
                assert!(s.ports.iter().filter(|p| !p.signal).count() == 2, "{} is in-line with other than two process ports", s.key);
            }
            // Ports sit on the symbol's box, so lines meet the drawing.
            for p in &s.ports {
                assert!(p.x.abs() <= s.w / 2.0 + 1e-9 && p.y.abs() <= s.h / 2.0 + 1e-9, "{} port {} outside its box", s.key, p.name);
            }
        }
        for c in Cat::ALL {
            assert!(lib.iter().any(|s| s.cat == c), "no symbols in {c:?}");
        }
    }

    #[test]
    fn placement_turns_and_mirrors() {
        let pl = Place { x: 10.0, y: 20.0, rot: 1, flip: false, scale: 2.0 };
        // (1, 0) turned a quarter clockwise (y down) is (0, 1); doubled and moved.
        assert_eq!(pl.pt([1.0, 0.0]), [10.0, 22.0]);
        assert_eq!(pl.dir(0), 1);
        let f = Place { x: 0.0, y: 0.0, rot: 0, flip: true, scale: 1.0 };
        assert_eq!(f.pt([3.0, 1.0]), [-3.0, 1.0]);
        assert_eq!((f.dir(0), f.dir(1)), (2, 1));
        let r2 = Place { x: 0.0, y: 0.0, rot: 2, flip: false, scale: 1.0 };
        assert_eq!(r2.pt([1.0, 2.0]), [-1.0, -2.0]);
        assert_eq!(r2.dir(3), 1);
    }
}
