//! A large project (120 sheets, each a copy of the pump transfer): how long an edit, the open
//! sheet, the checks, the lists and the to-do list take.
//! `cargo run --release -p datum-core --example bench_big`
use datum_core::doc::{Doc, Op};
use datum_core::model::Sheet;
use std::time::Instant;

fn main() {
    let one = datum_core::examples::get("pump").unwrap();
    let mut d = Doc::new(one.clone());
    let t = Instant::now();
    for _ in 1..120 {
        let s = d.apply(Op::AddSheet { sheet: Sheet::default() }).unwrap()[0];
        d.apply(Op::Paste { sheet: s, items: one.items.clone(), lines: one.lines.clone(), notes: vec![], dx: 0.0, dy: 0.0 }).unwrap();
    }
    println!("built {} sheets, {} symbols, {} lines in {:?}", d.p.sheets.len(), d.p.items.len(), d.p.lines.len(), t.elapsed());
    let id = d.p.items[5].id;
    let t = Instant::now();
    for _ in 0..5 {
        d.apply(Op::Move { ids: vec![id], dx: 2.5, dy: 0.0 }).unwrap();
    }
    println!("one edit (move a symbol): {:?}", t.elapsed() / 5);
    let s = d.p.sheets[60].id;
    {
        use datum_core::{checks, graph::Net, hydraulics, view};
        let t = Instant::now();
        let net = Net::new(&d.p);
        println!("  net {:?}", t.elapsed());
        let t = Instant::now();
        let hyd = hydraulics::all(&d.p, &net);
        println!("  hydraulics {:?}", t.elapsed());
        let t = Instant::now();
        view::sheet(&d.p, d.p.sheet(s).unwrap(), &net, &hyd);
        println!("  view {:?}", t.elapsed());
        let on = |_: u8| true;
        let _ = on;
        let mut q = d.p.clone();
        for r in 1..=datum_core::checks::RULES {
            q.settings.off = (1..=datum_core::checks::RULES).filter(|x| *x != r).collect();
            let t = Instant::now();
            checks::run(&q, &net, &hyd);
            let e = t.elapsed();
            if e.as_millis() > 20 {
                println!("  check {r}: {e:?}");
            }
        }
    }
    let t = Instant::now();
    let v = datum_core::api::derived(&d.p, s);
    println!("the open sheet (view, checks, hydraulics): {:?} ({} issues)", t.elapsed(), v.issues.len());
    let t = Instant::now();
    for k in ["lines", "valves", "takeoff", "instruments"] {
        datum_core::lists::build(&d.p, k).unwrap();
    }
    println!("four lists: {:?}", t.elapsed());
    let t = Instant::now();
    let n = datum_core::propagate::todo(&d.p).len();
    println!("to do: {:?} ({n})", t.elapsed());
    let t = Instant::now();
    let path = std::env::temp_dir().join("bench-big.dschema");
    datum_core::file::write(&path, &d.p, &d.history).unwrap();
    println!("save: {:?}", t.elapsed());
    let t = Instant::now();
    datum_core::file::read(&path).unwrap();
    println!("open: {:?}", t.elapsed());
    let _ = std::fs::remove_file(&path);
}
