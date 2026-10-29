//! Pipe sizes and schedules, copied from Datum Pipe's library: nominal pipe sizes ½" to 24" with
//! their outside diameters and the wall thickness of each schedule. Dimensions are the published
//! nominal dimensions of wrought steel pipe (ASME B36.10M) and stainless steel pipe (ASME
//! B36.19M): public facts, given here in inches as the standards list them and converted to metres
//! on use. The hydraulics take the inside diameter from here.

use serde::Serialize;

const IN: f64 = 0.0254;

/// NPS, outside diameter (in), and (schedule, wall thickness in inches).
type Row = (&'static str, f64, &'static [(&'static str, f64)]);

// A 0.318" wall is a dimension, not an approximation of 1/π.
#[rustfmt::skip]
#[allow(clippy::approx_constant)]
const SIZES: &[Row] = &[
    ("1/2", 0.840, &[("5S", 0.065), ("10S", 0.083), ("10", 0.083), ("40", 0.109), ("STD", 0.109), ("40S", 0.109), ("80", 0.147), ("XS", 0.147), ("80S", 0.147), ("160", 0.188), ("XXS", 0.294)]),
    ("3/4", 1.050, &[("5S", 0.065), ("10S", 0.083), ("10", 0.083), ("40", 0.113), ("STD", 0.113), ("40S", 0.113), ("80", 0.154), ("XS", 0.154), ("80S", 0.154), ("160", 0.219), ("XXS", 0.308)]),
    ("1", 1.315, &[("5S", 0.065), ("10S", 0.109), ("10", 0.109), ("40", 0.133), ("STD", 0.133), ("40S", 0.133), ("80", 0.179), ("XS", 0.179), ("80S", 0.179), ("160", 0.250), ("XXS", 0.358)]),
    ("1-1/4", 1.660, &[("5S", 0.065), ("10S", 0.109), ("10", 0.109), ("40", 0.140), ("STD", 0.140), ("40S", 0.140), ("80", 0.191), ("XS", 0.191), ("80S", 0.191), ("160", 0.250), ("XXS", 0.382)]),
    ("1-1/2", 1.900, &[("5S", 0.065), ("10S", 0.109), ("10", 0.109), ("40", 0.145), ("STD", 0.145), ("40S", 0.145), ("80", 0.200), ("XS", 0.200), ("80S", 0.200), ("160", 0.281), ("XXS", 0.400)]),
    ("2", 2.375, &[("5S", 0.065), ("10S", 0.109), ("10", 0.109), ("40", 0.154), ("STD", 0.154), ("40S", 0.154), ("80", 0.218), ("XS", 0.218), ("80S", 0.218), ("160", 0.344), ("XXS", 0.436)]),
    ("2-1/2", 2.875, &[("5S", 0.083), ("10S", 0.120), ("10", 0.120), ("40", 0.203), ("STD", 0.203), ("40S", 0.203), ("80", 0.276), ("XS", 0.276), ("80S", 0.276), ("160", 0.375), ("XXS", 0.552)]),
    ("3", 3.500, &[("5S", 0.083), ("10S", 0.120), ("10", 0.120), ("40", 0.216), ("STD", 0.216), ("40S", 0.216), ("80", 0.300), ("XS", 0.300), ("80S", 0.300), ("160", 0.438), ("XXS", 0.600)]),
    ("3-1/2", 4.000, &[("5S", 0.083), ("10S", 0.120), ("10", 0.120), ("40", 0.226), ("STD", 0.226), ("40S", 0.226), ("80", 0.318), ("XS", 0.318), ("80S", 0.318)]),
    ("4", 4.500, &[("5S", 0.083), ("10S", 0.120), ("10", 0.120), ("40", 0.237), ("STD", 0.237), ("40S", 0.237), ("80", 0.337), ("XS", 0.337), ("80S", 0.337), ("120", 0.438), ("160", 0.531), ("XXS", 0.674)]),
    ("5", 5.563, &[("5S", 0.109), ("10S", 0.134), ("10", 0.134), ("40", 0.258), ("STD", 0.258), ("40S", 0.258), ("80", 0.375), ("XS", 0.375), ("80S", 0.375), ("120", 0.500), ("160", 0.625), ("XXS", 0.750)]),
    ("6", 6.625, &[("5S", 0.109), ("10S", 0.134), ("10", 0.134), ("40", 0.280), ("STD", 0.280), ("40S", 0.280), ("80", 0.432), ("XS", 0.432), ("80S", 0.432), ("120", 0.562), ("160", 0.719), ("XXS", 0.864)]),
    ("8", 8.625, &[("5S", 0.109), ("10S", 0.148), ("10", 0.148), ("20", 0.250), ("30", 0.277), ("40", 0.322), ("STD", 0.322), ("40S", 0.322), ("60", 0.406), ("80", 0.500), ("XS", 0.500), ("80S", 0.500), ("100", 0.594), ("120", 0.719), ("140", 0.812), ("160", 0.906), ("XXS", 0.875)]),
    ("10", 10.750, &[("5S", 0.134), ("10S", 0.165), ("10", 0.165), ("20", 0.250), ("30", 0.307), ("40", 0.365), ("STD", 0.365), ("40S", 0.365), ("60", 0.500), ("XS", 0.500), ("80S", 0.500), ("80", 0.594), ("100", 0.719), ("120", 0.844), ("140", 1.000), ("160", 1.125), ("XXS", 1.000)]),
    ("12", 12.750, &[("5S", 0.156), ("10S", 0.180), ("10", 0.180), ("20", 0.250), ("30", 0.330), ("STD", 0.375), ("40S", 0.375), ("40", 0.406), ("XS", 0.500), ("80S", 0.500), ("60", 0.562), ("80", 0.688), ("100", 0.844), ("120", 1.000), ("140", 1.125), ("160", 1.312), ("XXS", 1.000)]),
    ("14", 14.000, &[("5S", 0.156), ("10S", 0.188), ("10", 0.250), ("20", 0.312), ("30", 0.375), ("STD", 0.375), ("40", 0.438), ("XS", 0.500), ("60", 0.594), ("80", 0.750), ("100", 0.938), ("120", 1.094), ("140", 1.250), ("160", 1.406)]),
    ("16", 16.000, &[("5S", 0.165), ("10S", 0.188), ("10", 0.250), ("20", 0.312), ("30", 0.375), ("STD", 0.375), ("40", 0.500), ("XS", 0.500), ("60", 0.656), ("80", 0.844), ("100", 1.031), ("120", 1.219), ("140", 1.438), ("160", 1.594)]),
    ("18", 18.000, &[("5S", 0.165), ("10S", 0.188), ("10", 0.250), ("20", 0.312), ("STD", 0.375), ("30", 0.438), ("XS", 0.500), ("40", 0.562), ("60", 0.750), ("80", 0.938), ("100", 1.156), ("120", 1.375), ("140", 1.562), ("160", 1.781)]),
    ("20", 20.000, &[("5S", 0.188), ("10S", 0.218), ("10", 0.250), ("20", 0.375), ("STD", 0.375), ("30", 0.500), ("XS", 0.500), ("40", 0.594), ("60", 0.812), ("80", 1.031), ("100", 1.281), ("120", 1.500), ("140", 1.750), ("160", 1.969)]),
    ("22", 22.000, &[("5S", 0.188), ("10S", 0.218), ("10", 0.250), ("20", 0.375), ("STD", 0.375), ("30", 0.500), ("XS", 0.500), ("60", 0.875), ("80", 1.125), ("100", 1.375), ("120", 1.625), ("140", 1.875), ("160", 2.125)]),
    ("24", 24.000, &[("5S", 0.218), ("10S", 0.250), ("10", 0.250), ("20", 0.375), ("STD", 0.375), ("XS", 0.500), ("30", 0.562), ("40", 0.688), ("60", 0.969), ("80", 1.219), ("100", 1.531), ("120", 1.812), ("140", 2.062), ("160", 2.344)]),
];

pub const SIZE_SOURCE: &str = "Nominal outside diameters and wall thicknesses of wrought steel pipe (ASME B36.10M) and stainless steel pipe (ASME B36.19M)";

/// One nominal size with every schedule it comes in (metres).
#[derive(Clone, Debug, Serialize)]
pub struct Size {
    pub nps: &'static str,
    pub od: f64,
    pub schedules: Vec<(&'static str, f64)>,
}

pub fn sizes() -> Vec<Size> {
    SIZES.iter().map(|(nps, od, s)| Size { nps, od: od * IN, schedules: s.iter().map(|(k, t)| (*k, t * IN)).collect() }).collect()
}

/// Outside diameter and wall (m) of a nominal size and schedule, e.g. ("6", "40").
pub fn pipe(nps: &str, sched: &str) -> Option<(f64, f64)> {
    let (_, od, s) = SIZES.iter().find(|r| r.0 == nps)?;
    let (_, t) = s.iter().find(|x| x.0.eq_ignore_ascii_case(sched))?;
    Some((od * IN, t * IN))
}

/// The nominal size as a number of inches ("1-1/2" → 1.5), for bend radii (1.5 × NPS for a
/// long-radius elbow).
/// The nominal size as a number of inches ("1-1/2" → 1.5; "1 1/2", "1.5" and "1½" are read too).
pub fn nps_inches(nps: &str) -> Option<f64> {
    let t = nps.trim().trim_end_matches('"').trim().replace('½', "-1/2").replace('¼', "-1/4").replace('¾', "-3/4").replace(' ', "-");
    let t = t.trim_start_matches('-');
    let frac = |s: &str| -> Option<f64> {
        match s.split_once('/') {
            Some((a, b)) => Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?).filter(|v| v.is_finite()),
            None => s.parse().ok().filter(|v: &f64| v.is_finite()),
        }
    };
    match t.split_once('-') {
        Some((w, f)) if !w.is_empty() => Some(w.parse::<f64>().ok()? + frac(f)?),
        _ => frac(t),
    }
}

/// The library's spelling of a size typed any common way ("1.5", "1 1/2" → "1-1/2").
pub fn nps_key(nps: &str) -> Option<&'static str> {
    let v = nps_inches(nps)?;
    SIZES.iter().find(|r| nps_inches(r.0).is_some_and(|x| (x - v).abs() < 1e-9)).map(|r| r.0)
}

/// Inside diameter (m) of a nominal size and schedule; schedule empty means STD (40).
pub fn inside(nps: &str, sched: &str) -> Option<f64> {
    let key = nps_key(nps)?;
    let sched = if sched.trim().is_empty() { "STD" } else { sched.trim() };
    let (od, t) = pipe(key, sched).or_else(|| pipe(key, sched.trim_start_matches("SCH").trim_start_matches("Sch").trim()))?;
    Some(od - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_inside_diameters() {
        assert_eq!(sizes().len(), 21);
        // 6" schedule 40: 6.625 − 2 × 0.280 = 6.065 in.
        assert!((inside("6", "40").unwrap() - 6.065 * IN).abs() < 1e-12);
        assert!((inside("1-1/2", "80").unwrap() - (1.900 - 0.400) * IN).abs() < 1e-12);
        assert_eq!(nps_key("1.5"), Some("1-1/2"));
        assert_eq!(nps_key("3\""), Some("3"));
        assert!(inside("3", "").is_some() && inside("7", "40").is_none() && inside("x", "40").is_none());
    }
}
