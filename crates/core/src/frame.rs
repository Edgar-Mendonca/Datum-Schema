//! The sheet frame: border, grid zones (A–H across, 1–8 down), the title block and the revision
//! table, as primitives in sheet millimetres (a text's point is the middle of its height). The window, the printout and the DXF all draw these
//! same primitives.

use crate::model::{Project, Sheet};
use crate::symbols::Prim;

/// Title block size (mm).
pub const TB_W: f64 = 180.0;
pub const TB_H: f64 = 44.0;
const ROW: f64 = 5.0;

fn line(a: [f64; 2], b: [f64; 2], w: f64) -> Prim {
    Prim::Poly { pts: vec![a, b], closed: false, fill: false, dash: false, w }
}
fn rect(x: f64, y: f64, w: f64, h: f64, lw: f64) -> Prim {
    Prim::Poly { pts: vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]], closed: true, fill: false, dash: false, w: lw }
}
fn text(x: f64, y: f64, h: f64, s: &str, anchor: u8, bold: bool) -> Prim {
    Prim::Text { p: [x, y], h, s: s.to_string(), anchor, bold }
}

/// The border margin and the zone band's width for a sheet.
pub fn margins(s: &Sheet) -> (f64, f64) {
    let (w, _) = s.dims();
    if w > 600.0 { (15.0, 6.0) } else { (10.0, 5.0) }
}

/// The drawing area inside the zone band: (x0, y0, x1, y1).
pub fn inner(s: &Sheet) -> (f64, f64, f64, f64) {
    let (w, h) = s.dims();
    let (m, z) = margins(s);
    (m + z, m + z, w - m - z, h - m - z)
}

/// The zone of a point, such as "C4".
pub fn zone(s: &Sheet, x: f64, y: f64) -> String {
    let (x0, y0, x1, y1) = inner(s);
    let c = (((x - x0) / (x1 - x0) * 8.0).floor().clamp(0.0, 7.0)) as u8;
    let r = (((y - y0) / (y1 - y0) * 8.0).floor().clamp(0.0, 7.0)) as u8;
    format!("{}{}", (b'A' + c) as char, r + 1)
}

/// The frame of one sheet.
pub fn frame(p: &Project, s: &Sheet) -> Vec<Prim> {
    let (w, h) = s.dims();
    let (m, z) = margins(s);
    let (x0, y0, x1, y1) = inner(s);
    let mut v = vec![rect(m, m, w - 2.0 * m, h - 2.0 * m, 0.5), rect(x0, y0, x1 - x0, y1 - y0, 0.35)];
    // Zones: letters across, numbers down, in the band on all four sides.
    for i in 0..8 {
        let (xa, xb) = (x0 + (x1 - x0) * i as f64 / 8.0, x0 + (x1 - x0) * (i + 1) as f64 / 8.0);
        let (ya, yb) = (y0 + (y1 - y0) * i as f64 / 8.0, y0 + (y1 - y0) * (i + 1) as f64 / 8.0);
        let l = ((b'A' + i as u8) as char).to_string();
        let n = (i + 1).to_string();
        v.push(text((xa + xb) / 2.0, m + z / 2.0, 2.5, &l, 1, false));
        v.push(text((xa + xb) / 2.0, h - m - z / 2.0, 2.5, &l, 1, false));
        v.push(text(m + z / 2.0, (ya + yb) / 2.0, 2.5, &n, 1, false));
        v.push(text(w - m - z / 2.0, (ya + yb) / 2.0, 2.5, &n, 1, false));
        if i > 0 {
            v.push(line([xa, m], [xa, y0], 0.25));
            v.push(line([xa, y1], [xa, h - m], 0.25));
            v.push(line([m, ya], [x0, ya], 0.25));
            v.push(line([x1, ya], [w - m, ya], 0.25));
        }
    }
    v.extend(title_block(p, s));
    v
}

/// The title block in the bottom right corner, with the revision table above it.
fn title_block(p: &Project, s: &Sheet) -> Vec<Prim> {
    let (_, _, x1, y1) = inner(s);
    let (tx, ty) = (x1 - TB_W, y1 - TB_H);
    let t = &p.title;
    let mut v = vec![rect(tx, ty, TB_W, TB_H, 0.5)];
    let n = p.sheets.len();
    let no = p.sheet_no(s.id);
    let cell = |v: &mut Vec<Prim>, x: f64, y: f64, w: f64, hh: f64, label: &str, val: &str, big: f64| {
        v.push(rect(x, y, w, hh, 0.25));
        v.push(text(x + 1.2, y + 1.9, 1.6, label, 0, false));
        v.push(text(x + 1.2, y + hh - big * 0.5 - 1.2, big, &crate::clip(val, ((w - 2.0) / (big * 0.62)).max(1.0) as usize), 0, big > 3.0));
    };
    cell(&mut v, tx, ty, 90.0, 12.0, "COMPANY", &t.company, 3.5);
    cell(&mut v, tx + 90.0, ty, 90.0, 12.0, "PROJECT", if t.project.is_empty() { &p.name } else { &t.project }, 3.0);
    cell(&mut v, tx, ty + 12.0, 180.0, 12.0, "TITLE", &s.title, 3.5);
    let r3 = ty + 24.0;
    cell(&mut v, tx, r3, 60.0, 10.0, "DRAWING NO.", &s.number, 3.0);
    cell(&mut v, tx + 60.0, r3, 20.0, 10.0, "REV", &s.rev, 3.0);
    cell(&mut v, tx + 80.0, r3, 25.0, 10.0, "SCALE", &s.scale, 2.5);
    cell(&mut v, tx + 105.0, r3, 30.0, 10.0, "SHEET", &format!("{no} of {n}"), 2.5);
    cell(&mut v, tx + 135.0, r3, 45.0, 10.0, "UNIT / AREA", &t.unit, 2.5);
    let r4 = ty + 34.0;
    cell(&mut v, tx, r4, 45.0, 10.0, "DRAWN", &t.by, 2.5);
    cell(&mut v, tx + 45.0, r4, 45.0, 10.0, "CHECKED", &t.checked, 2.5);
    cell(&mut v, tx + 90.0, r4, 45.0, 10.0, "APPROVED", &t.approved, 2.5);
    cell(&mut v, tx + 135.0, r4, 45.0, 10.0, "DATE", &s.date, 2.5);
    // Revision table: header on the title block, revisions stacked above it (first lowest).
    let cols: [(f64, &str); 6] = [(15.0, "REV"), (25.0, "DATE"), (80.0, "DESCRIPTION"), (20.0, "BY"), (20.0, "CHK"), (20.0, "APP")];
    let row = |v: &mut Vec<Prim>, y: f64, vals: [&str; 6], bold: bool| {
        let mut x = tx;
        for ((cw, _), val) in cols.iter().zip(vals) {
            v.push(rect(x, y, *cw, ROW, 0.25));
            v.push(text(x + 1.0, y + ROW / 2.0, if bold { 1.8 } else { 2.2 }, &crate::clip(val, (cw / 1.4) as usize), 0, bold));
            x += cw;
        }
    };
    row(&mut v, ty - ROW, cols.map(|c| c.1), true);
    for (i, r) in s.revs.iter().take(12).enumerate() {
        row(&mut v, ty - ROW * (i as f64 + 2.0), [&r.rev, &r.date, &r.desc, &r.by, &r.checked, &r.approved], false);
    }
    v
}

/// The top of the revision table: drawing should stay above and left of it.
pub fn reserved(s: &Sheet) -> (f64, f64) {
    let (_, _, x1, y1) = inner(s);
    (x1 - TB_W, y1 - TB_H - ROW * (s.revs.len().min(12) as f64 + 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_has_zones_and_title_block() {
        let mut p = Project::new();
        p.title.company = "ACME <script>".into();
        p.sheets[0].revs.push(crate::model::Rev { rev: "A".into(), desc: "Issued for review".into(), ..Default::default() });
        let f = frame(&p, &p.sheets[0]);
        let texts: Vec<&str> = f.iter().filter_map(|x| if let Prim::Text { s, .. } = x { Some(s.as_str()) } else { None }).collect();
        for z in ["A", "H", "1", "8", "PID-001", "Issued for review", "1 of 1", "NTS"] {
            assert!(texts.contains(&z), "missing {z}: {texts:?}");
        }
        // Hostile text stays text: the primitives hold it as written; SVG escapes it.
        assert!(texts.iter().any(|t| t.contains("<script>")));
        assert!(!crate::symbols::svg(&f).contains("<script>"));
        assert_eq!(zone(&p.sheets[0], 16.0, 16.0), "A1");
        assert_eq!(zone(&p.sheets[0], 400.0, 280.0), "H8");
    }
}
