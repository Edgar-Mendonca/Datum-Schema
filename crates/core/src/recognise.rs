//! Symbols recognised from loose lines. A drawing printed to PDF, or a DXF whose blocks were
//! exploded, has its symbols as plain circles, triangles and lines; the common ones are found by
//! their shape and put back as symbols, so they convert like blocks do:
//! - instrument bubbles (a circle 7 to 18 mm across with its letters and number inside): in a square
//!   a shared display, with a line across a control-room instrument, else field-mounted;
//! - valves drawn as a bow tie (two triangles meeting at a point): with a filled dot a globe valve,
//!   with a circle a ball valve, with a stem and an actuator a control valve, else a gate valve;
//! - pumps: a circle whose nearest tag starts with P.
//!
//! Each shape found becomes an insert named "~<symbol key>" (a bubble's tag in its TAG attribute)
//! and the lines it was made of are taken out, so they are not read again as pipes.

use crate::dxfin::{Attrib, Dxf, Ent};

/// The prefix of the block name a recognised shape is given.
pub const SHAPE: &str = "~";

#[derive(Clone, Copy)]
struct Circle {
    i: usize,
    c: [f64; 2],
    r: f64,
    fill: bool,
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// A closed shape's corners with a repeated last point dropped.
fn corners(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut v = pts.to_vec();
    while v.len() > 2 && dist(v[0], v[v.len() - 1]) < 1e-6 {
        v.pop();
    }
    v
}

fn centroid(pts: &[[f64; 2]]) -> [f64; 2] {
    let n = pts.len().max(1) as f64;
    [pts.iter().map(|p| p[0]).sum::<f64>() / n, pts.iter().map(|p| p[1]).sum::<f64>() / n]
}

/// A closed polyline that is a circle drawn with many points (curves flattened): its centre and
/// radius.
fn roundish(pts: &[[f64; 2]]) -> Option<([f64; 2], f64)> {
    let v = corners(pts);
    if v.len() < 8 {
        return None;
    }
    let c = centroid(&v);
    let r = v.iter().map(|p| dist(*p, c)).sum::<f64>() / v.len() as f64;
    (r > 0.0 && v.iter().all(|p| (dist(*p, c) - r).abs() < 0.12 * r)).then_some((c, r))
}

/// Recognises the shapes in a drawing's model space. Returns how many were found.
pub fn recognise(d: &mut Dxf) -> usize {
    let mm = if d.mm > 0.0 && d.mm.is_finite() { d.mm } else { 1.0 };
    let n = d.ents.len();
    if n > 200_000 {
        return 0;
    }
    let mut gone = vec![false; n];
    let mut found: Vec<Ent> = vec![];
    // Everything in millimetres.
    let m = |p: [f64; 2]| [p[0] * mm, p[1] * mm];
    let mut circles: Vec<Circle> = vec![];
    let mut tris: Vec<(usize, Vec<[f64; 2]>)> = vec![];
    let mut rects: Vec<(usize, [f64; 4])> = vec![];
    let mut opens: Vec<(usize, Vec<[f64; 2]>)> = vec![];
    let mut texts: Vec<(usize, [f64; 2], String)> = vec![];
    for (i, e) in d.ents.iter().enumerate() {
        match e {
            Ent::Circle { c, r } => circles.push(Circle { i, c: m(*c), r: r * mm, fill: false }),
            Ent::Poly { pts, closed: true, fill } => {
                let pts: Vec<[f64; 2]> = pts.iter().map(|p| m(*p)).collect();
                let v = corners(&pts);
                if let Some((c, r)) = roundish(&pts) {
                    circles.push(Circle { i, c, r, fill: *fill });
                } else if v.len() == 3 {
                    tris.push((i, v));
                } else if v.len() == 4 {
                    let (x0, x1) = (v.iter().map(|p| p[0]).fold(f64::MAX, f64::min), v.iter().map(|p| p[0]).fold(f64::MIN, f64::max));
                    let (y0, y1) = (v.iter().map(|p| p[1]).fold(f64::MAX, f64::min), v.iter().map(|p| p[1]).fold(f64::MIN, f64::max));
                    // Axis-aligned: every corner on the box.
                    let tol = 0.02 * (x1 - x0).max(y1 - y0);
                    if v.iter().all(|p| ((p[0] - x0).abs() < tol || (p[0] - x1).abs() < tol) && ((p[1] - y0).abs() < tol || (p[1] - y1).abs() < tol)) {
                        rects.push((i, [x0, y0, x1, y1]));
                    }
                }
            }
            Ent::Poly { pts, closed: false, .. } => opens.push((i, pts.iter().map(|p| m(*p)).collect())),
            Ent::Text { p, h, s, anchor, .. } => {
                // Roughly the middle of the text.
                let (h, w) = (h * mm, 0.6 * h * mm * s.chars().count() as f64);
                let x = match anchor { 1 => 0.0, 2 => -w / 2.0, _ => w / 2.0 };
                texts.push((i, [p[0] * mm + x, p[1] * mm + 0.35 * h], s.trim().to_string()));
            }
            _ => {}
        }
    }
    let ins = |sym: &str, c: [f64; 2], rot: f64, tag: Option<String>| Ent::Insert {
        name: format!("{SHAPE}{sym}"),
        p: [c[0] / mm, c[1] / mm],
        sx: 1.0,
        sy: 1.0,
        rot,
        attribs: tag.map(|t| vec![Attrib { tag: "TAG".into(), value: t, p: [c[0] / mm, c[1] / mm], h: 2.5 / mm, visible: true }]).unwrap_or_default(),
    };
    // Open lines lying wholly within a found shape are part of it.
    let inside = |opens: &[(usize, Vec<[f64; 2]>)], gone: &mut Vec<bool>, c: [f64; 2], r: f64| {
        for (i, pts) in opens {
            if pts.iter().all(|p| dist(*p, c) <= r) {
                gone[*i] = true;
            }
        }
    };

    // Instrument bubbles.
    for b in circles.iter().filter(|b| !b.fill && (3.5..=9.0).contains(&b.r)) {
        let mut inner: Vec<&(usize, [f64; 2], String)> = texts.iter().filter(|t| !gone[t.0] && dist(t.1, b.c) < 0.95 * b.r && !t.2.is_empty()).collect();
        if inner.is_empty() || inner.len() > 3 {
            continue;
        }
        inner.sort_by(|x, y| y.1[1].total_cmp(&x.1[1]));
        let letters = inner.iter().find(|t| t.2.chars().all(|c| c.is_ascii_alphabetic()) && t.2.len() <= 5);
        let number = inner.iter().find(|t| t.2.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.2.len() <= 8);
        let tag = match (letters, number) {
            (Some(l), Some(nn)) => Some(format!("{}-{}", l.2.to_uppercase(), nn.2)),
            _ => inner.iter().map(|t| t.2.replace(' ', "-").to_uppercase()).find(|t| crate::tags::split(t).1.is_some() && t.chars().next().is_some_and(|c| c.is_ascii_alphabetic())),
        };
        let Some(tag) = tag else { continue };
        // A square round it: a shared display.
        let sq = rects.iter().find(|(i, r)| !gone[*i] && dist([(r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0], b.c) < 0.25 * b.r && ((r[2] - r[0]) / (2.0 * b.r) - 1.0).abs() < 0.25 && ((r[3] - r[1]) / (2.0 * b.r) - 1.0).abs() < 0.25);
        // A line straight across: a control-room instrument.
        let across = opens.iter().any(|(i, p)| !gone[*i] && p.len() == 2 && (p[0][1] - b.c[1]).abs() < 0.2 * b.r && (p[1][1] - b.c[1]).abs() < 0.2 * b.r && (p[0][0] - p[1][0]).abs() > 1.6 * b.r && p.iter().all(|q| dist(*q, b.c) < 1.1 * b.r));
        let sym = match (sq, across) {
            (Some((i, _)), _) => {
                gone[*i] = true;
                "inst_dcs"
            }
            (None, true) => "inst_panel",
            _ => "inst_field",
        };
        gone[b.i] = true;
        for t in &inner {
            gone[t.0] = true;
        }
        inside(&opens, &mut gone, b.c, 1.05 * b.r);
        found.push(ins(sym, b.c, 0.0, Some(tag)));
    }

    // Bow-tie valves.
    let mut used_tri = vec![false; tris.len()];
    for a in 0..tris.len() {
        for bi in a + 1..tris.len() {
            if used_tri[a] || used_tri[bi] || gone[tris[a].0] || gone[tris[bi].0] {
                continue;
            }
            let (ta, tb) = (&tris[a].1, &tris[bi].1);
            let size = |t: &[[f64; 2]]| t.iter().flat_map(|p| t.iter().map(move |q| dist(*p, *q))).fold(0.0, f64::max);
            let (sa, sb) = (size(ta), size(tb));
            if !(1.5..=14.0).contains(&sa) || (sa / sb - 1.0).abs() > 0.3 {
                continue;
            }
            // A corner they share.
            let Some(apex) = ta.iter().find(|p| tb.iter().any(|q| dist(**p, *q) < 0.08 * sa)).copied() else { continue };
            let (ca, cb) = (centroid(ta), centroid(tb));
            let (da, db) = ([ca[0] - apex[0], ca[1] - apex[1]], [cb[0] - apex[0], cb[1] - apex[1]]);
            // On opposite sides of the point, along one axis.
            if da[0] * db[0] + da[1] * db[1] >= 0.0 {
                continue;
            }
            let axis = [cb[0] - ca[0], cb[1] - ca[1]];
            let horiz = axis[0].abs() >= axis[1].abs();
            let half = (axis[0].hypot(axis[1]) * 0.75).max(1.0);
            used_tri[a] = true;
            used_tri[bi] = true;
            gone[tris[a].0] = true;
            gone[tris[bi].0] = true;
            // A dot or a circle at the point.
            let mut sym = "gate";
            if let Some(c) = circles.iter().find(|c| !gone[c.i] && dist(c.c, apex) < 0.15 * sa && c.r < 0.45 * sa) {
                sym = if c.fill { "globe" } else { "ball" };
                gone[c.i] = true;
            }
            let mut rot = if horiz { 0.0 } else { 90.0 };
            // A stem across the axis from the point, with something on its end: a control valve.
            let stem = opens.iter().find(|(i, p)| {
                !gone[*i] && p.len() == 2 && {
                    let (near, far) = if dist(p[0], apex) < dist(p[1], apex) { (p[0], p[1]) } else { (p[1], p[0]) };
                    let v = [far[0] - near[0], far[1] - near[1]];
                    dist(near, apex) < 0.15 * sa && (1.5..=12.0).contains(&v[0].hypot(v[1])) && if horiz { v[1].abs() > 3.0 * v[0].abs() } else { v[0].abs() > 3.0 * v[1].abs() }
                }
            });
            if let Some((si, sp)) = stem {
                let far = if dist(sp[0], apex) < dist(sp[1], apex) { sp[1] } else { sp[0] };
                let top = opens.iter().find(|(i, p)| *i != *si && !gone[*i] && p.iter().all(|q| dist(*q, far) < 0.9 * sa) && p.len() >= 2).map(|x| x.0).or_else(|| circles.iter().find(|c| !gone[c.i] && dist(c.c, far) < 0.6 * sa).map(|c| c.i)).or_else(|| rects.iter().find(|(i, r)| !gone[*i] && far[0] >= r[0] - 0.5 && far[0] <= r[2] + 0.5 && far[1] >= r[1] - 0.5 && far[1] <= r[3] + 0.5).map(|x| x.0));
                if let Some(t) = top {
                    sym = "cv_pneu";
                    gone[*si] = true;
                    gone[t] = true;
                    // The stem up (y up in the drawing) is the symbol's own way.
                    let v = [far[0] - apex[0], far[1] - apex[1]];
                    rot = if horiz { if v[1] > 0.0 { 0.0 } else { 180.0 } } else if v[0] < 0.0 { 90.0 } else { 270.0 };
                    inside(&opens, &mut gone, far, 0.9 * sa);
                }
            }
            inside(&opens, &mut gone, apex, half);
            found.push(ins(sym, apex, rot, None));
        }
    }

    // Pumps: a plain circle with a pump's tag beside it.
    for b in circles.iter().filter(|b| !b.fill && (4.0..=15.0).contains(&b.r)) {
        if gone[b.i] {
            continue;
        }
        let tag = texts.iter().filter(|t| !gone[t.0] && dist(t.1, b.c) < 4.0 * b.r).min_by(|x, y| dist(x.1, b.c).total_cmp(&dist(y.1, b.c)));
        let Some(t) = tag else { continue };
        let (pre, num, _) = crate::tags::split(&t.2);
        if !(pre.eq_ignore_ascii_case("P") && num.is_some()) {
            continue;
        }
        gone[b.i] = true;
        inside(&opens, &mut gone, b.c, 1.8 * b.r);
        found.push(ins("pump_cent", b.c, 0.0, None));
    }

    let count = found.len();
    if count > 0 {
        let mut ents = vec![];
        let mut layers = vec![];
        for (i, e) in std::mem::take(&mut d.ents).into_iter().enumerate() {
            if !gone[i] {
                ents.push(e);
                layers.push(d.layers.get(i).cloned().unwrap_or_default());
            }
        }
        for e in found {
            ents.push(e);
            layers.push("SHAPES".into());
        }
        d.ents = ents;
        d.layers = layers;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(d: &Dxf) -> Vec<String> {
        d.ents.iter().filter_map(|e| match e { Ent::Insert { name, .. } => Some(name.clone()), _ => None }).collect()
    }

    #[test]
    fn shapes_on_a_pdf_page_become_symbols() {
        let (mut d, _) = crate::pdfin::read(&crate::pdfin::tests::sample(true), 0).unwrap();
        let before = d.ents.len();
        assert_eq!(recognise(&mut d), 2);
        let n = names(&d);
        assert!(n.contains(&"~inst_field".to_string()) && n.contains(&"~gate".to_string()), "{n:?}");
        // The bubble's tag from the two texts inside it; those texts and the shapes are gone.
        let tag = d.ents.iter().find_map(|e| match e { Ent::Insert { name, attribs, .. } if name == "~inst_field" => attribs.first().map(|a| a.value.clone()), _ => None });
        assert_eq!(tag.as_deref(), Some("FIC-101"));
        assert!(!d.ents.iter().any(|e| matches!(e, Ent::Text { s, .. } if s == "FIC")));
        assert!(d.ents.len() < before);
        assert_eq!(d.ents.len(), d.layers.len());
        // The valve sits at the triangles' shared point.
        let at = d.ents.iter().find_map(|e| match e { Ent::Insert { name, p, .. } if name == "~gate" => Some(*p), _ => None }).unwrap();
        assert!((at[0] - 296.0).abs() < 1e-6 && (at[1] - 200.0).abs() < 1e-6);
    }

    fn poly(pts: &[[f64; 2]], closed: bool, fill: bool) -> Ent {
        Ent::Poly { pts: pts.to_vec(), closed, fill }
    }

    fn circle_pts(c: [f64; 2], r: f64) -> Vec<[f64; 2]> {
        (0..24).map(|k| { let a = k as f64 * std::f64::consts::TAU / 24.0; [c[0] + r * a.cos(), c[1] + r * a.sin()] }).collect()
    }

    fn text(p: [f64; 2], s: &str) -> Ent {
        Ent::Text { p, h: 2.5, s: s.into(), rot: 0.0, anchor: 1 }
    }

    #[test]
    fn each_kind_of_shape_is_told_apart() {
        // In millimetres: a shared display, a panel instrument, a vertical globe valve, a ball
        // valve, a control valve with its stem down, a pump, and a circle with no tag (left alone).
        let mut ents = vec![
            Ent::Circle { c: [20.0, 100.0], r: 5.5 },
            poly(&[[14.5, 94.5], [25.5, 94.5], [25.5, 105.5], [14.5, 105.5]], true, false),
            text([20.0, 101.5], "TI"),
            text([20.0, 97.5], "205"),
            poly(&circle_pts([50.0, 100.0], 5.5), true, false),
            poly(&[[44.5, 100.0], [55.5, 100.0]], false, false),
            text([50.0, 102.0], "PIC"),
            text([50.0, 97.0], "7"),
            // Globe, vertical: triangles above and below (100, 50).
            poly(&[[97.5, 46.0], [102.5, 46.0], [100.0, 50.0]], true, false),
            poly(&[[97.5, 54.0], [102.5, 54.0], [100.0, 50.0]], true, false),
            poly(&circle_pts([100.0, 50.0], 1.3), true, true),
            // Ball, horizontal at (130, 50).
            poly(&[[126.0, 47.5], [126.0, 52.5], [130.0, 50.0]], true, false),
            poly(&[[134.0, 47.5], [134.0, 52.5], [130.0, 50.0]], true, false),
            Ent::Circle { c: [130.0, 50.0], r: 1.7 },
            // Control valve at (160, 50), stem down to an arc.
            poly(&[[156.0, 47.5], [156.0, 52.5], [160.0, 50.0]], true, false),
            poly(&[[164.0, 47.5], [164.0, 52.5], [160.0, 50.0]], true, false),
            poly(&[[160.0, 50.0], [160.0, 45.0]], false, false),
            poly(&[[157.0, 45.0], [158.0, 43.0], [160.0, 42.0], [162.0, 43.0], [163.0, 45.0]], false, false),
            // Pump at (200, 50) with its tag below and its base.
            poly(&circle_pts([200.0, 50.0], 6.0), true, false),
            poly(&[[195.5, 46.0], [194.0, 42.0], [206.0, 42.0], [204.5, 46.0]], false, false),
            text([200.0, 36.0], "P-101A"),
            // A circle on its own.
            Ent::Circle { c: [250.0, 50.0], r: 6.0 },
            // A pipe, which stays.
            poly(&[[60.0, 50.0], [126.0, 50.0]], false, false),
        ];
        ents.push(text([250.0, 70.0], "NOTE 1"));
        let layers = vec!["0".to_string(); ents.len()];
        let mut d = Dxf { mm: 1.0, ents, layers, ..Dxf::default() };
        assert_eq!(recognise(&mut d), 6);
        let got: Vec<(String, f64, String)> = d.ents.iter().filter_map(|e| match e { Ent::Insert { name, rot, attribs, .. } => Some((name.clone(), *rot, attribs.first().map(|a| a.value.clone()).unwrap_or_default())), _ => None }).collect();
        for want in [("~inst_dcs", 0.0, "TI-205"), ("~inst_panel", 0.0, "PIC-7"), ("~globe", 90.0, ""), ("~ball", 0.0, ""), ("~cv_pneu", 180.0, ""), ("~pump_cent", 0.0, "")] {
            assert!(got.iter().any(|g| g.0 == want.0 && g.1 == want.1 && g.2 == want.2), "{want:?} in {got:?}");
        }
        // The pipe, the lone circle, the pump's tag and the note stay; the pump's base and the
        // actuator went with their shapes.
        assert!(d.ents.iter().any(|e| matches!(e, Ent::Poly { pts, .. } if pts.len() == 2 && pts[0] == [60.0, 50.0])));
        assert!(d.ents.iter().any(|e| matches!(e, Ent::Circle { c, .. } if c[0] == 250.0)));
        assert!(d.ents.iter().any(|e| matches!(e, Ent::Text { s, .. } if s == "P-101A")));
        assert!(!d.ents.iter().any(|e| matches!(e, Ent::Poly { pts, .. } if pts.len() == 4 && pts[0] == [195.5, 46.0])));
        assert!(!d.ents.iter().any(|e| matches!(e, Ent::Poly { pts, .. } if pts.len() == 5)));
    }

    #[test]
    fn a_drawing_with_nothing_to_recognise_is_left_alone() {
        let mut d = crate::dxfin::read(crate::dxfin::tests::SAMPLE).unwrap();
        let n = d.ents.len();
        assert_eq!(recognise(&mut d), 0);
        assert_eq!(d.ents.len(), n);
    }
}
