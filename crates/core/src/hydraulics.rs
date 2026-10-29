//! Line hydraulics: for a line with a flow, a density and a viscosity, the inside diameter (from
//! its size and schedule), the velocity, the Reynolds number, the Darcy friction factor (Colebrook,
//! as Datum Calc's pipe pressure-drop pack solves it) and the pressure drop per 100 m and over the
//! line's length plus its fittings allowance, judged against the limits for its service.
//!
//! Sources: Darcy–Weisbach Δp = f (L/D) ρ v²/2; C. F. Colebrook, J. Inst. Civil Eng. 11 (1939)
//! 133–156; laminar f = 64/Re (Hagen–Poiseuille). Results are a sizing aid, checked by the engineer.

use crate::graph::Net;
use crate::model::{Data, Limit, Project};
use crate::pipes;
use serde::Serialize;

/// One run's hydraulic result. SI units: m, m/s, Pa per 100 m, Pa.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Hyd {
    pub id: f64,
    pub v: f64,
    pub re: f64,
    pub f: f64,
    pub dp100: f64,
    pub dp: f64,
    pub length: f64,
    /// The service whose limits apply (`Limit::service`) and its name.
    pub service: String,
    pub service_name: String,
    pub vmin: f64,
    pub vmax: f64,
    pub dpmax: f64,
    /// 0 within limits (✓), 1 close to or just outside them (!), 2 well outside (✗).
    pub status: u8,
    pub reason: String,
}

/// The Darcy friction factor: 64/Re in laminar flow (Re < 2000), Colebrook otherwise (solved to
/// round-off, from the Swamee–Jain estimate). `rel` is roughness / diameter.
pub fn friction(re: f64, rel: f64) -> f64 {
    if re.is_nan() || re <= 0.0 || !re.is_finite() {
        return f64::NAN;
    }
    if re < 2000.0 {
        return 64.0 / re;
    }
    let rel = rel.max(0.0);
    let sj = 0.25 / ((rel / 3.7 + 5.74 / re.powf(0.9)).log10()).powi(2);
    let mut x = 1.0 / sj.sqrt();
    for _ in 0..100 {
        let nx = -2.0 * (rel / 3.7 + 2.51 * x / re).log10();
        if (nx - x).abs() < 1e-14 * x.abs() {
            x = nx;
            break;
        }
        x = nx;
    }
    1.0 / (x * x)
}

/// The basic calculation, for a known inside diameter (m), volume flow (m³/s), density (kg/m³),
/// viscosity (Pa·s), roughness (m) and length (m): (v, Re, f, Δp per 100 m, Δp over the length).
pub fn calc(d: f64, q: f64, rho: f64, mu: f64, rough: f64, length: f64) -> (f64, f64, f64, f64, f64) {
    let a = std::f64::consts::PI * d * d / 4.0;
    let v = q / a;
    let re = rho * v * d / mu;
    let f = friction(re, rough / d);
    let per_m = f / d * rho * v * v / 2.0;
    (v, re, f, per_m * 100.0, per_m * length)
}

fn num(l: &Data, k: &str) -> Option<f64> {
    l.get(k).and_then(|v| v.num())
}
fn text(l: &Data, k: &str) -> String {
    l.get(k).map(|v| v.text()).unwrap_or_default()
}

/// The limits that apply to a run's lead line: its own choice, else pump suction or discharge from
/// its connections, else by its phase.
pub fn limit_for<'a>(p: &'a Project, net: &Net, run: usize, l: &Data) -> Option<&'a Limit> {
    let chosen = text(l, "limits");
    let key = if !chosen.is_empty() && chosen != "auto" {
        chosen
    } else {
        match text(l, "phase").as_str() {
            "Gas" => "gas".into(),
            "Steam" => "steam".into(),
            "Two-phase" => "twophase".into(),
            _ => net.pump_side(run).unwrap_or("liquid").to_string(),
        }
    };
    p.settings.limits.iter().find(|x| x.service == key).or(p.settings.limits.first())
}

/// The hydraulics of a run, from its lead line (the run's first line; every line of a run carries
/// the same data). None when the flow, density, viscosity or size is missing.
pub fn run(p: &Project, net: &Net, run: usize) -> Option<Hyd> {
    let l = p.data_of(*net.run_obj.get(run)?);
    let rho = num(l, "density").filter(|x| *x > 0.0)?;
    let mu = num(l, "viscosity").filter(|x| *x > 0.0)?;
    let q = num(l, "flow").filter(|x| *x > 0.0).or_else(|| num(l, "mflow").filter(|x| *x > 0.0).map(|m| m / rho))?;
    let d = pipes::inside(&text(l, "size"), &text(l, "sched"))?;
    let length = num(l, "length").unwrap_or(0.0).max(0.0) * (1.0 + num(l, "fittings").unwrap_or(0.0).max(0.0) / 100.0);
    let (v, re, f, dp100, dp) = calc(d, q, rho, mu, p.settings.roughness, length);
    if !(v.is_finite() && f.is_finite()) {
        return None;
    }
    let lim = limit_for(p, net, run, l)?;
    let (mut status, mut why) = (0u8, vec![]);
    let over = |x: f64, max: f64| if max <= 0.0 { 0 } else if x > 1.1 * max { 2 } else if x > max { 1 } else { 0 };
    let sv = over(v, lim.vmax);
    if sv > 0 {
        status = status.max(sv);
        why.push(format!("velocity above {} m/s", crate::fmt_sig(lim.vmax, 3)));
    }
    let sd = over(dp100, lim.dp);
    if sd > 0 {
        status = status.max(sd);
        why.push(format!("pressure drop above {} kPa/100 m", crate::fmt_sig(lim.dp / 1000.0, 3)));
    }
    if v < lim.vmin {
        status = status.max(1);
        why.push(format!("velocity below {} m/s", crate::fmt_sig(lim.vmin, 3)));
    }
    Some(Hyd { id: d, v, re, f, dp100, dp, length, service: lim.service.clone(), service_name: lim.name.clone(), vmin: lim.vmin, vmax: lim.vmax, dpmax: lim.dp, status, reason: why.join("; ") })
}

/// Every run's hydraulics (None where data is missing), by run index.
pub fn all(p: &Project, net: &Net) -> Vec<Option<Hyd>> {
    (0..net.runs.len()).map(|r| run(p, net, r)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * b.abs()
    }

    #[test]
    fn same_answer_as_datum_calc_pipe_pack() {
        // Datum Calc's "Pipe pressure drop (Darcy–Weisbach)" pack, same inputs: Q = 120 m³/h,
        // D = 154.1 mm, L = 250 m, ε = 0.045 mm, ρ = 998 kg/m³, μ = 1 cP, K = 8.5. Calc's tested
        // answers: v = 1.787243 m/s, Re = 274 863.3, f = 0.01706669, Δp = 57 680.44 Pa with K.
        let (v, re, f, _, dp) = calc(0.1541, 120.0 / 3600.0, 998.0, 1e-3, 4.5e-5, 250.0);
        assert!(close(v, 1.787_243, 1e-6), "{v}");
        assert!(close(re, 274_863.3, 1e-6), "{re}");
        assert!(close(f, 0.017_066_69, 1e-6), "{f}");
        let with_k = dp + 8.5 * 998.0 * v * v / 2.0;
        assert!(close(with_k, 57_680.44, 1e-5), "{with_k}");
    }

    #[test]
    fn hand_calculation() {
        // 3" schedule 40 (ID 3.068 in = 77.93 mm), 30 m³/h of water (ρ 1000, μ 1 cP), smooth-ish
        // steel 0.045 mm, 50 m. By hand: A = π 0.077927²/4 = 4.7694e-3 m²; v = 0.0083333/A =
        // 1.7472 m/s; Re = 1000 × 1.7472 × 0.077927 / 0.001 = 136 157; ε/D = 5.775e-4;
        // Colebrook (iterated from 0.02) f = 0.019 92; Δp/m = f/D ρ v²/2 = 390.2 Pa/m, so
        // 39.02 kPa per 100 m and 19.51 kPa over 50 m.
        let d = pipes::inside("3", "40").unwrap();
        assert!(close(d, 0.077_927_2, 1e-6));
        let (v, re, f, dp100, dp) = calc(d, 30.0 / 3600.0, 1000.0, 1e-3, 4.5e-5, 50.0);
        assert!(close(v, 1.7472, 1e-4), "{v}");
        assert!(close(re, 136_157.0, 1e-4), "{re}");
        assert!(close(f, 0.019_920, 1e-4), "{f}");
        assert!(close(dp100, 39_019.0, 1e-4), "{dp100}");
        assert!(close(dp, dp100 / 2.0, 1e-12));
        // Laminar: Re = 1000 gives 64/Re.
        assert!(close(friction(1000.0, 0.001), 0.064, 1e-12));
        assert!(friction(0.0, 0.001).is_nan());
    }
}
