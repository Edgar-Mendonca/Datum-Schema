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
                let tag: String = if r.below(3) == 0 { HOSTILE[r.below(8)].into() } else { String::new() };
                let _ = d.apply(Op::AddItem { item: Item { sheet, sym: s.key.into(), x: r.f(0.0, 400.0), y: r.f(0.0, 290.0), rot: r.below(4) as u8, flip: r.below(2) == 0, ..Item::default() }, on, tag, obj: None });
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
    for d in crate::datasheets::all(p) {
        assert!(!d.sections.is_empty());
    }
    let _ = crate::datasheets::xlsx(p);
    let _ = crate::datasheets::layout_rows(p);
    for l in crate::loops::all(p) {
        let d = crate::loops::diagram(p, &l.key).expect("every loop draws");
        crate::dxf::read_back(&crate::dxf::write_prims(&d.prims, d.w, d.h)).expect("a loop diagram's DXF reads back");
    }
    let m = crate::cause::matrix(p);
    assert_eq!(m.cells.len(), m.causes.len());
    let _ = crate::cause::xlsx(p);
    for s in &p.sheets {
        let (_, ids, refs) = crate::dexpi::tests::parse(&crate::dexpi::write(p, s));
        let set: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(set.len(), ids.len(), "DEXPI IDs are unique");
        assert!(refs.iter().all(|r| set.contains(r)), "DEXPI references resolve: {:?}", refs.iter().filter(|r| !set.contains(r)).collect::<Vec<_>>());
        let (mut q, _) = crate::dexpi::read(&crate::dexpi::write(p, s)).expect("our own DEXPI reads back");
        assert!(q.normalize().is_empty(), "an imported project is sound");
    }
    if let Ok(book) = crate::datasheets::xlsx(p) {
        crate::import::plan(p, &book).expect("an exported workbook reads back");
    }
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
        let op = match r.below(15) {
            0 => Op::Move { ids: vec![id], dx: [1.0, -1e9, f64::NAN, 5.0][r.below(4)], dy: 2.5 },
            1 => Op::Rotate { ids: vec![id] },
            2 => Op::Flip { ids: vec![id] },
            3 => if r.below(3) == 0 { Op::DeleteObjects { ids: vec![id] } } else { Op::Delete { ids: vec![id], keep: r.below(2) == 0 } },
            4 => Op::SetTag { id, tag: HOSTILE[r.below(8)].into() },
            5 => Op::Split { line: id, item: *r.pick(&ids).unwrap() },
            6 => Op::SetPts { id, pts: vec![[r.f(-10.0, 400.0), r.f(-10.0, 300.0)]] },
            7 => Op::Paste { sheet, items: d.p.items.iter().take(3).cloned().collect(), lines: d.p.lines.iter().take(3).cloned().collect(), notes: vec![], dx: 10.0, dy: 10.0 },
            8 => Op::Link { a: id, b: *r.pick(&ids).unwrap() },
            9 => Op::DeleteSheet { id: sheet },
            10 => Op::Renumber,
            11 => Op::SetData { ids: vec![id], key: "flow".into(), val: Some(Val::Num(f64::INFINITY)) },
            12 => Op::MoveSheet { id: sheet, to: usize::MAX },
            13 => Op::IssueDatasheet { ids: vec![id], rev: [String::new(), HOSTILE[r.below(8)].into()][r.below(2)].clone(), status: ["Preliminary", "As-built", "", "<b>x</b>"][r.below(4)].into(), date: HOSTILE[r.below(8)].into(), note: HOSTILE[r.below(8)].into(), by: String::new() },
            _ => Op::AddItem { item: Item { sheet, sym: "gate".into(), x: r.f(0.0, 400.0), y: r.f(0.0, 290.0), ..Item::default() }, on: Some(id), tag: String::new(), obj: None },
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

/// Random and damaged DXF into the legend scan and the sheet background: an answer or a clear
/// error, never a panic; whatever is read is a sound library or background.
#[test]
fn random_dxf_never_panics() {
    let mut r = Rng(0x5eed_d0f0);
    let good = dxf::write(&crate::examples::get("pump").unwrap(), &crate::examples::get("pump").unwrap().sheets[0]);
    let words = ["0", "SECTION", "ENTITIES", "BLOCKS", "BLOCK", "ENDBLK", "INSERT", "ATTRIB", "LWPOLYLINE", "LINE", "CIRCLE", "ARC", "TEXT", "MTEXT", "HATCH", "SEQEND", "ENDSEC", "EOF", "2", "8", "10", "20", "40", "42", "70", "90", "-1e308", "nan", "1e9", "VLV", "*U2"];
    let sheet = Sheet::default();
    for i in 0..60 {
        let text: String = if i % 3 == 0 {
            // A real drawing with lines cut out or swapped.
            let mut ls: Vec<&str> = good.lines().collect();
            for _ in 0..r.below(40) + 1 {
                let (a, b) = (r.below(ls.len()), r.below(ls.len()));
                if r.below(2) == 0 { ls.swap(a, b) } else { ls.remove(a); }
            }
            ls.join("\n")
        } else {
            (0..r.below(3000)).map(|_| *r.pick(&words).unwrap()).collect::<Vec<_>>().join("\n")
        };
        if let Ok(scan) = crate::symlib::scan_legend(&text, HOSTILE[i % HOSTILE.len()]) {
            let (lib, _) = crate::symlib::validate(scan.library);
            for c in &lib.symbols {
                assert!(c.w.is_finite() && c.h.is_finite() && c.ports.iter().all(|q| q.x.is_finite() && q.y.is_finite()));
            }
        }
        if let Ok((u, _)) = crate::dxfin::underlay(&text, HOSTILE[i % HOSTILE.len()], &sheet) {
            assert!(u.k.is_finite() && u.k > 0.0 && u.x.is_finite() && u.y.is_finite());
            let _ = symbols::svg(&u.prims());
        }
    }
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
        let sym = if i % 2 == 0 { "vvessel" } else { "gate" };
        let obj = p.new_obj(sym, &format!("X-{i}"));
        p.items.push(Item { id, sheet: s, sym: sym.into(), x: 20.0 + (i % 25) as f64 * 15.0, y: 20.0 + (i / 25) as f64 * 13.0, obj, ..Item::default() });
    }
    let ids: Vec<u32> = p.items.iter().map(|i| i.id).collect();
    for w in ids.windows(2) {
        let id = p.id();
        let obj = p.new_obj(model::LINE_CLASS, "");
        p.lines.push(Line { id, sheet: s, a: End::Port { item: w[0], port: 1 }, b: End::Port { item: w[1], port: 0 }, obj, ..Line::default() });
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

/// Regressions for the findings of the independent engine review (0.1.3).
mod review {
    use super::*;
    use crate::checks::tests::B;
    use crate::graph::Net;

    #[test]
    fn three_lines_on_a_valve_join_by_number() {
        let mut b = B::new();
        let (v1, v2, v3) = (b.item("vvessel", 50.0, 100.0), b.item("vvessel", 250.0, 100.0), b.item("vvessel", 250.0, 200.0));
        let g = b.item("gate", 150.0, 108.0);
        let l1 = b.line((v1, 5), (g, 0));
        let l3 = b.line((g, 1), (v3, 3));
        let l2 = b.line((g, 1), (v2, 3));
        b.spec(&[l1, l2], "3", "A1A");
        b.spec(&[l3], "3", "A1A");
        b.set(&[l1, l2], "seq", Val::Text("7".into()));
        b.set(&[l3], "seq", Val::Text("9".into()));
        let net = Net::new(&b.d.p);
        assert_eq!(net.run_of[&l1], net.run_of[&l2]);
        assert!(!b.issues().iter().any(|i| i.rule == 1));
    }

    #[test]
    fn a_pasted_pipeline_keeps_one_number() {
        let mut b = B::new();
        let (v1, v2) = (b.item("vvessel", 50.0, 100.0), b.item("vvessel", 250.0, 100.0));
        let l = b.line((v1, 5), (v2, 3));
        b.spec(&[l], "3", "A1A");
        let sheet = b.sheet();
        let made = b.d.apply(Op::AddItem { item: Item { sheet, sym: "gate".into(), x: 150.0, y: 108.0, ..Item::default() }, on: Some(l), tag: String::new(), obj: None }).unwrap();
        let p = b.d.p.clone();
        let lines: Vec<Line> = p.lines.iter().filter(|x| x.id == l || x.id == made[1]).cloned().collect();
        let items = vec![p.item(made[0]).unwrap().clone()];
        let new = b.d.apply(Op::Paste { sheet, items, lines, notes: vec![], dx: 0.0, dy: 60.0 }).unwrap();
        let seqs: Vec<String> = new[1..].iter().map(|id| b.d.p.data_of(b.d.p.line(*id).unwrap().obj)["seq"].text()).collect();
        assert_eq!(seqs[0], seqs[1]);
        assert_ne!(seqs[0], p.data_of(p.line(l).unwrap().obj)["seq"].text());
    }

    #[test]
    fn check_valve_found_across_sheets() {
        let mut b = B::new();
        let pm = b.item("pump_cent", 50.0, 100.0);
        let a = b.item("offpage", 150.0, 94.0);
        b.line((pm, 1), (a, 0));
        let s2 = b.d.apply(Op::AddSheet { sheet: Sheet::default() }).unwrap()[0];
        let add = |b: &mut B, sym: &str, x: f64| b.d.apply(Op::AddItem { item: Item { sheet: s2, sym: sym.into(), x, y: 100.0, ..Item::default() }, on: None, tag: String::new(), obj: None }).unwrap()[0];
        let c = add(&mut b, "offpage", 30.0);
        let ck = add(&mut b, "check", 80.0);
        let v = add(&mut b, "vvessel", 200.0);
        b.d.apply(Op::Link { a, b: c }).unwrap();
        for (x, y) in [((c, 0), (ck, 0)), ((ck, 1), (v, 2))] {
            b.d.apply(Op::AddLine { line: Line { sheet: s2, a: End::Port { item: x.0, port: x.1 }, b: End::Port { item: y.0, port: y.1 }, ..Line::default() } }).unwrap();
        }
        assert!(!b.has(8), "{:?}", b.issues());
    }

    #[test]
    fn deleting_a_valve_in_a_loop_is_allowed_and_merge_keeps_data() {
        let mut b = B::new();
        let v = b.item("vvessel", 50.0, 100.0);
        let g = b.item("gate", 150.0, 108.0);
        let a = b.line((v, 5), (g, 0));
        let sheet = b.sheet();
        b.d.apply(Op::AddLine { line: Line { sheet, a: End::Port { item: g, port: 1 }, b: End::Line { line: a, x: 100.0, y: 108.0 }, ..Line::default() } }).unwrap();
        assert!(b.d.apply(Op::Delete { ids: vec![g], keep: false }).is_ok());
        // A plain merge keeps data only the second half had.
        let mut c = B::new();
        let (v1, v2) = (c.item("vvessel", 50.0, 100.0), c.item("vvessel", 250.0, 100.0));
        let g = c.item("gate", 150.0, 108.0);
        let (l1, l2) = (c.line((v1, 5), (g, 0)), c.line((g, 1), (v2, 3)));
        c.set(&[l2], "flow", Val::Num(0.01));
        c.d.apply(Op::Delete { ids: vec![g], keep: false }).unwrap();
        assert_eq!(c.d.p.data_of(c.d.p.line(l1).unwrap().obj).get("flow"), Some(&Val::Num(0.01)));
    }

    #[test]
    fn running_out_of_ids_is_an_error_not_a_silent_loss() {
        let mut p = Project::new();
        p.next = u32::MAX;
        let mut d = Doc::new(p);
        let s = d.p.sheets[0].id;
        let add = |d: &mut Doc| d.apply(Op::AddItem { item: Item { sheet: s, sym: "gate".into(), ..Item::default() }, on: None, tag: String::new(), obj: None });
        let first = add(&mut d);
        let second = add(&mut d);
        assert!(first.is_ok() || second.is_err());
        assert!(second.is_err(), "the second add must fail, not vanish");
    }

    #[test]
    fn csv_quotes_a_carriage_return_and_dxf_handles_wide_characters() {
        let mut b = B::new();
        let v = b.item("vvessel", 50.0, 100.0);
        b.set(&[v], "service", Val::Text("a\rb".into()));
        assert!(lists::csv(&lists::build(&b.d.p, "equipment").unwrap()).contains("\"a\rb\""));
        b.tag(v, "V-1😀");
        let t = dxf::write(&b.d.p, &b.d.p.sheets[0]);
        assert!(t.contains("V-1?") && !t.contains("1F600"));
        // Symbol letters are upright text, not inside the blocks.
        let m = b.item("motor", 100.0, 100.0);
        let _ = m;
        let t = dxf::write(&b.d.p, &b.d.p.sheets[0]);
        let block = &t[t.find("SYM_MOTOR").unwrap()..t.find("ENDBLK").unwrap()];
        assert!(!block.contains("TEXT"));
    }
}
