//! The data each kind of thing carries: equipment, valves, instruments and lines. Numbers are kept
//! in SI base units; `q` names the quantity so the window shows and reads them in SI or US units
//! (`units::QUANTITIES`). Text fields have `q` empty; `choices` offers a list.

use crate::symbols::Cat;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub q: &'static str,
    pub choices: Vec<&'static str>,
}

fn f(key: &'static str, label: &'static str, q: &'static str) -> Field {
    Field { key, label, q, choices: vec![] }
}
fn c(key: &'static str, label: &'static str, choices: &[&'static str]) -> Field {
    Field { key, label, q: "", choices: choices.to_vec() }
}

pub const SIGNALS: &[&str] = &["4–20 mA", "4–20 mA HART", "Fieldbus", "Pneumatic 0.2–1 bar", "Discrete", "Software link", "Capillary"];
pub const LOCATIONS: &[&str] = &["Field", "Local panel", "Control room panel", "DCS", "PLC"];
pub const POSITIONS: &[&str] = &["NO", "NC", "LO", "LC"];
pub const FAIL: &[&str] = &["", "FO", "FC", "FL"];
pub const OPERATORS: &[&str] = &["Handwheel", "Lever", "Gear", "Pneumatic", "Motor", "Solenoid", "Self-acting"];
/// Line services for the hydraulic limits (`model::default_limits`); "auto" picks from the
/// connections (pump suction or discharge) and the phase.
pub const SERVICES: &[&str] = &["auto", "liquid", "suction", "discharge", "drain", "gas", "steam", "twophase"];
pub const PHASES: &[&str] = &["Liquid", "Gas", "Steam", "Two-phase"];

/// The equipment fields common to every vessel, machine and exchanger, then the category's own.
pub fn for_cat(cat: Cat, key: &str) -> Vec<Field> {
    let mut v = vec![];
    match cat {
        Cat::Vessel | Cat::Rotating | Cat::Heat => {
            v.push(f("service", "Name or service", ""));
            v.push(f("type", "Type", ""));
            v.extend([f("design_p", "Design pressure", "press"), f("design_t", "Design temperature", "temp"), f("oper_p", "Operating pressure", "press"), f("oper_t", "Operating temperature", "temp"), f("material", "Material", "")]);
            match cat {
                Cat::Vessel => v.extend([f("volume", "Volume", "volume"), f("size", "Size (diameter × length)", "")]),
                Cat::Rotating if key.starts_with("pump") || key.starts_with("comp") || key == "fan" => v.extend([f("flow", "Rated flow", "flow"), f("head", "Head", "head"), f("power", "Power", "power")]),
                Cat::Rotating => v.push(f("power", "Power", "power")),
                Cat::Heat => v.extend([f("duty", "Duty", "power"), f("area", "Area", "area")]),
                _ => {}
            }
            v.push(f("nozzles", "Nozzles", ""));
            if matches!(cat, Cat::Vessel | Cat::Heat) {
                v.push(f("relief", "Relief path (if not drawn)", ""));
            }
        }
        Cat::Valve | Cat::Control => {
            v.extend([f("size", "Size (NPS)", ""), f("class", "Class or rating", ""), c("operator", "Operator", OPERATORS)]);
            if cat == Cat::Valve {
                v.push(c("position", "Normal position", POSITIONS));
            } else {
                v.extend([c("fail", "Fail position", FAIL), f("set_p", "Set pressure", "press"), f("loop", "Loop number", "")]);
            }
            v.push(f("service", "Service", ""));
        }
        Cat::Instrument | Cat::Primary => {
            v.extend([f("function", "Function", ""), f("loop", "Loop number", ""), f("range", "Range", ""), f("setpoints", "Set points (alarms, trips)", ""), c("signal", "Signal", SIGNALS), c("location", "Location", LOCATIONS)]);
            if cat == Cat::Primary {
                v.push(f("size", "Size (NPS)", ""));
            }
        }
        Cat::Connector => v.extend([f("to_text", "Continues to (text)", "")]),
        Cat::Fitting => v.extend([f("size", "Size (NPS)", ""), f("service", "Notes", "")]),
        Cat::Mark => v.push(f("service", "Notes", "")),
    }
    v
}

/// A line's fields.
pub fn line() -> Vec<Field> {
    vec![
        f("size", "Size (NPS)", ""),
        f("sched", "Schedule", ""),
        f("fluid", "Fluid code", ""),
        f("seq", "Sequence number", ""),
        f("class", "Pipe class", ""),
        f("service", "Service", ""),
        c("phase", "Phase", PHASES),
        c("limits", "Hydraulic limits", SERVICES),
        f("flow", "Volume flow", "flow"),
        f("mflow", "Mass flow", "mflow"),
        f("density", "Density", "density"),
        f("viscosity", "Viscosity", "visc"),
        f("length", "Length for hydraulics", "length"),
        f("fittings", "Fittings allowance (% of length)", ""),
        f("design_p", "Design pressure", "press"),
        f("design_t", "Design temperature", "temp"),
        f("oper_p", "Operating pressure", "press"),
        f("oper_t", "Operating temperature", "temp"),
        f("test_p", "Test pressure", "press"),
        f("ins_type", "Insulation type", ""),
        f("ins_thk", "Insulation thickness", "thk"),
        f("tracing", "Tracing", ""),
        f("number", "Line number (typed, overrides the format)", ""),
    ]
}
