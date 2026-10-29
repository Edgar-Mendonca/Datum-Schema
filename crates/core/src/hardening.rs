//! FAMILY §11 hardening: a failed edit changes nothing; random projects and random sequences of
//! edits never panic and keep the project sound; damaged files open or give a clear error; hostile
//! values are clamped; a speed gate on a large drawing.

use crate::api;
use crate::doc::{Doc, Op};
use crate::model::{self, End, Item, Kind, Line, Note, NoteKind, Project, Sheet, Val};
use crate::{dxf, lists, symbols, view};

/// A small built-in random generator (no extra crate).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn f(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next() % 1_000_000) as f64 / 1_000_000.0 * (hi - lo)
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> Option<&'a T> {
        (!v.is_empty()).then(|| &v[self.below(v.len())])
    }
}

const HOSTILE: [&str; 8] = ["<script>alert(1)</script>", "=HYPERLINK(\"http://x\")", "\u{0}\u{1}\n\r\t", "😀 P-101 «€»", "", "                ", "-1e308", "P-101A"];

/// A random project: sheets, symbols, lines between random ends, notes and data.
fn random_project(r: &mut Rng, n: usize) -> Project {
    let mut d = Doc::new(Project::new());
    let syms = symbols::all();
    for _ in 0..r.below(3) {
        let _ = d.apply(Op::AddSheet { sheet: Sheet { size: ["A3", "A1", "ANSI B", "ANSI D"][r.below(4)].into(), legend: r.below(4) == 0, ..Sheet::default() } });
    }
    for _ in 0..n {
        let sheet = d.p.sheets[r.below(d.p.sheets.len())].id;
        match r.below(10) {
            0..=4 => {
                let s = &syms[r.below(syms.len())];
                let on = if s.inline { d.p.lines.iter().filter(|l| l.sheet == sheet).map(|l| l.id).collect::<Vec<_>>().first().copied() } else { None };
                let _ = d.apply(Op::AddItem { item: Item { sheet, sym: s.key.into(), x: r.f(0.0, 400.0), y: r.f(0.0, 290.0), rot: r.below(4) as u8, flip: r.below(2) == 0, tag: if r.below(3) == 0 { HOSTILE[r.below(8)].into() } else { String::new() }, ..Item::default() }, on });
            }
            5..=7 => {
                let items: Vec<&Item> = d.p.items.iter().filter(|i| i.sheet == sheet).collect();
                let lines: Vec<u32> = d.p.lines.iter().filter(|l| l.sheet == sheet).map(|l| l.id).collect();
                let end = |r: &mut Rng| -> End {
                    match r.below(3) {
                        0 if !items.is_empty() => {
                            let it = items[r.below(items.len())];
                            let np = symbols::get(&it.sym).map_or(0, |s| s.ports.len());
                            if np == 0 { End::Free { x: r.f(0.0, 400.0), y: r.f(0.0, 290.0) } } else { End::Port { item: it.id, port: r.below(np) as u8 } }
                        }
                        1 if !lines.is_empty() => End::Line { line: lines[r.below(lines.len())], x: r.f(0.0, 400.0), y: r.f(0.0, 290.0) },
                        _ => End::Free { x: r.f(0.0, 400.0), y: r.f(0.0, 290.0) },
                    }
                };
                let (a, b) = (end(r), end(r));
                let _ = d.apply(Op::AddLine { line: Line { sheet, kind: Kind::ALL[r.below(7)], a, b, pts: if r.below(4) == 0 { vec![[r.f(0.0, 400.0), r.f(0.0, 290.0)]] } else { vec![] }, flip: r.below(2) == 0, ..Line::default() } });
            }
            8 => {
                let _ = d.apply(Op::AddNote { note: Note { sheet, kind: [NoteKind::Note, NoteKind::Leader, NoteKind::Hold][r.below(3)], x: r.f(0.0, 400.0), y: r.f(0.0, 290.0), w: r.f(0.0, 80.0), h: r.f(0.0, 60.0), text: HOSTILE[r.below(8)].into(), ..Note::default() } });
            }
            _ => {
                let ids: Vec<u32> = d.p.items.iter().map(|i| i.id).chain(d.p.lines.iter().map(|l| l.id)).collect();
                if let Some(&id) = r.pick(&ids) {
                    let key = ["size", "sched", "fluid", "seq", "class", "flow", "density", "viscosity", "length", "fittings", "loop", "relief", "design_p"][r.below(13)];
                    let val = if r.below(2) == 0 { Val::Num([0.0, -1.0, 1e300, 1e-300, 30.0 / 3600.0, 1000.0, 1e-3, 3.0][r.below(8)]) } else { Val::Text(["3", "1-1/2", "x", "40", "A1A", "P", "", "4"][r.below(8)].into()) };
                    let _ = d.apply(Op::SetData { ids: vec![id], key: key.into(), val: Some(val) });
                }
            }
        }
    }
    d.p
}

/// Everything the window, the lists, print and export can ask of a project.
fn exercise(p: &Project) {
    for s in &p.sheets {
        let a = api::derived(p, s.id);
        assert!(a.view.is_some());
        let text = dxf::write(p, s);
        dxf::read_back(&text).expect("the DXF reads back");
    }
    for (k, _) in lists::KINDS {
        let l = lists::build(p, k).unwrap();
        let _ = lists::csv(&l);
    }
    let _ = lists::xlsx(p, "lines").unwrap();
    let _ = view::all(p);
    let _ = crate::tags::renumber(p);
}

#[test]
fn random_projects_never_panic_and_stay_sound() {
    let mut r = Rng(0x9C1F_8401);
    for case in 0..40 {
        let p = random_project(&mut r, 20 + case);
        let mut q = p.clone();
        assert!(q.normalize().is_empty(), "every edit leaves the project sound (case {case})");
        exercise(&p);
        // Round trip through the file.
        let (back, w) = model::from_json(&model::to_json(&p)).unwrap();
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(back, p);
    }
}

#[test]
fn random_edits_never_panic_and_failed_ones_change_nothing() {
    let mut r = Rng(0x5EED);
    let mut d = Doc::new(random_project(&mut r, 40));
    for _ in 0..600 {
        let ids: Vec<u32> = d.p.items.iter().map(|i| i.id).chain(d.p.lines.iter().map(|l| l.id)).chain(d.p.notes.iter().map(|n| n.id)).chain([0, 999_999, u32::MAX]).collect();
        let id = *r.pick(&ids).unwrap();
        let sheet = d.p.sheets[r.below(d.p.sheets.len())].id;
        let op = match r.below(14) {
            0 => Op::Move { ids: vec![id], dx: [1.0, -1e9, f64::NAN, 5.0][r.below(4)], dy: 2.5 },
            1 => Op::Rotate { ids: vec![id] },
            2 => Op::Flip { ids: vec![id] },
            3 => Op::Delete { ids: vec![id] },
            4 => Op::SetTag { id, tag: HOSTILE[r.below(8)].into() },
            5 => Op::Split { line: id, item: *r.pick(&ids).unwrap() },
            6 => Op::SetPts { id, pts: vec![[r.f(-10.0, 400.0), r.f(-10.0, 300.0)]] },
            7 => Op::Paste { sheet, items: d.p.items.iter().take(3).cloned().collect(), lines: d.p.lines.iter().take(3).cloned().collect(), notes: vec![], dx: 10.0, dy: 10.0 },
            8 => Op::Link { a: id, b: *r.pick(&ids).unwrap() },
            9 => Op::DeleteSheet { id: sheet },
            10 => Op::Renumber,
            11 => Op::SetData { ids: vec![id], key: "flow".into(), val: Some(Val::Num(f64::INFINITY)) },
            12 => Op::MoveSheet { id: sheet, to: usize::MAX },
            _ => Op::AddItem { item: Item { sheet, sym: "gate".into(), x: r.f(0.0, 400.0), y: r.f(0.0, 290.0), ..Item::default() }, on: Some(id) },
        };
        let before = d.p.clone();
        let res = d.apply(op);
        if res.is_err() {
            assert_eq!(d.p, before, "a failed edit changes nothing");
        }
        let mut q = d.p.clone();
        assert!(q.normalize().is_empty());
        if r.below(20) == 0 {
            d.undo();
        }
    }
    exercise(&d.p);
}

#[test]
fn damaged_files_open_or_say_why() {
    let mut r = Rng(42);
    let p = random_project(&mut r, 60);
    let text = model::to_json(&p);
    let bytes = text.as_bytes();
    for k in 0..300 {
        let mut b = bytes[..r.below(bytes.len())].to_vec();
        if k % 2 == 0 {
            for _ in 0..5 {
                let i = r.below(bytes.len());
                if i < b.len() {
                    b[i] = r.next() as u8;
                }
            }
        }
        if let Ok((q, _)) = model::from_json(&String::from_utf8_lossy(&b)) {
            exercise(&q);
        }
    }
    // Wrong types, references to nothing, loops, a newer format.
    for bad in [
        r#"{"format":1,"sheets":"x","items":[{"id":5,"sym":"pump_cent","x":"a"}],"lines":[{"id":5,"a":{"at":"line","line":5,"x":0,"y":0},"b":{"at":"port","item":77,"port":9}}]}"#,
        r#"{"format":1,"items":[{"id":1,"sym":"offpage","link":1},{"id":2,"sym":"nope","scale":1e300,"x":1e300}],"lines":[{"id":3,"a":{"at":"line","line":4,"x":0,"y":0},"b":{"at":"free","x":0,"y":0}},{"id":4,"a":{"at":"line","line":3,"x":0,"y":0},"b":{"at":"free","x":1,"y":0}}],"next":4294967295}"#,
        r#"[]"#,
        r#"{"format":99}"#,
        "",
    ] {
        match model::from_json(bad) {
            Ok((q, _)) => exercise(&q),
            Err(e) => assert!(!e.is_empty()),
        }
    }
    assert!(model::from_json(r#"{"format":99}"#).unwrap_err().contains("newer"));
}

#[test]
fn hostile_values_are_clamped() {
    let mut p = Project::new();
    let s = p.sheets[0].id;
    p.items.push(Item { id: u32::MAX, sheet: s, sym: "vvessel".into(), x: f64::NAN, y: 1e300, scale: 0.0, rot: 200, ..Item::default() });
    p.lines.push(Line { id: 7, sheet: 999, a: End::Free { x: f64::INFINITY, y: 0.0 }, b: End::Port { item: u32::MAX, port: 99 }, ..Line::default() });
    p.settings.grid = -5.0;
    p.settings.roughness = f64::NAN;
    let w = p.normalize();
    assert!(!w.is_empty());
    let it = &p.items[0];
    assert!(it.x == 0.0 && it.y == model::MAX_COORD && it.scale == 0.25 && it.rot < 4);
    assert_eq!(p.lines[0].b, End::Free { x: 0.0, y: 0.0 });
    assert!(p.settings.grid >= 0.5 && p.settings.roughness.is_finite());
    exercise(&p);
}

#[test]
fn speed_gate_500_items() {
    // A drawing as big as a busy sheet set: 500 symbols, 500 lines.
    let mut p = Project::new();
    let s = p.sheets[0].id;
    for i in 0..500u32 {
        let id = p.id();
        p.items.push(Item { id, sheet: s, sym: if i % 2 == 0 { "vvessel" } else { "gate" }.into(), x: 20.0 + (i % 25) as f64 * 15.0, y: 20.0 + (i / 25) as f64 * 13.0, tag: format!("X-{i}"), ..Item::default() });
    }
    let ids: Vec<u32> = p.items.iter().map(|i| i.id).collect();
    for w in ids.windows(2) {
        let id = p.id();
        p.lines.push(Line { id, sheet: s, a: End::Port { item: w[0], port: 1 }, b: End::Port { item: w[1], port: 0 }, ..Line::default() });
    }
    assert!(p.normalize().is_empty());
    let t = std::time::Instant::now();
    let d = api::derived(&p, s);
    let ms = t.elapsed().as_millis();
    assert_eq!(d.view.unwrap().lines.len(), 499);
    let limit = if cfg!(debug_assertions) { 5_000 } else { 500 };
    assert!(ms < limit, "derived took {ms} ms");
    eprintln!("500 symbols, 499 lines: {ms} ms");
    // A realistic large sheet: the pump transfer tiled 23 times on an A1 sheet (about 500 symbols).
    let big = crate::examples::big(23);
    let t = std::time::Instant::now();
    let d = api::derived(&big, big.sheets[0].id);
    let ms = t.elapsed().as_millis();
    eprintln!("{} symbols, {} lines tiled: {ms} ms", big.items.len(), d.view.unwrap().lines.len());
    assert!(ms < limit, "derived took {ms} ms");
}
