//! Sending work on to the family's other apps: a line to Datum Pipe (a `.dpipe` stress model to
//! start from) and a control loop to Datum Sim (a `.dsim` block diagram to tune). Both files are
//! the other app's own JSON, written from the drawing and the line's or loop's data; anything the
//! drawing cannot know (the routing in three dimensions, the process dynamics) is a stated
//! placeholder for the engineer to replace.

use crate::graph::Net;
use crate::model::{Item, Project, Val};
use crate::symbols::{self, Cat};
use serde_json::json;

/// A file for another app: its name, its text, and what was assumed.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Sent {
    pub file: String,
    pub text: String,
    pub notes: Vec<String>,
}

fn file_name(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let mut t = t.trim_matches('-').to_string();
    while t.contains("--") {
        t = t.replace("--", "-");
    }
    if t.is_empty() { "line".into() } else { crate::clip(&t, 60) }
}


/// The pieces of a run in order from one end, with the in-line item after each piece (if any).
fn chain(net: &Net, run: &[u32]) -> Vec<(u32, Option<u32>)> {
    use std::collections::HashSet;
    let inline_at = |l: u32, at_b: bool| -> Option<u32> {
        let line = net.lines.get(&l)?;
        let (it, _) = (if at_b { &line.b } else { &line.a }).item()?;
        let s = symbols::get(&net.items.get(&it)?.sym)?;
        s.inline.then_some(it)
    };
    // The other piece of the run on an in-line item.
    let across = |it: u32, from: u32| -> Option<(u32, bool)> {
        let s = symbols::get(&net.items.get(&it)?.sym)?;
        (0..s.ports.len() as u8).flat_map(|k| net.on_port(it, k).iter().copied()).find(|(l, _)| *l != from && run.contains(l))
    };
    // Start from a piece with an end that does not continue through an in-line item.
    let start = run.iter().copied().find_map(|l| {
        for at_b in [false, true] {
            if inline_at(l, at_b).and_then(|it| across(it, l)).is_none() {
                return Some((l, at_b));
            }
        }
        None
    });
    let Some((mut l, mut from_b)) = start.or(run.first().map(|l| (*l, false))) else { return vec![] };
    let mut seen = HashSet::new();
    let mut out = vec![];
    while seen.insert(l) {
        let it = inline_at(l, !from_b);
        out.push((l, it));
        match it.and_then(|i| across(i, l)) {
            Some((n, n_at_b)) => {
                l = n;
                from_b = n_at_b;
            }
            None => break,
        }
    }
    out
}

fn route_len(pts: &[[f64; 2]]) -> f64 {
    pts.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum()
}

fn num(d: Option<&Val>) -> Option<f64> {
    d.and_then(|v| v.num()).filter(|x| x.is_finite())
}

/// A line as a Datum Pipe model: a straight run between two anchors, the valves and other in-line
/// items at their places along it, the line's size, schedule, design conditions and density.
pub fn to_pipe(p: &Project, line: u32) -> Result<Sent, String> {
    let net = Net::new(p);
    let r = *net.run_of.get(&line).ok_or("Choose a process line to send to Datum Pipe.")?;
    let number = net.numbers.get(&line).cloned().filter(|n| !n.is_empty() && !n.contains('?')).unwrap_or_else(|| "Line".into());
    let number = number.as_str();
    let run = &net.runs[r];
    let o = p.obj(net.run_obj[r]).ok_or("That line has no data.")?;
    let txt = |k: &str| o.data.get(k).map(|v| v.text()).unwrap_or_default();
    let mut notes = vec![];
    let nps = crate::pipes::nps_key(&txt("size")).ok_or_else(|| format!("Line {number} has no size Datum Pipe knows; give it a size (NPS) first."))?;
    let mut sched = txt("sched");
    if crate::pipes::pipe(nps, &sched).is_none() {
        let fallback = ["40", "STD", "10S", "40S"].into_iter().find(|s| crate::pipes::pipe(nps, s).is_some()).unwrap_or("40");
        notes.push(if sched.is_empty() { format!("No schedule: Sch {fallback} was assumed.") } else { format!("Schedule {sched} is not in the pipe table for {nps}\": Sch {fallback} was assumed.") });
        sched = fallback.into();
    }
    let (od, wall) = crate::pipes::pipe(nps, &sched).ok_or("No pipe dimensions for that size.")?;
    // Lengths: the line's length for hydraulics, shared along the run as drawn.
    let routes = crate::view::routes(p, run.iter().filter_map(|l| net.lines.get(l)).map(|l| l.sheet).next().unwrap_or(0), None);
    let pieces = chain(&net, run);
    let drawn: Vec<f64> = pieces.iter().map(|(l, _)| routes.get(l).map_or(0.0, |pts| route_len(pts)).max(1.0)).collect();
    let total_drawn: f64 = drawn.iter().sum();
    let length = match num(o.data.get("length")).filter(|x| *x > 0.0) {
        Some(x) => x,
        None => {
            notes.push("The line has no length: 10 m was assumed.".into());
            10.0
        }
    };
    let ambient = 294.15;
    let t = num(o.data.get("design_t")).filter(|x| *x > 0.0).unwrap_or_else(|| {
        notes.push("No design temperature: ambient was used.".into());
        ambient
    });
    let pr = num(o.data.get("design_p")).unwrap_or_else(|| {
        notes.push("No design pressure: 0 was used.".into());
        0.0
    });
    let rho = num(o.data.get("density")).filter(|x| *x >= 0.0).unwrap_or(0.0);
    // Ids share one namespace in Datum Pipe.
    let (pipe_id, mat_id, cond_id) = (1, 2, 3);
    let mut next = 4u32;
    let mut id = || {
        next += 1;
        next - 1
    };
    let mut elements = vec![];
    let mut node = 10u32;
    let mut supports = vec![];
    let tag = |it: u32| net.items.get(&it).map(|i| p.tag_of(i).to_string()).unwrap_or_default();
    supports.push(json!({ "id": id(), "node": node, "kind": { "kind": "anchor" }, "tag": format!("A-{node}") }));
    let valve_len = (od * 3.0).clamp(0.15, 1.5);
    for (k, (_, it)) in pieces.iter().enumerate() {
        let mut d = length * drawn[k] / total_drawn;
        let has_valve = it.is_some() && k + 1 < pieces.len();
        if has_valve {
            d = (d - valve_len / 2.0).max(0.05);
        }
        if k > 0 && pieces[k - 1].1.is_some() {
            d = (d - valve_len / 2.0).max(0.05);
        }
        elements.push(json!({ "id": id(), "from": node, "to": node + 10, "d": [d, 0.0, 0.0], "pipe": pipe_id, "material": mat_id, "cond": cond_id, "comp": { "kind": "pipe" } }));
        node += 10;
        if let (true, Some(it)) = (has_valve, it) {
            let sym = net.items.get(it).and_then(|i| symbols::get(&i.sym));
            let rigid = if sym.is_some_and(|s| matches!(s.cat, Cat::Valve | Cat::Control)) { "valve" } else { "rigid" };
            let mass = (od * 1000.0).powf(1.6) * 0.05;
            elements.push(json!({ "id": id(), "from": node, "to": node + 10, "d": [valve_len, 0.0, 0.0], "pipe": pipe_id, "material": mat_id, "cond": cond_id, "comp": { "kind": "rigid", "rigid": rigid, "mass": (mass * 10.0).round() / 10.0 } }));
            let _ = tag(*it);
            node += 10;
        }
    }
    supports.push(json!({ "id": id(), "node": node, "kind": { "kind": "anchor" }, "tag": format!("A-{node}") }));
    let valves: Vec<String> = pieces.iter().filter_map(|x| x.1).map(tag).filter(|t| !t.is_empty()).collect();
    if !valves.is_empty() {
        notes.push(format!("Valve masses are estimates: {}. Enter the real ones.", valves.join(", ")));
    }
    notes.push("A P&ID has no routing: the line goes out as a straight run between two anchors. Add the bends, supports and real routing in Datum Pipe.".into());
    let ope = id();
    let sus = id();
    let exp = id();
    let m = json!({
        "format": 1,
        "name": number,
        "units": p.units,
        "notes": format!("From {} in Datum Schema. {}", if p.name.is_empty() { "a P&ID".to_string() } else { p.name.clone() }, notes.join(" ")),
        "project": { "project": p.title.project, "line": number, "client": p.title.client, "by": p.title.by },
        "pipes": [{ "id": pipe_id, "name": format!("{nps}\" Sch {sched}"), "nps": nps, "sched": sched, "od": od, "wall": wall, "mill": 0.125, "fluid_rho": rho }],
        "materials": [{ "id": mat_id, "name": "Carbon steel" }],
        "conds": [{ "id": cond_id, "name": "Design", "t": [t, ambient, ambient], "p": [pr, 0.0, 0.0], "hydro": (pr * 1.5).max(0.0) }],
        "nodes": [{ "num": 10, "x": 0.0, "y": 0.0, "z": 0.0 }],
        "elements": elements,
        "supports": supports,
        "cases": [
            { "id": ope, "name": "OPE1", "kind": "ope", "recipe": { "kind": "sum", "w": true, "water": false, "t": 1, "d": 1, "p": 1 } },
            { "id": sus, "name": "SUS", "kind": "sus", "recipe": { "kind": "sum", "w": true, "water": false, "t": 0, "d": 0, "p": 1 } },
            { "id": exp, "name": "EXP1", "kind": "exp", "recipe": { "kind": "range", "a": ope, "b": sus } }
        ],
        "next": next,
    });
    Ok(Sent { file: format!("{}.dpipe", file_name(number)), text: serde_json::to_string_pretty(&m).unwrap_or_default(), notes })
}

/// A control loop as a Datum Sim model: set point, controller (PID), final element, process and
/// transmitter, with a trend of the set point, the measurement and the output. The dynamics are
/// variables at the top of the model, set to typical values for the loop's measured variable.
pub fn to_sim(p: &Project, key: &str) -> Result<Sent, String> {
    let info = crate::loops::all(p).into_iter().find(|l| l.key == key).ok_or_else(|| format!("There is no loop {key}."))?;
    let items: Vec<&Item> = p.items.iter().filter(|i| info.tags.iter().any(|t| t == p.tag_of(i))).collect();
    let letters = |it: &Item| crate::tags::split(p.tag_of(it)).0;
    let pick = |f: &dyn Fn(&str, Cat) -> bool| items.iter().find(|i| symbols::get(&i.sym).is_some_and(|s| f(&letters(i), s.cat))).map(|i| p.tag_of(i).to_string());
    let var = key.chars().next().unwrap_or('F');
    let rest: String = key.chars().skip(1).collect();
    let ctrl = pick(&|l, _| l.len() >= 2 && (l.ends_with('C') || l.contains("IC"))).unwrap_or_else(|| format!("{var}C{rest}"));
    let tx = pick(&|l, _| l.ends_with('T')).unwrap_or_else(|| format!("{var}T{rest}"));
    let fe = pick(&|l, c| c == Cat::Control || l.ends_with('V') || l.ends_with('Y')).unwrap_or_else(|| format!("{var}V{rest}"));
    // Typical dynamics by measured variable: (process gain, time constant s, integrating, Kp, Ti s, unit).
    let (kp_proc, tau, integ, kc, ti, what) = match var {
        'L' => (0.02, 0.0, true, 2.0, 300.0, "level, %"),
        'P' => (1.0, 5.0, false, 1.0, 10.0, "pressure, %"),
        'T' => (1.0, 120.0, false, 2.0, 240.0, "temperature, %"),
        _ => (1.0, 3.0, false, 0.5, 3.0, "flow, %"),
    };
    let mut notes = vec![format!("The dynamics are typical values for {what}: set Kproc, Tproc, Tvalve and Tmeas from the plant, then tune Kp and Ti.")];
    if integ {
        notes.push("Level is integrating: the process is an integrator with gain Kproc (%/s per % of valve).".into());
    }
    let mut vars = vec![
        json!({ "name": "Kp", "expr": kc.to_string(), "unit": "", "note": "controller gain" }),
        json!({ "name": "Ti", "expr": ti.to_string(), "unit": "s", "note": "integral time" }),
        json!({ "name": "Kproc", "expr": kp_proc.to_string(), "unit": "", "note": "process gain" }),
        json!({ "name": "Tproc", "expr": tau.to_string(), "unit": "s", "note": "process time constant" }),
        json!({ "name": "Tvalve", "expr": "2", "unit": "s", "note": "valve stroke lag" }),
        json!({ "name": "Tmeas", "expr": "1", "unit": "s", "note": "transmitter lag" }),
        json!({ "name": "SP", "expr": "50", "unit": "%", "note": "set point step to" }),
    ];
    let process = if integ {
        json!({ "id": 5, "kind": "Integrator", "name": "Process", "x": 470, "y": 78, "w": 60, "h": 40, "params": { "ic": "40", "lower": "0", "upper": "100" } })
    } else {
        json!({ "id": 5, "kind": "TransferFcn", "name": "Process", "x": 470, "y": 78, "w": 90, "h": 44, "params": { "num": "[Kproc]", "den": "[Tproc 1]" } })
    };
    let mut blocks = vec![
        json!({ "id": 1, "kind": "Step", "name": "Set point", "x": 20, "y": 80, "w": 60, "h": 40, "params": { "time": "5", "initial": if integ { "40" } else { "0" }, "final": "SP" } }),
        json!({ "id": 2, "kind": "Sum", "name": "Error", "x": 140, "y": 82, "w": 36, "h": 36, "params": { "signs": "+-" } }),
        json!({ "id": 3, "kind": "PID", "name": ctrl, "x": 210, "y": 78, "w": 90, "h": 44, "params": { "P": "Kp", "I": "Kp/Ti", "D": "0", "upper": "100", "lower": "0" } }),
        json!({ "id": 4, "kind": "TransferFcn", "name": fe, "x": 340, "y": 78, "w": 90, "h": 44, "params": { "num": "[1]", "den": "[Tvalve 1]" } }),
        process,
        json!({ "id": 6, "kind": "TransferFcn", "name": tx, "x": 470, "y": 190, "w": 90, "h": 44, "flip": true, "params": { "num": "[1]", "den": "[Tmeas 1]" } }),
        json!({ "id": 7, "kind": "Scope", "name": "Trend", "x": 640, "y": 40, "w": 50, "h": 50, "params": { "n": "3" } }),
    ];
    let mut wires = vec![
        json!({ "id": 9, "from": [1, 0], "to": [2, 0], "pts": [], "name": "SP" }),
        json!({ "id": 10, "from": [2, 0], "to": [3, 0], "pts": [] }),
        json!({ "id": 11, "from": [3, 0], "to": [4, 0], "pts": [], "name": "OP" }),
        json!({ "id": 12, "from": [4, 0], "to": [5, 0], "pts": [] }),
        json!({ "id": 13, "from": [5, 0], "to": [6, 0], "pts": [] }),
        json!({ "id": 14, "from": [6, 0], "to": [2, 1], "pts": [], "name": "PV" }),
        json!({ "id": 15, "from": [6, 0], "to": [7, 0], "pts": [] }),
        json!({ "id": 16, "from": [1, 0], "to": [7, 1], "pts": [] }),
        json!({ "id": 17, "from": [3, 0], "to": [7, 2], "pts": [] }),
    ];
    if integ {
        // The tank: Kproc × (inflow through the valve − outflow), integrated.
        blocks.push(json!({ "id": 8, "kind": "Gain", "name": "Kproc", "x": 400, "y": 150, "w": 60, "h": 40, "params": { "k": "Kproc" } }));
        blocks.push(json!({ "id": 18, "kind": "Sum", "name": "In − out", "x": 340, "y": 150, "w": 36, "h": 36, "params": { "signs": "+-" } }));
        blocks.push(json!({ "id": 19, "kind": "Constant", "name": "Outflow", "x": 240, "y": 200, "w": 60, "h": 40, "params": { "value": "Qout" } }));
        wires[3] = json!({ "id": 12, "from": [4, 0], "to": [18, 0], "pts": [] });
        wires.push(json!({ "id": 20, "from": [19, 0], "to": [18, 1], "pts": [] }));
        wires.push(json!({ "id": 21, "from": [18, 0], "to": [8, 0], "pts": [] }));
        wires.push(json!({ "id": 22, "from": [8, 0], "to": [5, 0], "pts": [] }));
        vars.push(json!({ "name": "Qout", "expr": "40", "unit": "%", "note": "outflow, as % of the valve's flow" }));
    }
    let m = json!({
        "format": 2,
        "name": format!("{key} control loop"),
        "notes": format!("From {} in Datum Schema: {}. {}", if p.name.is_empty() { "a P&ID".to_string() } else { p.name.clone() }, info.tags.join(", "), notes.join(" ")),
        "vars": vars,
        "sim": { "start": 0, "stop": if integ || var == 'T' { 1200 } else { 120 }, "solver": "rk45", "step": 0.01, "max_step": 0, "rtol": 1e-6, "atol": 1e-8, "log_all": false },
        "root": { "blocks": blocks, "wires": wires },
        "next": 23,
    });
    notes.push("Open it in Datum Sim and press Run.".into());
    Ok(Sent { file: format!("{}.dsim", file_name(key)), text: serde_json::to_string_pretty(&m).unwrap_or_default(), notes })
}

/// The ids in a JSON model, for the tests: unique and below `next`.
#[cfg(test)]
fn ids(v: &serde_json::Value, out: &mut Vec<u64>) {
    match v {
        serde_json::Value::Object(m) => {
            if let Some(i) = m.get("id").and_then(|x| x.as_u64()) {
                out.push(i);
            }
            m.values().for_each(|x| ids(x, out));
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| ids(x, out)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn a_line_goes_to_datum_pipe() {
        let p = crate::examples::get("pump").unwrap();
        // A line through a valve: anchors at both ends, the valve in the run.
        let net = Net::new(&p);
        let r = (0..net.runs.len()).find(|r| chain(&net, &net.runs[*r]).iter().any(|x| x.1.is_some())).expect("a line with a valve");
        let s = to_pipe(&p, net.runs[r][0]).unwrap();
        assert!(s.file.ends_with(".dpipe") && !s.file.contains("--"));
        let v: Value = serde_json::from_str(&s.text).unwrap();
        assert_eq!(v["format"], 1);
        let els = v["elements"].as_array().unwrap();
        assert!(els.iter().any(|e| e["comp"]["kind"] == "rigid" && e["comp"]["rigid"] == "valve"));
        // Elements chain node to node; the anchors are at the two ends.
        for w in els.windows(2) {
            assert_eq!(w[0]["to"], w[1]["from"]);
        }
        let sup = v["supports"].as_array().unwrap();
        assert_eq!(sup.len(), 2);
        assert_eq!(sup[0]["node"], els[0]["from"]);
        assert_eq!(sup[1]["node"], els.last().unwrap()["to"]);
        // The run's length is the line's length; the pipe is the line's size.
        let o = p.obj(net.run_obj[r]).unwrap();
        let len: f64 = els.iter().map(|e| e["d"][0].as_f64().unwrap()).sum();
        if let Some(l) = o.data.get("length").and_then(|v| v.num()) {
            assert!((len - l).abs() < 0.2, "{len} vs {l}");
        }
        let (od, wall) = crate::pipes::pipe(v["pipes"][0]["nps"].as_str().unwrap(), v["pipes"][0]["sched"].as_str().unwrap()).unwrap();
        assert_eq!(v["pipes"][0]["od"].as_f64().unwrap(), od);
        assert!(wall < od / 2.0);
        assert!(v["conds"][0]["t"][0].as_f64().unwrap() > 0.0);
        let mut all = vec![];
        ids(&v, &mut all);
        let n_ids = all.len();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), n_ids, "unique ids");
        assert!(all.iter().all(|i| *i > 0 && *i < v["next"].as_u64().unwrap()));
        assert!(to_pipe(&p, 999_999).is_err());
        // Every process line of both examples goes.
        for ex in ["pump", "compressor"] {
            let q = crate::examples::get(ex).unwrap();
            let net = Net::new(&q);
            for run in &net.runs {
                to_pipe(&q, run[0]).unwrap();
            }
        }
    }

    #[test]
    fn a_loop_goes_to_datum_sim() {
        for ex in ["pump", "compressor"] {
            let p = crate::examples::get(ex).unwrap();
            for l in crate::loops::all(&p) {
                let s = to_sim(&p, &l.key).unwrap();
                let v: Value = serde_json::from_str(&s.text).unwrap();
                assert_eq!(v["format"], 2);
                let blocks = v["root"]["blocks"].as_array().unwrap();
                let wires = v["root"]["wires"].as_array().unwrap();
                let bid: Vec<u64> = blocks.iter().map(|b| b["id"].as_u64().unwrap()).collect();
                // Every wire joins two blocks; one wire per input.
                let mut inputs = std::collections::HashSet::new();
                for w in wires {
                    assert!(bid.contains(&w["from"][0].as_u64().unwrap()) && bid.contains(&w["to"][0].as_u64().unwrap()));
                    assert!(inputs.insert((w["to"][0].as_u64(), w["to"][1].as_u64())), "{}", l.key);
                }
                // Every block input is fed (Step has none; Sum two; Scope three).
                for b in blocks {
                    let n = match b["kind"].as_str().unwrap() { "Step" => 0, "Sum" => 2, "Scope" => 3, "Constant" => 0, _ => 1 };
                    for k in 0..n {
                        assert!(inputs.contains(&(b["id"].as_u64(), Some(k))), "{} {} input {k}", l.key, b["kind"]);
                    }
                }
                assert!(blocks.iter().any(|b| b["kind"] == "PID"));
                let mut all = vec![];
                ids(&v, &mut all);
                let n_ids = all.len();
                all.sort();
                all.dedup();
                assert_eq!(all.len(), n_ids);
                assert!(all.iter().all(|i| *i < v["next"].as_u64().unwrap()));
            }
        }
        assert!(to_sim(&crate::examples::get("pump").unwrap(), "X-999").is_err());
    }
}
