//! Sizing: a control valve's flow coefficient (ISA-75.01.01 / IEC 60534-2-1, liquid and gas, with
//! choked flow), an orifice plate's bore (ISO 5167-2, Reader-Harris/Gallagher, corner taps) and
//! a relief valve's area (API 520 Part I, gas or vapour at critical flow, and liquid, with the
//! API 526 orifice letter). Inputs come from the device and its lines; what was assumed is said.
//! A sizing aid checked by the engineer.

use crate::graph::Net;
use crate::model::{Data, Item, Project, Val};
use serde::Serialize;

const ATM: f64 = 101_325.0;

/// One sizing: what it is, the inputs (label, value as shown), the results, what was assumed,
/// and the values to write to the datasheet.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Sizing {
    pub what: String,
    pub method: String,
    pub inputs: Vec<(String, String)>,
    pub results: Vec<(String, String)>,
    pub notes: Vec<String>,
    pub writes: Vec<(String, Val)>,
}

// ---------------------------------------------------------------- the formulas (SI in, SI out)

/// Control valve, liquid (IEC 60534-2-1 eq. 1 and 3): Kv in m³/h at 1 bar. `q` m³/s, `dp` Pa,
/// `rho` kg/m³; choked when Δp > FL²(p1 − FF pv). Returns (Cv, Kv, choked).
pub fn cv_liquid(q: f64, dp: f64, rho: f64, p1: f64, pv: Option<f64>, pc: f64, fl: f64) -> (f64, f64, bool) {
    let ff = pv.map_or(0.96, |pv| (0.96 - 0.28 * (pv / pc).max(0.0).sqrt()).clamp(0.68, 0.96));
    let dp_max = fl * fl * (p1 - ff * pv.unwrap_or(0.0));
    let choked = pv.is_some() && dp > dp_max;
    let dp_s = if choked { dp_max } else { dp };
    let kv = q * 3600.0 * (rho / 1000.0 / (dp_s / 1e5)).sqrt();
    (kv * 1.156, kv, choked)
}

/// Control valve, gas (IEC 60534-2-1 eq. 8 with N6 = 27.3, Fp = 1): `w` kg/s, `p1` Pa abs,
/// `dp` Pa, `rho1` kg/m³ at the inlet; x limited to Fγ xT (choked). Returns (Cv, Kv, Y, choked).
pub fn cv_gas(w: f64, p1: f64, dp: f64, rho1: f64, k: f64, xt: f64) -> (f64, f64, f64, bool) {
    let fk = k / 1.4;
    let x = dp / p1;
    let choked = x > fk * xt;
    let xs = x.min(fk * xt);
    let y = 1.0 - xs / (3.0 * fk * xt);
    let kv = (w * 3600.0) / (27.3 * y * (xs * p1 / 1000.0 * rho1).sqrt());
    (kv * 1.156, kv, y, choked)
}

/// The discharge coefficient of an orifice with corner taps (ISO 5167-2 eq. 4, L1 = L'2 = 0).
pub fn orifice_c(beta: f64, re_d: f64, d_pipe: f64) -> f64 {
    let a = (19_000.0 * beta / re_d).powf(0.8);
    let mut c = 0.5961 + 0.0261 * beta.powi(2) - 0.216 * beta.powi(8) + 0.000521 * (1e6 * beta / re_d).powf(0.7) + (0.0188 + 0.0063 * a) * beta.powf(3.5) * (1e6 / re_d).powf(0.3);
    if d_pipe < 0.07112 {
        c += 0.011 * (0.75 - beta) * (2.8 - d_pipe / 0.0254);
    }
    c
}

/// The mass flow through an orifice (ISO 5167-1 eq. 1, liquid: ε = 1).
pub fn orifice_flow(beta: f64, d_pipe: f64, dp: f64, rho: f64, mu: f64) -> f64 {
    // C depends on Re, which depends on the flow: a few rounds settle it.
    let d = beta * d_pipe;
    let area = std::f64::consts::PI / 4.0 * d * d;
    let mut qm = area * (2.0 * dp * rho).sqrt() * 0.6 / (1.0 - beta.powi(4)).sqrt();
    for _ in 0..20 {
        let re = 4.0 * qm / (std::f64::consts::PI * d_pipe * mu);
        let c = orifice_c(beta, re.max(1.0), d_pipe);
        qm = c / (1.0 - beta.powi(4)).sqrt() * area * (2.0 * dp * rho).sqrt();
    }
    qm
}

/// The beta ratio that passes `qm` kg/s at `dp` (bisection over 0.1–0.75, the standard's range).
pub fn orifice_beta(qm: f64, d_pipe: f64, dp: f64, rho: f64, mu: f64) -> Option<f64> {
    let f = |b: f64| orifice_flow(b, d_pipe, dp, rho, mu) - qm;
    let (mut lo, mut hi) = (0.1, 0.75);
    if f(lo) > 0.0 || f(hi) < 0.0 {
        return None;
    }
    for _ in 0..80 {
        let mid = (lo + hi) / 2.0;
        if f(mid) > 0.0 { hi = mid } else { lo = mid }
    }
    Some((lo + hi) / 2.0)
}

/// The gas coefficient C of API 520 (SI): 0.03948 √(k (2/(k+1))^((k+1)/(k−1))).
pub fn api_c(k: f64) -> f64 {
    let k = if (k - 1.0).abs() < 1e-6 { 1.001 } else { k };
    0.03948 * (k * (2.0 / (k + 1.0)).powf((k + 1.0) / (k - 1.0))).sqrt()
}

/// Relief area for gas or vapour at critical flow (API 520 Part I eq. 9, SI): mm². `w` kg/h,
/// `p1` kPa abs (set + overpressure + atmosphere), `t` K, K = 0.975, Kb = Kc = 1.
pub fn psv_gas(w: f64, p1: f64, t: f64, z: f64, m: f64, k: f64) -> f64 {
    w / (api_c(k) * 0.975 * p1) * (t * z / m).sqrt()
}

/// Relief area for liquid (API 520 Part I eq. 30, SI): mm². `q` L/min, `dp` kPa (p1 − p2),
/// G relative density; Kd = 0.65, Kw = Kc = Kv = 1.
pub fn psv_liquid(q: f64, g: f64, dp: f64) -> f64 {
    11.78 * q / 0.65 * (g / dp).sqrt()
}

/// API 526 effective orifice areas (mm²).
pub const API526: [(&str, f64); 14] = [("D", 71.0), ("E", 126.0), ("F", 198.0), ("G", 325.0), ("H", 506.0), ("J", 830.0), ("K", 1186.0), ("L", 1841.0), ("M", 2323.0), ("N", 2800.0), ("P", 4116.0), ("Q", 7129.0), ("R", 10323.0), ("T", 16774.0)];

pub fn api526(area: f64) -> Option<(&'static str, f64)> {
    API526.iter().find(|x| x.1 >= area).copied()
}

// ---------------------------------------------------------------- from the drawing

fn num(d: &Data, k: &str) -> Option<f64> {
    d.get(k).and_then(|v| v.num()).filter(|x| x.is_finite())
}

/// The lines on a device's process ports, with their data: (data, the device is at the line's downstream end).
fn lines<'a>(p: &'a Project, net: &Net, it: &Item) -> Vec<&'a Data> {
    let Some(s) = crate::symbols::get(&it.sym) else { return vec![] };
    (0..s.ports.len()).filter(|k| !s.ports[*k].signal).flat_map(|k| net.on_port(it.id, k as u8).to_vec()).filter_map(|(l, _)| net.lines.get(&l)).filter(|l| l.kind.process() && l.obj != 0).map(|l| p.data_of(l.obj)).collect()
}

fn gas(d: &[&Data]) -> bool {
    let phase = d.iter().find_map(|x| x.get("phase").map(|v| v.text())).unwrap_or_default();
    match phase.as_str() {
        "Gas" | "Steam" | "Vapour" => true,
        "Liquid" => false,
        _ => d.iter().find_map(|x| num(x, "density")).is_some_and(|r| r < 100.0),
    }
}

fn first(d: &[&Data], k: &str) -> Option<f64> {
    d.iter().find_map(|x| num(x, k))
}

fn show(x: f64, q: &str, us: bool) -> String {
    crate::units::show(x, q, us)
}

/// Sizes a control valve, an orifice plate or a relief valve from its data and its lines.
pub fn size(p: &Project, item: u32) -> Result<Sizing, String> {
    let s = size_raw(p, item)?;
    // Data that make no physical sense (a negative drop, a zero density) give no result.
    let bad = s.writes.iter().any(|(_, v)| match v {
        Val::Num(x) => !x.is_finite() || *x <= 0.0,
        v => v.text().contains('—') || v.text().starts_with('-') || v.text() == "0",
    }) || s.results.iter().any(|r| r.1.contains("NaN") || r.1.contains("inf"));
    if bad {
        return Err(format!("{}: the data give no sensible result; check its flow, pressures and density.", s.what));
    }
    Ok(s)
}

fn size_raw(p: &Project, item: u32) -> Result<Sizing, String> {
    let it = p.item(item).ok_or("That symbol no longer exists.")?;
    let o = p.obj(it.obj).ok_or("That symbol has no object.")?;
    let d = &o.data;
    let net = Net::new(p);
    let ls = lines(p, &net, it);
    let us = p.units == "US";
    let tag = if o.tag.trim().is_empty() { "The device".to_string() } else { o.tag.clone() };
    let base = it.base_key();
    let mut notes: Vec<String> = vec![];
    let need = |x: Option<f64>, what: &str| x.ok_or_else(|| format!("{tag} needs {what} to be sized."));
    if matches!(base.as_str(), "cv_pneu" | "cv_motor") {
        let rho = need(first(&ls, "density"), "a density on its line")?;
        // Inlet pressure: the higher operating pressure of its lines; the drop: typed, else the lines' difference.
        let ps: Vec<f64> = ls.iter().filter_map(|x| num(x, "oper_p")).collect();
        let p1 = need(ps.iter().cloned().fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b)))), "an operating pressure on its line")? + ATM;
        let dp = match num(d, "dp_design") {
            Some(x) => x,
            None => {
                let lo = ps.iter().cloned().fold(f64::INFINITY, f64::min);
                let hi = ps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                if ps.len() >= 2 && hi - lo > 0.0 {
                    notes.push("Pressure drop: the difference between its lines' operating pressures.".into());
                    hi - lo
                } else {
                    return Err(format!("{tag} needs its pressure drop at design flow (Sizing › Pressure drop), or different operating pressures on its two lines."));
                }
            }
        };
        let mut inputs = vec![("Inlet pressure".to_string(), show(p1 - ATM, "press", us) + " g"), ("Pressure drop".into(), show(dp, "dp", us)), ("Density".into(), show(rho, "density", us))];
        let (cv, kv, extra, method) = if gas(&ls) {
            let w = need(first(&ls, "mflow").or_else(|| first(&ls, "flow").map(|q| q * rho)), "a flow on its line")?;
            let k = num(d, "k_ratio").unwrap_or_else(|| { notes.push("k = 1.3 assumed (Sizing › Ratio of specific heats).".into()); 1.3 });
            let xt = num(d, "xt").unwrap_or_else(|| { notes.push("xT = 0.70 assumed (globe valve; Sizing › xT).".into()); 0.70 });
            let (cv, kv, y, choked) = cv_gas(w, p1, dp, rho, k, xt);
            inputs.push(("Mass flow".into(), show(w, "mflow", us)));
            if choked {
                notes.push("The flow is choked: the drop beyond Fγ xT adds no flow.".into());
            }
            (cv, kv, vec![("Expansion factor Y".to_string(), crate::fmt_sig(y, 3))], "IEC 60534-2-1 (ISA-75.01.01), gas".to_string())
        } else {
            let q = need(first(&ls, "flow").or_else(|| first(&ls, "mflow").map(|m| m / rho)), "a flow on its line")?;
            let fl = num(d, "fl").unwrap_or_else(|| { notes.push("FL = 0.90 assumed (globe valve; Sizing › FL).".into()); 0.90 });
            let pv = num(d, "vapour_p").map(|x| x + ATM);
            if pv.is_none() {
                notes.push("No vapour pressure: choked flow (cavitation) is not checked.".into());
            }
            let (cv, kv, choked) = cv_liquid(q, dp, rho, p1, pv, 22.064e6, fl);
            inputs.push(("Volume flow".into(), show(q, "flow", us)));
            if choked {
                notes.push("The flow is choked (flashing or cavitation): the valve is sized at the choked drop; check the trim.".into());
            }
            (cv, kv, vec![], "IEC 60534-2-1 (ISA-75.01.01), liquid".to_string())
        };
        let mut results = vec![("Cv required".to_string(), crate::fmt_sig(cv, 3)), ("Kv required".into(), crate::fmt_sig(kv, 3))];
        results.extend(extra);
        if let Some(rated) = d.get("cv_rated").and_then(|v| v.num()).filter(|x| *x > 0.0) {
            results.push(("Of the rated Cv".into(), format!("{} %", crate::fmt_sig(cv / rated * 100.0, 3))));
            if cv > rated * 0.8 {
                notes.push("Above 80 % of the rated Cv: the valve may be too small.".into());
            }
        }
        return Ok(Sizing { what: format!("{tag}: control valve"), method, inputs, results, notes, writes: vec![("cv_required".into(), Val::Text(crate::fmt_sig(cv, 3)))] });
    }
    if base == "orifice" {
        let dpipe = {
            let size = first_text(&ls, "size");
            let sched = first_text(&ls, "sched");
            crate::pipes::inside(&size, if sched.is_empty() { "40" } else { &sched })
        };
        let dpipe = need(dpipe, "a size on its line")?;
        let rho = need(first(&ls, "density"), "a density on its line")?;
        let mu = first(&ls, "viscosity").unwrap_or_else(|| { notes.push("Viscosity 1 cP assumed.".into()); 1e-3 });
        let qm = need(first(&ls, "mflow").or_else(|| first(&ls, "flow").map(|q| q * rho)), "a flow on its line")?;
        let dp = num(d, "dp_design").unwrap_or_else(|| { notes.push("Differential pressure 25 kPa (250 mbar) assumed at the flow (Sizing › Differential pressure).".into()); 25_000.0 });
        if gas(&ls) {
            notes.push("Gas: the expansion factor is taken as 1; size it again with the transmitter's range at low pressure ratios.".into());
        }
        let beta = orifice_beta(qm, dpipe, dp, rho, mu).ok_or_else(|| format!("{tag}: no beta ratio between 0.10 and 0.75 passes the flow at that differential pressure; change the differential pressure."))?;
        let re = 4.0 * qm / (std::f64::consts::PI * dpipe * mu);
        let inputs = vec![("Pipe inside diameter".to_string(), show(dpipe, "thk", us)), ("Mass flow".into(), show(qm, "mflow", us)), ("Differential pressure".into(), show(dp, "dp", us)), ("Density".into(), show(rho, "density", us))];
        let results = vec![("Beta ratio".to_string(), crate::fmt_sig(beta, 4)), ("Bore".into(), show(beta * dpipe, "thk", us)), ("Discharge coefficient C".into(), crate::fmt_sig(orifice_c(beta, re, dpipe), 4)), ("Reynolds number".into(), crate::fmt_sig(re, 3))];
        if re < 5000.0 {
            notes.push("Reynolds number below the standard's range for corner taps.".into());
        }
        return Ok(Sizing { what: format!("{tag}: orifice plate"), method: "ISO 5167-2 (Reader-Harris/Gallagher, corner taps)".into(), inputs, results, notes, writes: vec![("beta_ratio".into(), Val::Text(crate::fmt_sig(beta, 4))), ("bore".into(), Val::Num(beta * dpipe))] });
    }
    if matches!(base.as_str(), "psv" | "rupture") || crate::classes::chain(&o.class).iter().any(|c| c.name == "ReliefDevice") {
        let set = need(num(d, "set_p"), "a set pressure")?;
        let over = num(d, "overpressure").unwrap_or(10.0);
        let p1 = set * (1.0 + over / 100.0) + ATM;
        let w = need(num(d, "relieving_rate").or_else(|| first(&ls, "mflow")), "a relieving rate (Sizing › Relieving rate)")?;
        let mut inputs = vec![("Set pressure".to_string(), show(set, "press", us) + " g"), ("Overpressure".into(), format!("{} %", crate::fmt_num(over))), ("Relieving rate".into(), show(w, "mflow", us))];
        let (area, method) = if gas(&ls) || num(d, "mw").is_some() {
            let t = need(num(d, "relieving_t").or_else(|| first(&ls, "oper_t")), "a relieving temperature")?;
            let m = need(num(d, "mw"), "a molar mass (Sizing › Molar mass)")?;
            let k = num(d, "k_ratio").unwrap_or_else(|| { notes.push("k = 1.3 assumed.".into()); 1.3 });
            let z = num(d, "z").unwrap_or(1.0);
            inputs.push(("Relieving temperature".into(), show(t, "temp", us)));
            inputs.push(("Molar mass".into(), format!("{} kg/kmol", crate::fmt_num(m))));
            (psv_gas(w * 3600.0, p1 / 1000.0, t, z, m, k), "API 520 Part I, gas or vapour, critical flow (K 0.975, Kb Kc 1)")
        } else {
            let rho = need(first(&ls, "density"), "a molar mass (Sizing › Molar mass) for gas or vapour, or a density on its line for liquid")?;
            let back = num(d, "back_p").unwrap_or(0.0) + ATM;
            let q = w / rho * 60_000.0;
            (psv_liquid(q, rho / 1000.0, (p1 - back) / 1000.0), "API 520 Part I, liquid (Kd 0.65, Kw Kc Kv 1)")
        };
        let letter = api526(area);
        let results = vec![("Required area".to_string(), format!("{} mm²", crate::fmt_sig(area, 4))), ("API 526 orifice".into(), letter.map_or("larger than T: two valves".into(), |(l, a)| format!("{l} ({} mm²)", crate::fmt_num(a))))];
        notes.push("Inlet loss, back pressure correction and two-phase relief are not included.".into());
        let write = letter.map_or_else(|| format!("{} mm² required", crate::fmt_sig(area, 4)), |(l, a)| format!("{l} ({} mm²), {} mm² required", crate::fmt_num(a), crate::fmt_sig(area, 4)));
        return Ok(Sizing { what: format!("{tag}: relief valve"), method: method.into(), inputs, results, notes, writes: vec![("orifice_area".into(), Val::Text(write))] });
    }
    Err(format!("{tag} is not a control valve, an orifice plate or a relief valve."))
}

fn first_text(d: &[&Data], k: &str) -> String {
    d.iter().find_map(|x| x.get(k).map(|v| v.text()).filter(|s| !s.trim().is_empty())).unwrap_or_default()
}

/// Whether a symbol can be sized here.
pub fn sizable(it: &Item) -> bool {
    matches!(it.base_key().as_str(), "cv_pneu" | "cv_motor" | "orifice" | "psv" | "rupture")
}

/// What the window shows for a symbol: nothing (not sizable), the sizing, or what it needs.
#[derive(Clone, Debug, Serialize, Default)]
pub struct Shown {
    pub sizable: bool,
    pub result: Option<Sizing>,
    pub error: String,
}

pub fn shown(p: &Project, item: u32) -> Shown {
    match p.item(item) {
        Some(it) if sizable(it) => match size(p, item) {
            Ok(s) => Shown { sizable: true, result: Some(s), error: String::new() },
            Err(e) => Shown { sizable: true, result: None, error: e },
        },
        _ => Shown::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * b.abs()
    }

    #[test]
    fn formulas_against_hand_calculations() {
        // Liquid: 100 m³/h of water through 1 bar: Kv 100, Cv 115.6.
        let (cv, kv, ch) = cv_liquid(100.0 / 3600.0, 1e5, 1000.0, 5e5, None, 22.064e6, 0.9);
        assert!(close(kv, 100.0, 1e-9) && close(cv, 115.6, 1e-9) && !ch);
        // Choked: 4.5 bar drop from 5 bar abs with water at 3 kPa: Δp max = 0.81 (5 − 0.947 × 0.03) bar.
        let (_, kv, ch) = cv_liquid(100.0 / 3600.0, 4.5e5, 1000.0, 5e5, Some(3000.0), 22.064e6, 0.9);
        let ff = 0.96 - 0.28 * (3000.0f64 / 22.064e6).sqrt();
        assert!(ch && close(kv, 100.0 / (0.81 * (5e5 - ff * 3000.0) / 1e5).sqrt(), 1e-9));
        // Gas: 10 t/h at 10 bar abs, 2 bar drop, 8 kg/m³, k 1.4, xT 0.7: x 0.2, Y 0.90476.
        let (_, kv, y, ch) = cv_gas(10_000.0 / 3600.0, 1e6, 2e5, 8.0, 1.4, 0.7);
        assert!(close(y, 1.0 - 0.2 / 2.1, 1e-9) && !ch);
        assert!(close(kv, 10_000.0 / (27.3 * y * (0.2f64 * 1000.0 * 8.0).sqrt()), 1e-9));
        // API 520's C for k = 1.4 is 0.02703 (SI).
        assert!(close(api_c(1.4), 0.02703, 2e-4));
        // Gas relief: 24 270 kg/h, 670 kPa abs, 348 K, Z 0.90, M 51, k 1.11.
        let a = psv_gas(24_270.0, 670.0, 348.0, 0.9, 51.0, 1.11);
        assert!(close(a, 24_270.0 / (api_c(1.11) * 0.975 * 670.0) * (348.0f64 * 0.9 / 51.0).sqrt(), 1e-12) && a > 3000.0 && a < 4500.0, "{a}");
        assert_eq!(api526(a).unwrap().0, "P");
        // Liquid relief: 1 000 L/min of water across 1 000 kPa.
        assert!(close(psv_liquid(1000.0, 1.0, 1000.0), 11.78 * 1000.0 / 0.65 / 1000f64.sqrt(), 1e-12));
        // Orifice: C near 0.60; the bore found for a flow gives that flow back.
        let c = orifice_c(0.5, 1e6, 0.1023);
        assert!(c > 0.598 && c < 0.612, "{c}");
        let qm = orifice_flow(0.5, 0.1023, 25_000.0, 998.0, 1e-3);
        assert!(close(orifice_beta(qm, 0.1023, 25_000.0, 998.0, 1e-3).unwrap(), 0.5, 1e-6));
    }

    #[test]
    fn devices_on_the_drawing_are_sized() {
        let p = crate::examples::get("pump").unwrap();
        let lv = p.items.iter().find(|i| p.tag_of(i) == "LV-101").unwrap();
        // The level valve: 30 m³/h of water across 0.8 bar: Kv = 30 √(0.998 / 0.8) = 33.5.
        let s = size(&p, lv.id).unwrap();
        assert!(s.method.contains("liquid") && s.results[1].1 == "33.5" && s.writes[0].0 == "cv_required", "{s:#?}");
        // Without a drop typed and its lines at one pressure, it says what it needs.
        let mut q = p.clone();
        q.obj_mut(lv.obj).unwrap().data.remove("dp_design");
        assert!(size(&q, lv.id).unwrap_err().contains("pressure drop"));
        // The example's relief valve is a steam relief: 7 200 kg/h at 10 bar g + 10 %, 458 K, M 18, k 1.33.
        let psv = p.items.iter().find(|i| p.tag_of(i) == "PSV-101").unwrap();
        let s = size(&p, psv.id).unwrap();
        let hand = psv_gas(7200.0, (1.1e6 + ATM) / 1000.0, 458.0, 1.0, 18.0, 1.33);
        assert!(close(hand, 1166.0, 0.01) && s.results[1].1.starts_with("K (1186"), "{hand} {s:#?}");
        // Without its relieving rate or its molar mass it says what it needs.
        let mut q = p.clone();
        q.obj_mut(psv.obj).unwrap().data.remove("relieving_rate");
        assert!(size(&q, psv.id).unwrap_err().contains("relieving rate"));
        let mut q = p.clone();
        q.obj_mut(psv.obj).unwrap().data.remove("mw");
        assert!(size(&q, psv.id).is_err_and(|e| e.contains("molar mass")));
        // Hostile values never panic, and never give a result that is not a positive number.
        for ex in ["pump", "faults"] {
            let Some(p) = crate::examples::get(ex) else { continue };
            for it in p.items.iter().filter(|i| sizable(i)) {
                for x in [-1e9, -1.0, 0.0, 1e-300, 1e300, f64::NAN, f64::INFINITY] {
                    let mut q = p.clone();
                    for o in q.objects.iter_mut() {
                        for k in ["dp_design", "density", "flow", "mflow", "oper_p", "set_p", "relieving_rate", "mw", "relieving_t", "k_ratio", "xt", "fl", "vapour_p", "viscosity", "z", "overpressure", "back_p"] {
                            o.data.insert(k.into(), Val::Num(x));
                        }
                    }
                    if let Ok(s) = size(&q, it.id) {
                        assert!(s.writes.iter().all(|w| w.1.num().is_some_and(|v| v.is_finite() && v > 0.0) || !w.1.text().is_empty()), "{s:#?}");
                    }
                }
            }
        }
        // A pump is not sized here.
        let pump = p.items.iter().find(|i| p.tag_of(i) == "P-101A").unwrap();
        assert!(size(&p, pump.id).is_err());
    }
}
