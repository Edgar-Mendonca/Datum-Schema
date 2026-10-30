//! The sheet frame: border, grid zones (A–H across, 1–8 down), the title block and the revision
//! table, as primitives in sheet millimetres (a text's point is the middle of its height). The window, the printout and the DXF all draw these
//! same primitives.

use crate::model::{Project, Sheet, TbCell};
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

/// The standard title block (180 × 44 mm), as cells.
pub fn standard() -> crate::model::TitleBlock {
    let c = |x: f64, y: f64, w: f64, h: f64, label: &str, field: &str, size: f64| TbCell { x, y, w, h, label: label.into(), field: field.into(), size, bold: size > 3.0 };
    crate::model::TitleBlock {
        on: false,
        name: "Standard".into(),
        w: TB_W,
        h: TB_H,
        cells: vec![
            c(0.0, 0.0, 90.0, 12.0, "COMPANY", "company", 3.5),
            c(90.0, 0.0, 90.0, 12.0, "PROJECT", "project", 3.0),
            c(0.0, 12.0, 180.0, 12.0, "TITLE", "title", 3.5),
            c(0.0, 24.0, 60.0, 10.0, "DRAWING NO.", "number", 3.0),
            c(60.0, 24.0, 20.0, 10.0, "REV", "rev", 3.0),
            c(80.0, 24.0, 25.0, 10.0, "SCALE", "scale", 2.5),
            c(105.0, 24.0, 30.0, 10.0, "SHEET", "sheet", 2.5),
            c(135.0, 24.0, 45.0, 10.0, "UNIT / AREA", "unit", 2.5),
            c(0.0, 34.0, 45.0, 10.0, "DRAWN", "by", 2.5),
            c(45.0, 34.0, 45.0, 10.0, "CHECKED", "checked", 2.5),
            c(90.0, 34.0, 45.0, 10.0, "APPROVED", "approved", 2.5),
            c(135.0, 34.0, 45.0, 10.0, "DATE", "date", 2.5),
        ],
    }
}

/// The fields a cell can show: (key, what it is).
pub const FIELDS: [(&str, &str); 14] = [
    ("company", "Company"), ("project", "Project"), ("client", "Client"), ("unit", "Unit or area"), ("title", "Sheet title"), ("number", "Drawing number"), ("rev", "Revision"),
    ("scale", "Scale"), ("sheet", "Sheet n of m"), ("date", "Date"), ("by", "Drawn by"), ("checked", "Checked by"), ("approved", "Approved by"), ("logo", "Company logo"),
];

/// The title block a project uses.
pub fn block(p: &Project) -> std::borrow::Cow<'_, crate::model::TitleBlock> {
    let t = &p.settings.title_block;
    if t.on && !t.cells.is_empty() { std::borrow::Cow::Borrowed(t) } else { std::borrow::Cow::Owned(standard()) }
}

/// What a cell shows on a sheet.
pub fn value(p: &Project, s: &Sheet, field: &str) -> String {
    let t = &p.title;
    if let Some(x) = field.strip_prefix("text:") {
        return x.to_string();
    }
    if let Some(x) = field.strip_prefix("f:") {
        return t.extra.get(x).cloned().unwrap_or_default();
    }
    match field {
        "company" => t.company.clone(),
        "project" => if t.project.is_empty() { p.name.clone() } else { t.project.clone() },
        "client" => t.client.clone(),
        "unit" => t.unit.clone(),
        "title" => s.title.clone(),
        "number" => s.number.clone(),
        "rev" => s.rev.clone(),
        "scale" => s.scale.clone(),
        "sheet" => format!("{} of {}", p.sheet_no(s.id), p.sheets.len()),
        "date" => s.date.clone(),
        "by" => t.by.clone(),
        "checked" => t.checked.clone(),
        "approved" => t.approved.clone(),
        _ => String::new(),
    }
}

/// A title block's drawing with its top left corner at (tx, ty), and where its logo goes
/// (x, y, w, h), if it has one.
pub fn draw_block(p: &Project, s: &Sheet, b: &crate::model::TitleBlock, tx: f64, ty: f64) -> (Vec<Prim>, Option<[f64; 4]>) {
    let mut v = vec![rect(tx, ty, b.w, b.h, 0.5)];
    let mut logo = None;
    for c in &b.cells {
        let (x, y) = (tx + c.x, ty + c.y);
        v.push(rect(x, y, c.w, c.h, 0.25));
        let top = if c.label.is_empty() { 0.0 } else { 2.6 };
        if !c.label.is_empty() {
            v.push(text(x + 1.2, y + 1.9, 1.6, &c.label, 0, false));
        }
        if c.field == "logo" {
            logo = Some([x + 1.0, y + top + 0.6, (c.w - 2.0).max(1.0), (c.h - top - 1.6).max(1.0)]);
            continue;
        }
        let val = value(p, s, &c.field);
        if val.is_empty() {
            continue;
        }
        let big = c.size.min((c.h - top - 1.0).max(1.0));
        // Near the bottom of the cell, under its caption; long text clipped to the width.
        v.push(text(x + 1.2, (y + c.h - big * 0.5 - 1.2).max(y + top + big * 0.5), big, &crate::clip(&val, ((c.w - 2.0) / (big * 0.62)).max(1.0) as usize), 0, c.bold));
    }
    (v, logo)
}

/// The title block in the bottom right corner, with the revision table above it.
fn title_block(p: &Project, s: &Sheet) -> Vec<Prim> {
    let (_, _, x1, y1) = inner(s);
    let b = block(p);
    let (tx, ty) = (x1 - b.w, y1 - b.h);
    let (mut v, _) = draw_block(p, s, &b, tx, ty);
    // Revision table: header on the title block, revisions stacked above it (first lowest), its
    // columns spread over the block's width.
    let k = b.w / TB_W;
    let cols: [(f64, &str); 6] = [(15.0 * k, "REV"), (25.0 * k, "DATE"), (80.0 * k, "DESCRIPTION"), (20.0 * k, "BY"), (20.0 * k, "CHK"), (20.0 * k, "APP")];
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

/// A title block on its own, for the editor's preview: its drawing at (0, 0) and its logo's place.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Preview {
    pub w: f64,
    pub h: f64,
    pub prims: Vec<Prim>,
    pub logo: Option<[f64; 4]>,
}

pub fn preview(p: &Project, s: &Sheet, b: &crate::model::TitleBlock) -> Preview {
    let mut b = b.clone();
    b.on = true;
    let mut q = p.clone();
    q.settings.title_block = b;
    q.normalize();
    let b = block(&q).into_owned();
    let (prims, logo) = draw_block(&q, s, &b, 0.0, 0.0);
    Preview { w: b.w, h: b.h, prims, logo }
}

/// Where the logo goes on a sheet (x, y, w, h), if the title block has one.
pub fn logo(p: &Project, s: &Sheet) -> Option<[f64; 4]> {
    let (_, _, x1, y1) = inner(s);
    let b = block(p);
    draw_block(p, s, &b, x1 - b.w, y1 - b.h).1
}

/// The top left of the title block and its revision table: drawing should stay above and left.
pub fn reserved(p: &Project, s: &Sheet) -> (f64, f64) {
    let (_, _, x1, y1) = inner(s);
    let b = block(p);
    (x1 - b.w, y1 - b.h - ROW * (s.revs.len().min(12) as f64 + 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_company_title_block_is_drawn_from_its_cells() {
        use crate::model::{TbCell, TitleBlock};
        let mut p = crate::examples::get("pump").unwrap();
        // The standard block, as cells, draws as before.
        assert_eq!(block(&p).w, TB_W);
        assert!(logo(&p, &p.sheets[0]).is_none());
        let c = |x: f64, y: f64, w: f64, h: f64, label: &str, field: &str| TbCell { x, y, w, h, label: label.into(), field: field.into(), size: 2.5, bold: false };
        p.settings.title_block = TitleBlock { on: true, name: "Acme".into(), w: 150.0, h: 30.0, cells: vec![
            c(0.0, 0.0, 40.0, 30.0, "", "logo"),
            c(40.0, 0.0, 110.0, 15.0, "DRAWING", "number"),
            c(40.0, 15.0, 55.0, 15.0, "CONTRACT", "f:Contract"),
            c(95.0, 15.0, 55.0, 15.0, "", "text:CONFIDENTIAL"),
            // Out of the block and absurd: brought back inside.
            TbCell { x: 500.0, y: -4.0, w: 900.0, h: f64::NAN, size: 99.0, ..c(0.0, 0.0, 1.0, 1.0, "X", "rev") },
        ] };
        p.title.extra.insert("Contract".into(), "C-2026-17".into());
        assert!(p.normalize().is_empty());
        let s = p.sheets[0].clone();
        let tb = &p.settings.title_block;
        assert!(tb.cells.iter().all(|c| c.x + c.w <= tb.w + 1e-9 && c.y + c.h <= tb.h + 1e-9 && c.size <= 10.0));
        let texts: Vec<String> = frame(&p, &s).into_iter().filter_map(|x| if let Prim::Text { s, .. } = x { Some(s) } else { None }).collect();
        assert!(texts.contains(&"C-2026-17".to_string()) && texts.contains(&"CONFIDENTIAL".to_string()) && texts.contains(&s.number));
        assert!(!texts.contains(&"COMPANY".to_string()));
        // The logo's place is in the bottom right corner of the drawing area, in its cell.
        let (_, _, x1, y1) = inner(&s);
        let l = logo(&p, &s).unwrap();
        assert!((l[0] - (x1 - 150.0 + 1.0)).abs() < 1e-9 && l[1] > y1 - 30.0 && l[2] > 30.0);
        // Drawing keeps clear of the narrower block; the preview draws it at the origin.
        assert_eq!(reserved(&p, &s).0, x1 - 150.0);
        let pv = preview(&p, &s, &p.settings.title_block);
        assert_eq!((pv.w, pv.h), (150.0, 30.0));
        assert!(pv.logo.is_some_and(|l| l[0] < 2.0));
        // Saved and opened again.
        let (q, w) = crate::model::from_json(&crate::model::to_json(&p)).unwrap();
        assert!(w.is_empty() && q.settings.title_block == p.settings.title_block && q.title.extra == p.title.extra);
        // A template with no cells falls back to the standard block.
        p.settings.title_block.cells.clear();
        p.normalize();
        assert!(!p.settings.title_block.on && block(&p).w == TB_W);
    }

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
