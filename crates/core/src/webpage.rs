//! The plant as one web page: a single HTML file anyone can open in a browser, with no program to
//! install. Every sheet is drawn (zoom and pan), any tag clicked shows its data, a search finds a
//! tag on whichever sheet it is, off-page connectors lead to their partner, and the line list,
//! equipment list, instrument index and valve list are tables that lead to the drawing. Read-only:
//! a snapshot of the project when it was saved. All text is escaped; the page runs no code but its
//! own and fetches nothing.

use crate::graph::Net;
use crate::model::{Project, Val};
use crate::symbols::{self, Cat, Place, Prim};
use crate::view;
use serde_json::{json, Value};

fn n(v: f64) -> String {
    crate::fmt_num((v * 1000.0).round() / 1000.0)
}

fn prim(p: &Prim) -> String {
    match p {
        Prim::Poly { pts, closed, fill, dash, w } => {
            let d: Vec<String> = pts.iter().map(|q| format!("{},{}", n(q[0]), n(q[1]))).collect();
            format!("<{} points=\"{}\" stroke-width=\"{}\"{}{}/>", if *closed { "polygon" } else { "polyline" }, d.join(" "), n(*w), if *dash { " stroke-dasharray=\"1.2 0.8\"" } else { "" }, if *fill { " fill=\"currentColor\"" } else { "" })
        }
        Prim::Circle { c, r, fill, w } => format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\" stroke-width=\"{}\"{}/>", n(c[0]), n(c[1]), n(*r), n(*w), if *fill { " fill=\"currentColor\"" } else { "" }),
        Prim::Arc { c, r, a0, a1, w } => {
            let pt = |a: f64| [c[0] + r * a.to_radians().cos(), c[1] + r * a.to_radians().sin()];
            let (p0, p1) = (pt(*a0), pt(*a1));
            let large = if (a1 - a0).rem_euclid(360.0) > 180.0 { 1 } else { 0 };
            format!("<path d=\"M{} {}A{} {} 0 {large} 1 {} {}\" stroke-width=\"{}\"/>", n(p0[0]), n(p0[1]), n(*r), n(*r), n(p1[0]), n(p1[1]), n(*w))
        }
        Prim::Text { p, h, s, anchor, bold } => format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"{}\" dominant-baseline=\"central\" fill=\"currentColor\" stroke=\"none\"{}>{}</text>",
            n(p[0]),
            n(p[1]),
            n(*h),
            ["start", "middle", "end"].get(*anchor as usize).unwrap_or(&"middle"),
            if *bold { " font-weight=\"600\"" } else { "" },
            crate::xml(s)
        ),
    }
}

fn width(k: crate::model::Kind) -> f64 {
    use crate::model::Kind::*;
    match k {
        Major => 0.6,
        Minor => 0.35,
        Utility => 0.45,
        _ => 0.25,
    }
}

/// A value shown in the project's units.
fn shown(v: &Val, q: &str, us: bool) -> String {
    match v {
        Val::Num(x) if !q.is_empty() => crate::units::show(*x, q, us),
        Val::Num(x) => crate::fmt_sig(*x, 6),
        Val::Text(t) => t.clone(),
    }
}

/// An object's data as label and value rows, in its class's order, then anything else it holds.
fn rows(p: &Project, obj: u32, us: bool) -> Vec<[String; 2]> {
    let Some(o) = p.obj(obj) else { return vec![] };
    let attrs = crate::classes::attrs(Some(p), &o.class);
    let mut out: Vec<[String; 2]> = vec![];
    for a in &attrs {
        if let Some(v) = o.data.get(&a.key) {
            let s = shown(v, &a.q, us);
            if !s.trim().is_empty() {
                out.push([a.label.clone(), s]);
            }
        }
    }
    for (k, v) in &o.data {
        if k.starts_with('~') || attrs.iter().any(|a| &a.key == k) {
            continue;
        }
        let s = shown(v, "", us);
        if !s.trim().is_empty() {
            out.push([k.clone(), s]);
        }
    }
    out
}

/// The web page, as HTML text. `stamp` is shown as when it was saved (a date).
pub fn page(p: &Project, stamp: &str) -> String {
    let us = p.units == "US";
    let net = Net::new(p);
    let hyd = crate::hydraulics::all(p, &net);
    let mut sheets: Vec<Value> = vec![];
    let mut ids: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut objs: serde_json::Map<String, Value> = serde_json::Map::new();
    let add_obj = |obj: u32, objs: &mut serde_json::Map<String, Value>| {
        if obj == 0 || objs.contains_key(&obj.to_string()) {
            return;
        }
        if let Some(o) = p.obj(obj) {
            let class = crate::classes::get(&o.class).map_or(o.class.clone(), |c| c.label.clone());
            objs.insert(obj.to_string(), json!({ "tag": o.tag, "class": class, "rows": rows(p, obj, us) }));
        }
    };
    for (si, s) in p.sheets.iter().enumerate() {
        let v = view::sheet(p, s, &net, &hyd);
        let mut b = String::with_capacity(1 << 15);
        b += &format!("<rect width=\"{}\" height=\"{}\" fill=\"#fff\" stroke=\"none\"/>", n(v.w), n(v.h));
        b += "<g class=\"fr\">";
        v.frame.iter().for_each(|x| b += &prim(x));
        b += "</g>";
        for l in &v.lines {
            let dash = if l.kind == crate::model::Kind::Electric { " stroke-dasharray=\"1.5 1\"" } else { "" };
            b += &format!("<g class=\"o\" data-i=\"{}\"><g stroke-width=\"{}\"{dash}>", l.id, n(width(l.kind)));
            for seg in &l.segs {
                let d: Vec<String> = seg.iter().map(|q| format!("{},{}", n(q[0]), n(q[1]))).collect();
                b += &format!("<polyline points=\"{}\"/>", d.join(" "));
            }
            // A wide invisible stroke to make thin lines easy to click.
            for seg in &l.segs {
                let d: Vec<String> = seg.iter().map(|q| format!("{},{}", n(q[0]), n(q[1]))).collect();
                b += &format!("<polyline class=\"hit\" points=\"{}\"/>", d.join(" "));
            }
            b += "</g>";
            l.marks.iter().for_each(|x| b += &prim(x));
            b += "</g>";
            let c = l.path.get(l.path.len() / 2).copied().unwrap_or([0.0, 0.0]);
            ids.insert(l.id.to_string(), json!({ "s": si, "o": l.obj, "x": c[0], "y": c[1], "n": l.number }));
            add_obj(l.obj, &mut objs);
        }
        for it in &v.items {
            let Some(item) = p.item(it.id) else { continue };
            let draw = symbols::get(&item.sym).map(|s| symbols::parse(s.draw)).unwrap_or_default();
            let shapes: Vec<Prim> = draw.into_iter().filter(|x| !matches!(x, Prim::Text { .. })).collect();
            b += &format!("<g class=\"o\" data-i=\"{}\"><rect class=\"hit\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>", it.id, n(it.bbox[0]), n(it.bbox[1]), n((it.bbox[2] - it.bbox[0]).max(1.0)), n((it.bbox[3] - it.bbox[1]).max(1.0)));
            Place::of(item).prims(&shapes).iter().for_each(|x| b += &prim(x));
            it.texts.iter().for_each(|x| b += &prim(x));
            b += "</g>";
            let link = if it.cat == Cat::Connector && item.link != 0 { item.link } else { 0 };
            ids.insert(it.id.to_string(), json!({ "s": si, "o": it.obj, "x": it.x, "y": it.y, "l": link }));
            add_obj(it.obj, &mut objs);
        }
        for nt in &v.notes {
            nt.prims.iter().for_each(|x| b += &prim(x));
        }
        v.clouds.iter().for_each(|x| b += &prim(x));
        for r in &v.legend_rows {
            if let Some(sym) = r.sym.as_deref().and_then(symbols::get) {
                let sc = (30.0 / sym.w).min(1.0);
                b += &format!("<g transform=\"translate({} {}) scale({})\">", n(r.x + 15.0), n(r.y), n(sc));
                symbols::parse(sym.draw).iter().for_each(|x| b += &prim(x));
                b += "</g>";
            }
            if let Some(k) = r.kind {
                b += &format!("<polyline points=\"{},{} {},{}\" stroke-width=\"{}\"/>", n(r.x), n(r.y), n(r.x + 30.0), n(r.y), n(width(k)));
            }
            let head = r.sym.is_none() && r.kind.is_none();
            b += &prim(&Prim::Text { p: [if head { r.x } else { r.x + 36.0 }, r.y], h: if head { 3.0 } else { 2.5 }, s: r.name.clone(), anchor: 0, bold: head });
        }
        sheets.push(json!({ "number": s.number, "title": s.title, "rev": s.rev, "w": v.w, "h": v.h, "svg": b }));
    }
    let mut lists: Vec<Value> = vec![];
    for kind in ["lines", "streams", "equipment", "instruments", "valves"] {
        let Ok(l) = crate::lists::build(p, kind) else { continue };
        if l.rows.is_empty() {
            continue;
        }
        let cols: Vec<&str> = l.cols.iter().map(|c| c.label.as_str()).collect();
        let rows: Vec<Value> = l.rows.iter().map(|r| json!({ "id": r.ids.first().copied().unwrap_or(0), "c": r.cells })).collect();
        lists.push(json!({ "title": l.title, "cols": cols, "rows": rows }));
    }
    let title = if p.title.project.trim().is_empty() { p.name.clone() } else { p.title.project.clone() };
    let data = json!({ "name": title, "company": p.title.company, "stamp": stamp, "units": if us { "US" } else { "SI" }, "sheets": sheets, "ids": ids, "objs": objs, "lists": lists });
    // Inside a script element no "<" may appear at all ("</script>", "<!--"): JSON writes it "\u003c".
    let data = data.to_string().replace('<', "\\u003c");
    TEMPLATE.replace("{{TITLE}}", &crate::xml(&title)).replace("{{VERSION}}", env!("CARGO_PKG_VERSION")).replacen("{{DATA}}", &data, 1)
}

const TEMPLATE: &str = include_str!("webpage.html");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_example_makes_a_page_with_its_sheets_tags_and_lists() {
        for ex in crate::examples::list().iter().map(|e| e.id) {
            let p = crate::examples::get(ex).unwrap();
            let h = page(&p, "2026-11-12");
            assert!(h.starts_with("<!doctype html>") && h.contains("</html>"), "{ex}");
            let json = h.split("<script id=\"data\" type=\"application/json\">").nth(1).unwrap().split("</script>").next().unwrap();
            let v: Value = serde_json::from_str(json).unwrap();
            assert_eq!(v["sheets"].as_array().unwrap().len(), p.sheets.len());
            // Every drawn symbol and line can be clicked, and every tagged object has its data.
            for it in &p.items {
                assert!(v["ids"].get(it.id.to_string()).is_some(), "{ex} item {}", it.id);
            }
            for o in p.objects.iter().filter(|o| !o.tag.is_empty() && p.items.iter().any(|i| i.obj == o.id)) {
                assert_eq!(v["objs"][o.id.to_string()]["tag"], o.tag.as_str());
            }
            assert!(v["lists"].as_array().unwrap().len() >= 2, "{ex}");
            assert!(!h.contains("<script src") && !h.contains("fetch("), "the page fetches nothing");
        }
        // The pump's data, in its units.
        let p = crate::examples::get("pump").unwrap();
        let h = page(&p, "");
        assert!(h.contains("P-101A") && h.contains("HV-104"));
    }

    #[test]
    fn hostile_text_stays_text() {
        let mut p = crate::examples::get("pump").unwrap();
        let o = p.objects.iter_mut().find(|o| o.tag == "P-101A").unwrap();
        o.data.insert("service".into(), Val::Text("</script><script>alert(1)</script><!--".into()));
        p.title.project = "<img src=x onerror=alert(1)>".into();
        let h = page(&p, "<b>");
        assert!(!h.contains("<script>alert(1)") && !h.contains("<img src=x"));
        assert_eq!(h.matches("</script>").count(), 2, "only the page's own two scripts end");
        let json = h.split("<script id=\"data\" type=\"application/json\">").nth(1).unwrap().split("</script>").next().unwrap();
        let v: Value = serde_json::from_str(json).unwrap();
        assert_eq!(v["name"], "<img src=x onerror=alert(1)>");
    }
}
