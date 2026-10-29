//! Writes each example's DXF to a folder, for checking in other readers:
//! `cargo run -p datum-core --example dxf_out -- out/`.
fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    for e in datum_core::examples::list() {
        let p = datum_core::examples::get(e.id).unwrap();
        let path = format!("{dir}/{}.dxf", e.id);
        std::fs::write(&path, datum_core::dxf::write(&p, &p.sheets[0])).unwrap();
        println!("{path}");
    }
}
