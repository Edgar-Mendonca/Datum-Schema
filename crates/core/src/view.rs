//! What the window draws for one sheet, worked out by the engine: the frame, each placed symbol
//! (with its ports and upright texts), each line's route (orthogonal, around symbols, with a gap
//! where it crosses another line), its flow arrows and signal marks, its line number and
//! hydraulic badge, the notes and Hold clouds, and the legend on a legend sheet. The printout,
//! the SVG export and the DXF use the same pieces.

use crate::frame;
use crate::graph::{self, end_dir, end_pt, Net};
use crate::hydraulics::{self, Hyd};
use crate::model::{End, Kind, NoteKind, Project, Sheet};
use crate::route::{self, Rect, Req};
use crate::symbols::{self, Cat, Place, Prim};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Serialize, Clone, Debug)]
pub struct PortView {
    pub p: [f64; 2],
    pub dir: u8,
    pub signal: bool,
    pub used: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct ItemView {
    pub id: u32,
    /// The object this symbol shows.
    pub obj: u32,
    pub sym: String,
    pub cat: Cat,
    pub x: f64,
    pub y: f64,
    pub rot: u8,
    pub flip: bool,
    pub scale: f64,
    pub tag: String,
    /// Upright texts: the symbol's own letters, the tag and other labels.
    pub texts: Vec<Prim>,
    pub bbox: [f64; 4],
    pub ports: Vec<PortView>,
    pub layer: &'static str,
}

#[derive(Serialize, Clone, Debug)]
pub struct LineView {
    pub id: u32,
    /// The line-number object (0 for signal lines).
    pub obj: u32,
    pub kind: Kind,
    /// The route, end to end, for picking and dragging.
    pub path: Vec<[f64; 2]>,
    /// The drawn pieces (the route with gaps where it hops over other lines).
    pub segs: Vec<Vec<[f64; 2]>>,
    /// Flow arrows, signal marks, the line number and the hydraulic badge.
    pub marks: Vec<Prim>,
    pub number: String,
    /// The hydraulic badge: 0 ✓, 1 !, 2 ✗, or none.
    pub badge: Option<u8>,
    pub run: Option<usize>,
    pub layer: &'static str,
}

#[derive(Serialize, Clone, Debug)]
pub struct NoteView {
    pub id: u32,
    pub kind: NoteKind,
    pub prims: Vec<Prim>,
    pub bbox: [f64; 4],
}

#[derive(Serialize, Clone, Debug)]
pub struct LegendRow {
    pub sym: Option<String>,
    pub kind: Option<Kind>,
    pub x: f64,
    pub y: f64,
    pub name: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct SheetView {
    pub id: u32,
    pub number: String,
    pub title: String,
    pub w: f64,
    pub h: f64,
    pub legend: bool,
    pub frame: Vec<Prim>,
    pub items: Vec<ItemView>,
    pub lines: Vec<LineView>,
    pub notes: Vec<NoteView>,
    pub legend_rows: Vec<LegendRow>,
    /// An old drawing behind the sheet (its drawing is fetched by `rev`, with `underlay_svg`).
    pub underlay: Option<crate::model::Underlay>,
    /// Revision clouds round the changes since the issued revision the sheet clouds against.
    pub clouds: Vec<Prim>,
    /// Where the company logo goes in the title block (x, y, w, h).
    pub logo: Option<[f64; 4]>,
    /// Review comments pinned to the sheet.
    pub pins: Vec<crate::review::Pin>,
}

/// The layer a line type is drawn on.
pub fn line_layer(k: Kind) -> &'static str {
    match k {
        Kind::Major | Kind::Minor => "process",
        Kind::Utility => "utility",
        _ => "instrument",
    }
}

/// Line widths (mm) by type.
pub fn line_width(k: Kind) -> f64 {
    match k {
        Kind::Major => 0.6,
        Kind::Utility => 0.45,
        Kind::Minor => 0.35,
        _ => 0.25,
    }
}

/// An item's box on the sheet.
pub fn bbox(it: &crate::model::Item) -> [f64; 4] {
    let Some(s) = symbols::get(&it.sym) else { return [it.x - 5.0, it.y - 5.0, it.x + 5.0, it.y + 5.0] };
    let pl = Place::of(it);
    let (a, b) = (pl.pt([-s.w / 2.0, -s.h / 2.0]), pl.pt([s.w / 2.0, s.h / 2.0]));
    [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])]
}

/// Routes every line on a sheet (without the item `skip`, which may sit over a line while it is
/// being dropped into it). Returns each line's route.
pub fn routes(p: &Project, sheet: u32, skip: Option<u32>) -> HashMap<u32, Vec<[f64; 2]>> {
    let rects: Vec<Rect> = p
        .items
        .iter()
        .filter(|i| i.sheet == sheet && Some(i.id) != skip && symbols::get(&i.sym).is_some_and(|s| s.cat != Cat::Mark || s.inline))
        .map(|i| {
            let b = bbox(i);
            Rect { x: b[0], y: b[1], w: b[2] - b[0], h: b[3] - b[1] }
        })
        .collect();
    let lines: Vec<&crate::model::Line> = p.lines.iter().filter(|l| l.sheet == sheet).collect();
    let mut out: HashMap<u32, Vec<[f64; 2]>> = HashMap::new();
    let req = |l: &crate::model::Line, out: &HashMap<u32, Vec<[f64; 2]>>| -> Req {
        // A branch's end sits on the route of the line it joins.
        let pt = |e: &End| -> [f64; 2] {
            match e {
                End::Line { line, x, y } => out.get(line).map_or([*x, *y], |path| graph::nearest_on(path, [*x, *y]).0),
                _ => end_pt(p, e),
            }
        };
        let (a, b) = (pt(&l.a), pt(&l.b));
        let d = |e: &End| end_dir(p, e).map(|v| (v[0], v[1]));
        Req { a: (a[0], a[1]), dir_a: d(&l.a), b: (b[0], b[1]), dir_b: d(&l.b), pts: l.pts.iter().map(|q| (q[0], q[1])).collect(), group: l.id as u64 }
    };
    // Lines that end on other lines go last, so the lines they join are routed first.
    let (plain, branches): (Vec<&&crate::model::Line>, Vec<&&crate::model::Line>) = lines.iter().partition(|l| !matches!(l.a, End::Line { .. }) && !matches!(l.b, End::Line { .. }));
    for group in [plain, branches] {
        let reqs: Vec<Req> = group.iter().map(|l| req(l, &out)).collect();
        for (l, path) in group.iter().zip(route::route(&rects, &reqs)) {
            out.insert(l.id, path.into_iter().map(|q| [q.0, q.1]).collect());
        }
    }
    out
}

const GAP: f64 = 1.2;

/// Splits routes where they cross: the line of lower rank (a signal under a process line; else the
/// vertical one) gets a small gap.
fn hops(paths: &[(u32, Kind, Vec<[f64; 2]>)]) -> HashMap<u32, Vec<Vec<[f64; 2]>>> {
    // Gaps per line: (segment index, point).
    let mut gaps: HashMap<u32, Vec<(usize, [f64; 2])>> = HashMap::new();
    let rank = |k: Kind| if k.process() { 1 } else { 0 };
    for (i, (ia, ka, pa)) in paths.iter().enumerate() {
        for (ib, kb, pb) in paths.iter().skip(i + 1) {
            for (sa, wa) in pa.windows(2).enumerate() {
                for (sb, wb) in pb.windows(2).enumerate() {
                    let ha = (wa[0][1] - wa[1][1]).abs() < 1e-9;
                    let hb = (wb[0][1] - wb[1][1]).abs() < 1e-9;
                    if ha == hb {
                        continue;
                    }
                    let (h, v) = if ha { (wa, wb) } else { (wb, wa) };
                    let (x, y) = (v[0][0], h[0][1]);
                    let inside = |a: f64, b: f64, t: f64| t > a.min(b) + GAP * 1.5 && t < a.max(b) - GAP * 1.5;
                    if !(inside(h[0][0], h[1][0], x) && inside(v[0][1], v[1][1], y)) {
                        continue;
                    }
                    // Who gets the gap: the lower rank, else the vertical segment.
                    let a_gets = match rank(*ka).cmp(&rank(*kb)) {
                        std::cmp::Ordering::Less => true,
                        std::cmp::Ordering::Greater => false,
                        _ => !ha,
                    };
                    if a_gets {
                        gaps.entry(*ia).or_default().push((sa, [x, y]));
                    } else {
                        gaps.entry(*ib).or_default().push((sb, [x, y]));
                    }
                }
            }
        }
    }
    let mut out = HashMap::new();
    for (id, _, path) in paths {
        let mut g = gaps.remove(id).unwrap_or_default();
        if g.is_empty() {
            out.insert(*id, vec![path.clone()]);
            continue;
        }
        g.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| {
            let s = path[a.0];
            let da = (a.1[0] - s[0]).abs() + (a.1[1] - s[1]).abs();
            let db = (b.1[0] - s[0]).abs() + (b.1[1] - s[1]).abs();
            da.total_cmp(&db)
        }));
        let mut pieces = vec![];
        let mut cur = vec![path[0]];
        let mut gi = 0;
        for k in 0..path.len() - 1 {
            let (a, b) = (path[k], path[k + 1]);
            let len = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
            let u = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
            while gi < g.len() && g[gi].0 == k {
                let q = g[gi].1;
                cur.push([q[0] - u[0] * GAP, q[1] - u[1] * GAP]);
                pieces.push(std::mem::take(&mut cur));
                cur.push([q[0] + u[0] * GAP, q[1] + u[1] * GAP]);
                gi += 1;
            }
            cur.push(b);
        }
        pieces.push(cur);
        out.insert(*id, pieces);
    }
    out
}

fn tri(tip: [f64; 2], u: [f64; 2], len: f64, half: f64) -> Prim {
    let base = [tip[0] - u[0] * len, tip[1] - u[1] * len];
    let n = [-u[1], u[0]];
    Prim::Poly { pts: vec![tip, [base[0] + n[0] * half, base[1] + n[1] * half], [base[0] - n[0] * half, base[1] - n[1] * half]], closed: true, fill: true, dash: false, w: 0.2 }
}

/// Flow arrows (process lines) or signal marks (pneumatic //, capillary ×, data link ○) along a route.
fn line_marks(kind: Kind, path: &[[f64; 2]], forward: bool) -> Vec<Prim> {
    let mut out = vec![];
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
        if len < 1e-9 {
            continue;
        }
        let u = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
        let n = [-u[1], u[0]];
        let at = |t: f64| [a[0] + u[0] * t, a[1] + u[1] * t];
        if kind.process() {
            if len >= 12.0 {
                let m = at(len / 2.0);
                let d = if forward { u } else { [-u[0], -u[1]] };
                out.push(tri([m[0] + d[0] * 1.6, m[1] + d[1] * 1.6], d, 3.2, 1.1));
            }
            continue;
        }
        let step = 10.0;
        let k = (len / step).floor() as usize;
        for i in 0..k {
            let m = at(step * (i as f64 + 0.5) + (len - step * k as f64) / 2.0);
            match kind {
                Kind::Pneumatic => {
                    for o in [-0.6, 0.6] {
                        let c = [m[0] + u[0] * o, m[1] + u[1] * o];
                        out.push(Prim::Poly { pts: vec![[c[0] - n[0] * 1.3 - u[0] * 0.8, c[1] - n[1] * 1.3 - u[1] * 0.8], [c[0] + n[0] * 1.3 + u[0] * 0.8, c[1] + n[1] * 1.3 + u[1] * 0.8]], closed: false, fill: false, dash: false, w: 0.25 });
                    }
                }
                Kind::Capillary => {
                    for s in [1.0, -1.0] {
                        out.push(Prim::Poly { pts: vec![[m[0] - n[0] * 1.1 - u[0] * s * 1.1, m[1] - n[1] * 1.1 - u[1] * s * 1.1], [m[0] + n[0] * 1.1 + u[0] * s * 1.1, m[1] + n[1] * 1.1 + u[1] * s * 1.1]], closed: false, fill: false, dash: false, w: 0.25 });
                    }
                }
                Kind::Data => out.push(Prim::Circle { c: m, r: 0.7, fill: false, w: 0.25 }),
                _ => {}
            }
        }
    }
    out
}

/// Where a line's number goes: above the middle of its longest horizontal segment, else beside its
/// longest vertical one. Returns the point and whether it is beside a vertical segment.
fn label_at(path: &[[f64; 2]]) -> Option<([f64; 2], bool)> {
    let mut best: Option<([f64; 2], bool, f64)> = None;
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let horiz = (a[1] - b[1]).abs() < 1e-9;
        let len = (b[0] - a[0]).abs() + (b[1] - a[1]).abs();
        let score = len * if horiz { 1.5 } else { 1.0 };
        if len >= 8.0 && best.is_none_or(|x| score > x.2) {
            best = Some(([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], !horiz, score));
        }
    }
    best.map(|x| (x.0, x.1))
}

fn text(p: [f64; 2], h: f64, s: impl Into<String>, anchor: u8, bold: bool) -> Prim {
    Prim::Text { p, h, s: s.into(), anchor, bold }
}

/// The view of one sheet. `hyd` holds each run's hydraulics (from `hydraulics::all`).
pub fn sheet(p: &Project, s: &Sheet, net: &Net, hyd: &[Option<Hyd>]) -> SheetView {
    let (w, h) = s.dims();
    let routes = routes(p, s.id, None);
    // Ports in use, for the open-port markers.
    let mut used: BTreeSet<(u32, u8)> = BTreeSet::new();
    for l in p.lines.iter().filter(|l| l.sheet == s.id) {
        for e in [&l.a, &l.b] {
            if let Some(x) = e.item() {
                used.insert(x);
            }
        }
    }
    let items = p
        .items
        .iter()
        .filter(|i| i.sheet == s.id)
        .map(|it| {
            let sym = symbols::get(&it.sym);
            let pl = Place::of(it);
            let bb = bbox(it);
            let mut texts: Vec<Prim> = sym.map(|s| pl.prims(&symbols::parse(s.draw).into_iter().filter(|x| matches!(x, Prim::Text { .. })).collect::<Vec<_>>())).unwrap_or_default();
            let cat = sym.map_or(Cat::Mark, |s| s.cat);
            let sc = it.scale;
            match cat {
                Cat::Instrument => {
                    let (l, n) = crate::tags::loop_of(p.tag_of(it));
                    texts.push(text([it.x, it.y - 1.5 * sc], 2.5 * sc, l, 1, true));
                    texts.push(text([it.x, it.y + 2.0 * sc], 2.2 * sc, n, 1, false));
                }
                Cat::Connector => {
                    let partner = p.item(it.link).and_then(|q| p.sheet(q.sheet)).map(|sh| sh.number.clone()).unwrap_or_else(|| "?".into());
                    texts.push(text([it.x - 1.0 * sc, it.y + 0.9 * sc], 2.0 * sc, partner, 1, false));
                    let line = net.on_port(it.id, 0).first().and_then(|x| net.numbers.get(&x.0)).cloned().unwrap_or_default();
                    let note = p.data_of(it.obj).get("to_text").map(|v| v.text()).unwrap_or_default();
                    let below = [line, note].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
                    if !below.is_empty() {
                        texts.push(text([it.x, bb[3] + 3.0], 2.0, below, 1, false));
                    }
                }
                _ => {
                    let tag = p.tag_of(it);
                    if !tag.is_empty() {
                        let big = cat.equipment();
                        let y = if big { bb[3] + 4.0 } else { bb[1] - 1.2 };
                        texts.push(text([it.x, y], if big { 3.0 } else { 2.2 }, tag.to_string(), 1, big));
                        if big {
                            if let Some(svc) = p.data_of(it.obj).get("service").map(|v| v.text()).filter(|t| !t.is_empty()) {
                                texts.push(text([it.x, y + 3.2], 2.2, crate::clip(&svc, 40), 1, false));
                            }
                        }
                    }
                    if cat == Cat::Control {
                        if let Some(f) = p.data_of(it.obj).get("fail").map(|v| v.text()).filter(|t| !t.is_empty()) {
                            texts.push(text([it.x, bb[3] + 2.6], 2.0, f, 1, false));
                        }
                    }
                }
            }
            // The data the project chose to show for this kind of symbol, under its other texts.
            if let Some(o) = p.obj(it.obj).filter(|_| !p.settings.labels.is_empty()) {
                let chain: Vec<String> = crate::classes::chain(&o.class).iter().map(|c| c.name.clone()).collect();
                let keys: Vec<&String> = p.settings.labels.iter().filter(|r| r.class == o.class || chain.contains(&r.class)).flat_map(|r| &r.keys).collect();
                if !keys.is_empty() {
                    let attrs = crate::classes::attrs(Some(p), &o.class);
                    let us = p.units == "US";
                    let mut y = texts.iter().filter_map(|t| if let Prim::Text { p: q, .. } = t { Some(q[1]) } else { None }).fold(bb[3], f64::max) + 3.0;
                    let mut done = std::collections::HashSet::new();
                    for k in keys {
                        if !done.insert(k.as_str()) {
                            continue;
                        }
                        let Some(v) = o.data.get(k.as_str()) else { continue };
                        let q = attrs.iter().find(|a| &a.key == k).map(|a| a.q.clone()).unwrap_or_default();
                        let s = match v.num() {
                            Some(x) if !q.is_empty() => crate::units::show(x, &q, us),
                            _ => v.text(),
                        };
                        if !s.trim().is_empty() {
                            texts.push(text([it.x, y], 2.0, crate::clip(&s, 30), 1, false));
                            y += 2.8;
                        }
                    }
                }
            }
            let ports = sym
                .map(|s| (0..s.ports.len()).filter_map(|k| graph::port_pos(it, k as u8).map(|(q, d)| PortView { p: q, dir: d, signal: s.ports[k].signal, used: used.contains(&(it.id, k as u8)) })).collect())
                .unwrap_or_default();
            ItemView { id: it.id, sym: it.sym.clone(), cat, x: it.x, y: it.y, rot: it.rot, flip: it.flip, scale: it.scale, tag: p.tag_of(it).to_string(), obj: it.obj, texts, bbox: bb, ports, layer: cat.layer() }
        })
        .collect();
    let mut sorted: Vec<&crate::model::Line> = p.lines.iter().filter(|l| l.sheet == s.id).collect();
    sorted.sort_by_key(|l| l.id);
    let paths: Vec<(u32, Kind, Vec<[f64; 2]>)> = sorted.iter().map(|l| (l.id, l.kind, routes.get(&l.id).cloned().unwrap_or_default())).collect();
    let segs = hops(&paths);
    // One label per line number on a sheet: on the piece with the longest straight segment.
    let mut best: HashMap<usize, (u32, f64)> = HashMap::new();
    for (l, (_, _, path)) in sorted.iter().zip(&paths) {
        if let Some(r) = net.run_of.get(&l.id) {
            let len = path.windows(2).map(|w| (w[1][0] - w[0][0]).abs() + (w[1][1] - w[0][1]).abs() * 0.67).fold(0.0, f64::max);
            if best.get(r).is_none_or(|b| len > b.1) {
                best.insert(*r, (l.id, len));
            }
        }
    }
    let lines = sorted
        .iter()
        .zip(&paths)
        .map(|(l, (_, _, path))| {
            let mut marks = line_marks(l.kind, path, !l.flip);
            let number = net.numbers.get(&l.id).cloned().unwrap_or_default();
            let run = net.run_of.get(&l.id).copied();
            let badge = run.and_then(|r| hyd.get(r)).and_then(|x| x.as_ref()).map(|x| x.status);
            let labelled = run.is_some_and(|r| best.get(&r).is_some_and(|b| b.0 == l.id));
            let stream = p.obj(l.obj).is_some_and(|o| o.class == crate::model::STREAM_CLASS);
            if let Some((at, vert)) = label_at(path).filter(|_| labelled && stream && !number.is_empty()) {
                // A stream's number in a diamond beside the line.
                let c = if vert { [at[0] + 3.6, at[1]] } else { [at[0], at[1] - 3.6] };
                let r = 2.6;
                marks.push(Prim::Poly { pts: vec![[c[0], c[1] - r], [c[0] + r, c[1]], [c[0], c[1] + r], [c[0] - r, c[1]]], closed: true, fill: false, dash: false, w: 0.25 });
                marks.push(text(c, if number.chars().count() > 2 { 1.6 } else { 2.0 }, number.clone(), 1, true));
            } else if let Some((at, vert)) = label_at(path).filter(|_| labelled) {
                if !number.is_empty() {
                    let (q, anchor) = if vert { ([at[0] + 1.5, at[1]], 0) } else { ([at[0], at[1] - 1.4], 1) };
                    marks.push(text(q, 2.2, number.clone(), anchor, false));
                }
                if let Some(b) = badge {
                    let c = if vert { [at[0] - 2.8, at[1]] } else { [at[0], at[1] + 3.0] };
                    marks.push(text(c, 2.4, ["✓", "!", "✗"][b.min(2) as usize], 1, true));
                }
            }
            LineView { id: l.id, obj: l.obj, kind: l.kind, path: path.clone(), segs: segs.get(&l.id).cloned().unwrap_or_default(), marks, number, badge, run, layer: line_layer(l.kind) }
        })
        .collect();
    let notes = p.notes.iter().filter(|n| n.sheet == s.id).map(note_view).collect();
    SheetView { id: s.id, number: s.number.clone(), title: s.title.clone(), w, h, legend: s.legend, frame: { let mut f = frame::frame(p, s); f.extend(crate::pfd::table(p, s)); f }, items, lines, notes, legend_rows: if s.legend { legend(p, s) } else { vec![] }, underlay: s.underlay.clone().filter(|u| u.show), clouds: crate::revs::clouds(p, s), logo: frame::logo(p, s), pins: crate::review::pins(p, s.id) }
}

/// Text lines of a note, and their box.
fn note_view(n: &crate::model::Note) -> NoteView {
    let lines: Vec<&str> = n.text.lines().take(40).collect();
    let hh = 2.5;
    let mut prims: Vec<Prim> = lines.iter().enumerate().map(|(i, l)| text([n.x, n.y + hh + i as f64 * hh * 1.4], hh, crate::clip(l, 200), 0, false)).collect();
    let tw = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f64 * hh * 0.6;
    let th = lines.len().max(1) as f64 * hh * 1.4;
    let mut bb = [n.x, n.y, n.x + tw.max(4.0), n.y + th];
    match n.kind {
        NoteKind::Leader => {
            let from = [if n.to[0] < n.x { n.x - 1.0 } else { n.x + tw + 1.0 }, n.y + hh * 0.7];
            let len = (n.to[0] - from[0]).hypot(n.to[1] - from[1]).max(1e-9);
            let u = [(n.to[0] - from[0]) / len, (n.to[1] - from[1]) / len];
            prims.push(Prim::Poly { pts: vec![from, n.to], closed: false, fill: false, dash: false, w: 0.25 });
            prims.push(tri(n.to, u, 2.5, 0.8));
            bb = [bb[0].min(n.to[0]), bb[1].min(n.to[1]), bb[2].max(n.to[0]), bb[3].max(n.to[1])];
        }
        NoteKind::Hold => {
            // A cloud: small arcs along the edges of the area, and the word HOLD.
            let (w, h) = (n.w.max(20.0), n.h.max(12.0));
            let r = 2.5;
            let mut edge = |a: [f64; 2], b: [f64; 2], start: f64| {
                let len = (b[0] - a[0]).hypot(b[1] - a[1]);
                let k = (len / (2.0 * r)).round().max(1.0) as usize;
                let step = [(b[0] - a[0]) / k as f64, (b[1] - a[1]) / k as f64];
                let rr = (step[0].hypot(step[1])) / 2.0;
                for i in 0..k {
                    let c = [a[0] + step[0] * (i as f64 + 0.5), a[1] + step[1] * (i as f64 + 0.5)];
                    prims.push(Prim::Arc { c, r: rr, a0: start, a1: start + 180.0, w: 0.35 });
                }
            };
            edge([n.x, n.y], [n.x + w, n.y], 180.0);
            edge([n.x + w, n.y], [n.x + w, n.y + h], 270.0);
            edge([n.x + w, n.y + h], [n.x, n.y + h], 0.0);
            edge([n.x, n.y + h], [n.x, n.y], 90.0);
            prims.push(text([n.x + w - 1.5, n.y + 4.5], 3.0, "HOLD", 2, true));
            bb = [n.x - r, n.y - r, n.x + w + r, n.y + h + r];
        }
        NoteKind::Note => {}
    }
    NoteView { id: n.id, kind: n.kind, prims, bbox: bb }
}

/// The legend: every symbol the project uses (by category) and every line type, in columns.
pub fn legend(p: &Project, s: &Sheet) -> Vec<LegendRow> {
    let used: BTreeSet<&str> = p.items.iter().filter(|i| p.sheet(i.sheet).is_some_and(|x| !x.legend)).map(|i| i.sym.as_str()).collect();
    let kinds: BTreeSet<Kind> = p.lines.iter().map(|l| l.kind).collect();
    let mut by_cat: BTreeMap<Cat, Vec<&symbols::Sym>> = BTreeMap::new();
    for sym in symbols::all().iter().filter(|x| used.contains(x.key)) {
        by_cat.entry(sym.cat).or_default().push(sym);
    }
    let (x0, y0, _, _) = frame::inner(s);
    let (rx, ry) = frame::reserved(p, s);
    let col_w = 95.0;
    let (mut x, mut y) = (x0 + 8.0, y0 + 14.0);
    let mut out = vec![LegendRow { sym: None, kind: None, x, y: y0 + 8.0, name: "LEGEND".into() }];
    let mut put = |out: &mut Vec<LegendRow>, sym: Option<String>, kind: Option<Kind>, name: String, hgt: f64| {
        let bottom = if x + col_w > rx { ry } else { s.dims().1 - 30.0 };
        if y + hgt > bottom {
            x += col_w;
            y = y0 + 14.0;
        }
        out.push(LegendRow { sym, kind, x, y: y + hgt / 2.0, name });
        y += hgt + 3.0;
    };
    for (cat, syms) in &by_cat {
        put(&mut out, None, None, cat.name().to_uppercase(), 5.0);
        for sym in syms {
            put(&mut out, Some(sym.key.to_string()), None, sym.name.to_string(), sym.h.clamp(6.0, 40.0));
        }
    }
    if !kinds.is_empty() {
        put(&mut out, None, None, "LINES".into(), 5.0);
        for k in kinds {
            put(&mut out, None, Some(k), k.name().to_string(), 5.0);
        }
    }
    out
}

/// Every sheet's view.
pub fn all(p: &Project) -> Vec<SheetView> {
    let net = Net::new(p);
    let hyd = hydraulics::all(p, &net);
    p.sheets.iter().map(|s| sheet(p, s, &net, &hyd)).collect()
}

/// The legend rows' drawings, for the DXF and SVG: the symbol at the row (scaled to fit 30 mm
/// wide) or a sample of the line type.
pub fn legend_prims(r: &LegendRow) -> Vec<Prim> {
    let mut v = vec![];
    if let Some(sym) = r.sym.as_deref().and_then(symbols::get) {
        let sc = (30.0 / sym.w).min(1.0);
        let pl = Place { x: r.x + 15.0, y: r.y, rot: 0, flip: false, scale: sc };
        v.extend(pl.prims(&symbols::parse(sym.draw)));
    } else if let Some(k) = r.kind {
        let path = [[r.x, r.y], [r.x + 30.0, r.y]];
        v.push(Prim::Poly { pts: path.to_vec(), closed: false, fill: false, dash: k == Kind::Electric, w: line_width(k) });
        v.extend(line_marks(k, &path, true));
    }
    let header = r.sym.is_none() && r.kind.is_none();
    v.push(text([if header { r.x } else { r.x + 36.0 }, r.y + 0.9], if header { 3.0 } else { 2.5 }, r.name.clone(), 0, header));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::{Item, Line};

    #[test]
    fn crossing_lines_get_a_gap_and_arrows_follow_the_flow() {
        let mut d = Doc::new(Project::new());
        let sh = d.p.sheets[0].id;
        let add = |d: &mut Doc, a: End, b: End, kind: Kind| d.apply(Op::AddLine { line: Line { sheet: sh, a, b, kind, ..Line::default() } }).unwrap()[0];
        let h = add(&mut d, End::Free { x: 50.0, y: 100.0 }, End::Free { x: 150.0, y: 100.0 }, Kind::Major);
        let v = add(&mut d, End::Free { x: 100.0, y: 50.0 }, End::Free { x: 100.0, y: 150.0 }, Kind::Major);
        let net = Net::new(&d.p);
        let sv = sheet(&d.p, &d.p.sheets[0], &net, &[]);
        let lh = sv.lines.iter().find(|l| l.id == h).unwrap();
        let lv = sv.lines.iter().find(|l| l.id == v).unwrap();
        assert_eq!(lh.segs.len(), 1, "the horizontal line is continuous");
        assert_eq!(lv.segs.len(), 2, "the vertical line hops");
        // One arrow on the horizontal line, pointing +x (tip to the right of the base).
        let Prim::Poly { pts, .. } = &lh.marks[0] else { panic!() };
        assert!(pts[0][0] > pts[1][0]);
        // A signal line crossing a process line gets the gap, whatever its direction.
        let s = add(&mut d, End::Free { x: 60.0, y: 60.0 }, End::Free { x: 60.0, y: 140.0 }, Kind::Electric);
        let net = Net::new(&d.p);
        let sv = sheet(&d.p, &d.p.sheets[0], &net, &[]);
        assert_eq!(sv.lines.iter().find(|l| l.id == s).unwrap().segs.len(), 2);
    }

    #[test]
    fn items_show_tags_and_ports() {
        let mut d = Doc::new(Project::new());
        let sh = d.p.sheets[0].id;
        let t = d.apply(Op::AddItem { item: Item { sheet: sh, sym: "inst_field".into(), x: 100.0, y: 100.0, ..Item::default() }, on: None, tag: "LIC".into(), obj: None }).unwrap()[0];
        let net = Net::new(&d.p);
        let sv = sheet(&d.p, &d.p.sheets[0], &net, &[]);
        let it = sv.items.iter().find(|i| i.id == t).unwrap();
        let words: Vec<String> = it.texts.iter().filter_map(|x| if let Prim::Text { s, .. } = x { Some(s.clone()) } else { None }).collect();
        assert_eq!(words, vec!["LIC".to_string(), "101".to_string()]);
        assert_eq!(it.ports.len(), 4);
        assert!(!sv.frame.is_empty());
    }

    #[test]
    fn legend_lists_used_symbols() {
        let mut d = Doc::new(Project::new());
        let sh = d.p.sheets[0].id;
        d.apply(Op::AddItem { item: Item { sheet: sh, sym: "pump_cent".into(), x: 100.0, y: 100.0, ..Item::default() }, on: None, tag: String::new(), obj: None }).unwrap();
        let lg = d.apply(Op::AddSheet { sheet: Sheet { legend: true, ..Sheet::default() } }).unwrap()[0];
        let rows = legend(&d.p, d.p.sheet(lg).unwrap());
        assert!(rows.iter().any(|r| r.name == "Centrifugal pump"));
        assert!(!rows.iter().any(|r| r.name == "Gate valve"));
    }
}
