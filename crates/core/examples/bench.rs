//! Where the time goes on a large drawing: `cargo run --release -p datum-core --example bench`.
use datum_core::model::{End, Item, Line, Project};
use datum_core::{checks, graph::Net, hydraulics, view};
use std::time::Instant;

fn main() {
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
    let t = Instant::now();
    let net = Net::new(&p);
    println!("net {:?}", t.elapsed());
    let t = Instant::now();
    let hyd = hydraulics::all(&p, &net);
    println!("hydraulics {:?}", t.elapsed());
    let t = Instant::now();
    let r = view::routes(&p, s, None);
    println!("routes {:?} ({})", t.elapsed(), r.len());
    let t = Instant::now();
    let v = view::sheet(&p, &p.sheets[0], &net, &hyd);
    println!("view {:?} ({} lines)", t.elapsed(), v.lines.len());
    let t = Instant::now();
    let c = checks::run(&p, &net, &hyd);
    println!("checks {:?} ({} issues)", t.elapsed(), c.len());
}
