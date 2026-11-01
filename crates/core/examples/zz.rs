fn main() {
    let mut p = datum_core::examples::get("pfd").unwrap();
    p.settings.components = vec!["Water".into(), "Methanol".into()];
    let t = "Stream results\nName,Units,1,2,S-99\nTemperature,C,25,80.5,10\nPressure,bara,5.01325,2,1\nMass Flow,kg/h,1000,2000,5\nStd Ideal Liq Vol Flow,m3/h,1,2,3\n";
    println!("{:#?}", datum_core::streamsin::plan(&p, t.as_bytes()).map(|p| p.changes.iter().map(|c| format!("{} {} {} {}", c.tag, c.key, c.kind, c.note)).collect::<Vec<_>>()));
    for o in p.objects.iter().filter(|o| o.class == "Stream") { println!("{} {:?} {:?}", o.tag, o.data.get("number"), o.data.get("name")); }
}
