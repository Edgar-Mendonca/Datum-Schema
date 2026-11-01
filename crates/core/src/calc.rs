//! Calculation sheets in Datum Calc: an object's data put into one of Datum Calc's ready sheets
//! (its packs, copied here as `calcpacks/*.dcalc`) and saved as that app's own file. Inputs the
//! drawing knows are filled in (from the object, the lines on its nozzles and its pipe spec); the
//! others keep the pack's typical value, and the sheet says which is which.

use crate::graph::Net;
use crate::links::Sent;
use crate::model::{Data, Project};
use serde::Serialize;
use serde_json::Value;

/// The packs: id, file, name, and the classes they are for.
const PACKS: [(&str, &str, &str, &[&str]); 8] = [
    ("pump-power", include_str!("../calcpacks/pump-power.dcalc"), "Pump power and motor size", &["Pump"]),
    ("npsh", include_str!("../calcpacks/npsh.dcalc"), "NPSH available", &["Pump"]),
    ("compressor", include_str!("../calcpacks/compressor.dcalc"), "Compressor power and discharge temperature", &["Compressor"]),
    ("hx-lmtd", include_str!("../calcpacks/hx-lmtd.dcalc"), "Exchanger duty, LMTD and area", &["HeatExchanger"]),
    ("pressure-vessel", include_str!("../calcpacks/pressure-vessel.dcalc"), "Vessel shell thickness", &["Vessel"]),
    ("tank", include_str!("../calcpacks/tank.dcalc"), "Tank volume and filling time", &["Tank"]),
    ("pipe-dp", include_str!("../calcpacks/pipe-dp.dcalc"), "Line pressure drop", &["PipingNetworkSystem"]),
    ("pipe-wall", include_str!("../calcpacks/pipe-wall.dcalc"), "Pipe wall thickness", &["PipingNetworkSystem"]),
];

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Sheet {
    pub id: &'static str,
    pub name: &'static str,
}

/// The calculation sheets that fit an object (by its class and the classes above it).
pub fn sheets_for(p: &Project, obj: u32) -> Vec<Sheet> {
    let Some(o) = p.obj(obj) else { return vec![] };
    let chain: Vec<String> = crate::classes::chain(&o.class).iter().map(|c| c.name.clone()).collect();
    PACKS.iter().filter(|(_, _, _, cls)| cls.iter().any(|c| *c == o.class || chain.iter().any(|x| x == c))).map(|(id, _, name, _)| Sheet { id, name }).collect()
}

fn num(d: &Data, k: &str) -> Option<f64> {
    d.get(k).and_then(|v| v.num()).filter(|x| x.is_finite())
}

/// The data of the process lines on an item's nozzles, first the one on a named port.
fn lines_of<'a>(p: &'a Project, net: &Net, item: u32, port: Option<&str>) -> Vec<&'a Data> {
    let Some(it) = p.item(item) else { return vec![] };
    let Some(s) = crate::symbols::get(&it.sym) else { return vec![] };
    let mut ks: Vec<usize> = (0..s.ports.len()).filter(|k| !s.ports[*k].signal).collect();
    if let Some(name) = port {
        ks.sort_by_key(|k| s.ports[*k].name != name);
    }
    ks.iter().flat_map(|k| net.on_port(it.id, *k as u8).to_vec()).filter_map(|(l, _)| net.lines.get(&l)).filter(|l| l.kind.process() && l.obj != 0).map(|l| p.data_of(l.obj)).collect()
}

fn first(ds: &[&Data], k: &str) -> Option<f64> {
    ds.iter().find_map(|d| num(d, k))
}

/// What goes into each input: (variable, value in the unit written, the unit as Datum Calc reads it).
fn inputs(p: &Project, net: &Net, obj: u32, pack: &str) -> Vec<(&'static str, f64, &'static str)> {
    let o = match p.obj(obj) { Some(o) => o, None => return vec![] };
    let d = &o.data;
    let item = p.items.iter().find(|i| i.obj == obj).map(|i| i.id);
    let all = item.map(|i| lines_of(p, net, i, None)).unwrap_or_default();
    let suction = item.map(|i| lines_of(p, net, i, Some("suction"))).unwrap_or_default();
    let discharge = item.map(|i| lines_of(p, net, i, Some("discharge"))).unwrap_or_default();
    let mut v: Vec<(&str, f64, &str)> = vec![];
    let mut put = |name: &'static str, x: Option<f64>, unit: &'static str| {
        if let Some(x) = x {
            v.push((name, x, unit));
        }
    };
    match pack {
        "pump-power" => {
            put("Q", num(d, "flow").or_else(|| first(&discharge, "flow")).map(|x| x * 3600.0), "m^3/h");
            put("H", num(d, "head"), "m");
            put("rho", first(&discharge, "density"), "kg/m^3");
            put("P_motor", num(d, "power").map(|x| x / 1000.0), "kW");
        }
        "npsh" => {
            put("rho", first(&suction, "density"), "kg/m^3");
        }
        "compressor" => {
            put("T_1", first(&suction, "oper_t").map(|k| k - 273.15), "°C");
            put("p_1", first(&suction, "oper_p").map(|x| x / 1e5 + 1.01325), "bar");
            put("p_2", first(&discharge, "oper_p").map(|x| x / 1e5 + 1.01325), "bar");
            put("m", first(&suction, "mflow").or_else(|| first(&suction, "flow").zip(first(&suction, "density")).map(|(q, r)| q * r)), "kg/s");
            put("P_driver", num(d, "power").map(|x| x / 1000.0), "kW");
        }
        "hx-lmtd" => {
            put("A_avail", num(d, "area"), "m^2");
        }
        "pressure-vessel" => {
            put("P", num(d, "design_p").map(|x| x / 1e6), "MPa");
        }
        "tank" => {
            put("rho", first(&all, "density"), "kg/m^3");
        }
        "pipe-dp" | "pipe-wall" => {
            let size = d.get("size").map(|v| v.text()).unwrap_or_default();
            let sched = d.get("sched").map(|v| v.text()).filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "40".into());
            let pipe = crate::pipes::nps_key(&size).and_then(|k| crate::pipes::pipe(k, &sched));
            if pack == "pipe-dp" {
                put("Q", num(d, "flow").map(|x| x * 3600.0), "m^3/h");
                put("D", crate::pipes::inside(&size, &sched).map(|x| x * 1000.0), "mm");
                put("L", num(d, "length"), "m");
                put("eps", Some(p.settings.roughness * 1000.0).filter(|x| *x > 0.0), "mm");
                put("rho", num(d, "density"), "kg/m^3");
                put("mu", num(d, "viscosity").map(|x| x * 1000.0), "cP");
                // The velocity limit of the line's service (Options › Hydraulic limits).
                let hyd = crate::hydraulics::all(p, net);
                put("v_max", net.run_obj.iter().position(|o| *o == obj).and_then(|r| hyd.get(r).cloned().flatten()).map(|h| h.vmax), "m/s");
            } else {
                put("P", num(d, "design_p").map(|x| x / 1e5), "bar");
                put("D", pipe.map(|x| x.0 * 1000.0), "mm");
                put("t_nom", pipe.map(|x| x.1 * 1000.0), "mm");
                let class = d.get("class").map(|v| v.text()).unwrap_or_default();
                put("c", crate::specs::find(p, &class).map(|s| s.corrosion * 1000.0).filter(|x| *x > 0.0), "mm");
            }
        }
        _ => {}
    }
    v
}

/// What each filled input is, where the pack's own words name its sample.
const WORDS: [(&str, &str, &str); 10] = [
    ("pipe-dp", "v_max", "velocity limit of the line's service"),
    ("pump-power", "rho", "liquid density"),
    ("npsh", "rho", "liquid density"),
    ("pipe-dp", "D", "inside diameter"),
    ("pipe-dp", "rho", "density"),
    ("pipe-dp", "mu", "viscosity"),
    ("pipe-dp", "eps", "pipe roughness"),
    ("pipe-wall", "D", "outside diameter"),
    ("pipe-wall", "t_nom", "nominal wall"),
    ("tank", "rho", "liquid density"),
];

fn fmt(x: f64) -> String {
    let s = crate::fmt_sig(x, 5);
    s.replace(',', "")
}

/// The inputs a sheet cannot do without: with any of them missing, a sample value would be mixed
/// with the object's own and the result would mean nothing.
const NEEDS: [(&str, &[(&str, &str)]); 6] = [
    ("pump-power", &[("Q", "a flow"), ("H", "a head")]),
    ("compressor", &[("m", "a mass flow on its suction line"), ("p_1", "an operating pressure on its suction line")]),
    ("pressure-vessel", &[("P", "a design pressure")]),
    ("pipe-dp", &[("Q", "a flow"), ("D", "a size")]),
    ("pipe-wall", &[("P", "a design pressure"), ("D", "a size")]),
    ("npsh", &[("rho", "a density on its suction line")]),
];

/// An object's calculation sheet as a Datum Calc file.
pub fn to_calc(p: &Project, obj: u32, pack: &str) -> Result<Sent, String> {
    let (_, json, name, _) = PACKS.iter().find(|x| x.0 == pack).ok_or_else(|| format!("There is no calculation sheet {pack}."))?;
    let o = p.obj(obj).ok_or("That object no longer exists.")?;
    if !sheets_for(p, obj).iter().any(|s| s.id == pack) {
        return Err(format!("{name} is not a calculation for this kind of object."));
    }
    let tag = if o.tag.trim().is_empty() { crate::graph::obj_number(p, obj) } else { o.tag.clone() };
    let tag = if tag.trim().is_empty() { "the line".to_string() } else { tag };
    let mut v: Value = serde_json::from_str(json).map_err(|e| format!("The {name} sheet could not be read: {e}"))?;
    let net = Net::new(p);
    let fill = inputs(p, &net, obj, pack);
    if let Some((_, needs)) = NEEDS.iter().find(|n| n.0 == pack) {
        let missing: Vec<&str> = needs.iter().filter(|(v, _)| !fill.iter().any(|f| f.0 == *v)).map(|(_, w)| *w).collect();
        if !missing.is_empty() {
            return Err(format!("{tag} needs {} for its {} sheet: fill it in first.", missing.join(" and "), name.to_lowercase()));
        }
    }
    let mut used: Vec<&str> = vec![];
    let mut kept: Vec<String> = vec![];
    if let Some(regions) = v.get_mut("regions").and_then(|r| r.as_array_mut()) {
        for r in regions.iter_mut() {
            if r.get("style").and_then(|s| s.as_str()) != Some("input") {
                continue;
            }
            let Some(text) = r.get("text").and_then(|t| t.as_str()).map(str::to_string) else { continue };
            let (lhs, comment) = text.split_once('#').map_or((text.as_str(), ""), |(a, b)| (a, b.trim()));
            let Some(var) = lhs.split_once('=').map(|x| x.0.trim().to_string()) else { continue };
            match fill.iter().find(|(n, _, _)| *n == var) {
                Some((n, x, unit)) => {
                    used.push(n);
                    // The pack's own words describe its sample (water at 20 °C, 6 in schedule 40):
                    // a filled value says only what it is and where it came from.
                    let what = comment.split(['(', ',']).next().unwrap_or("").split(" of ").next().unwrap_or("").trim();
                    let what = WORDS.iter().find(|w| w.0 == pack && w.1 == *n).map_or(what, |w| w.2);
                    r["text"] = Value::String(format!("{n} = {} {unit}  # {what}, from {tag}", fmt(*x)));
                }
                None => kept.push(var),
            }
        }
    }
    let title = format!("{name}: {tag}");
    v["name"] = Value::String(title.clone());
    if let Some(t) = v.get_mut("title") {
        t["title"] = Value::String(title.clone());
        t["number"] = Value::String(format!("CALC-{}-{}", crate::clip(&tag.replace(['"', ' '], ""), 30), pack.to_uppercase()));
        t["project"] = Value::String(if p.title.project.is_empty() { p.name.clone() } else { p.title.project.clone() });
        t["company"] = Value::String(p.title.company.clone());
        t["client"] = Value::String(p.title.client.clone());
        t["by"] = Value::String(p.title.by.clone());
    }
    if let Some(regions) = v.get_mut("regions").and_then(|r| r.as_array_mut()) {
        let next = regions.iter().filter_map(|r| r.get("id").and_then(|i| i.as_u64())).max().unwrap_or(0) + 1;
        let note = format!("From {tag} in Datum Schema: {} taken from the drawing; {} kept at the sheet's typical values: check them.", if used.is_empty() { "nothing".into() } else { used.join(", ") }, if kept.is_empty() { "nothing".into() } else { kept.join(", ") });
        regions.insert(2.min(regions.len()), serde_json::json!({ "id": next, "kind": "text", "text": note, "unit": "", "style": "note", "data": null }));
        v["next_id"] = Value::from(next + 1);
    }
    let mut notes = vec![format!("{} input{} from {tag}", used.len(), if used.len() == 1 { "" } else { "s" })];
    if !kept.is_empty() {
        notes.push(format!("kept at the sheet's typical values: {}", kept.join(", ")));
    }
    let file = format!("{} {}.dcalc", crate::clip(&tag.replace(['"', '/', '\\', ':'], ""), 40), name.to_lowercase());
    Ok(Sent { file, text: serde_json::to_string_pretty(&v).unwrap_or_default(), notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(s: &Sent, var: &str) -> String {
        let v: Value = serde_json::from_str(&s.text).unwrap();
        v["regions"].as_array().unwrap().iter().filter_map(|r| r["text"].as_str()).find(|t| t.starts_with(&format!("{var} ="))).unwrap_or("").to_string()
    }

    #[test]
    fn a_pump_and_a_line_go_to_datum_calc() {
        let p = crate::examples::get("pump").unwrap();
        let pump = p.objects.iter().find(|o| o.tag == "P-101A").unwrap().id;
        let ids: Vec<&str> = sheets_for(&p, pump).iter().map(|s| s.id).collect();
        assert_eq!(ids, ["pump-power", "npsh"]);
        let s = to_calc(&p, pump, "pump-power").unwrap();
        assert!(s.file.ends_with(".dcalc"));
        // Flow and head from the pump, density from its discharge line, in units Datum Calc reads.
        assert!(text_of(&s, "Q").contains("m^3/h") && text_of(&s, "Q").ends_with("# rated flow, from P-101A"), "{}", text_of(&s, "Q"));
        assert!(text_of(&s, "rho").starts_with("rho = 998 kg/m^3"), "{}", text_of(&s, "rho"));
        assert!(text_of(&s, "eta_p").starts_with("eta_p = 0.75"), "the rest keep the pack's values");
        let v: Value = serde_json::from_str(&s.text).unwrap();
        assert!(v["title"]["number"].as_str().unwrap().starts_with("CALC-P-101A"));
        assert!(v["regions"].as_array().unwrap().iter().any(|r| r["text"].as_str().is_some_and(|t| t.contains("taken from the drawing"))));
        // Ids stay unique and next_id above them.
        let ids: Vec<u64> = v["regions"].as_array().unwrap().iter().map(|r| r["id"].as_u64().unwrap()).collect();
        let mut u = ids.clone();
        u.sort();
        u.dedup();
        assert_eq!(u.len(), ids.len());
        assert!(v["next_id"].as_u64().unwrap() > *u.last().unwrap());
        // A line: its inside diameter from the size and schedule, its roughness from the project.
        let line = p.objects.iter().find(|o| o.class == crate::model::LINE_CLASS && o.data.get("size").is_some_and(|v| v.text() == "3")).unwrap().id;
        let s = to_calc(&p, line, "pipe-dp").unwrap();
        assert_eq!(text_of(&s, "D"), "D = 77.927 mm  # inside diameter, from 3\"-P-1003-A1A");
        let w = to_calc(&p, line, "pipe-wall").unwrap();
        assert!(text_of(&w, "c").starts_with("c = 1.5 mm"), "corrosion from the A1A spec: {}", text_of(&w, "c"));
        // Wrong pairings are refused.
        assert!(to_calc(&p, pump, "pipe-dp").is_err());
        // A line without a flow has no pressure drop sheet (a sample flow would mean nothing).
        let dry = p.objects.iter().find(|o| o.class == crate::model::LINE_CLASS && !o.data.contains_key("flow") && o.data.contains_key("size")).unwrap().id;
        assert!(to_calc(&p, dry, "pipe-dp").unwrap_err().contains("needs a flow"));
        // The velocity limit is the line's own service limit.
        assert!(text_of(&s, "v_max").starts_with("v_max = ") && text_of(&s, "v_max").contains("from 3"), "{}", text_of(&s, "v_max"));
        assert!(to_calc(&p, pump, "nothing").is_err());
        assert!(to_calc(&p, 999_999, "npsh").is_err());
    }
}
