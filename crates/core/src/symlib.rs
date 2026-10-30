//! Symbol libraries: a client's own symbols, read from their legend sheet (a DXF), kept in a
//! library file of their own and used in any project. Each symbol says which of Datum Schema's
//! symbols it stands for ("like": a gate valve, a centrifugal pump…), which gives it its class,
//! category, tag prefix and the names of its connection points; so objects drawn with it are the
//! same objects, with the same data, checks and lists.
//!
//! A project's drawing standard maps Datum Schema's symbols to a library's: every symbol drawn is
//! shown with the client's drawing, and switching the standard back shows ours again. A project
//! keeps a copy of every library symbol it uses, so it opens anywhere.

use crate::dxfin::{self, Ent, Tf};
use crate::symbols::{self, Cat, Port, Prim, Sym};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct CPort {
    pub x: f64,
    pub y: f64,
    /// The direction a line leaves it: 0 right, 1 down, 2 left, 3 up.
    pub dir: u8,
    pub name: String,
    pub signal: bool,
}

/// A library symbol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CustomSym {
    /// Unique: "x.<library>.<block>".
    pub key: String,
    /// Its description (from the legend).
    pub name: String,
    pub library: String,
    /// The block name in the client's drawing (kept for DXF export).
    pub block: String,
    /// The Datum Schema symbol it stands for ("" for none).
    pub like: String,
    pub class: String,
    pub cat: Cat,
    pub w: f64,
    pub h: f64,
    /// The drawing, in the symbol language (`symbols` module), millimetres round its centre.
    pub draw: String,
    pub ports: Vec<CPort>,
    pub prefix: String,
    pub inline: bool,
    pub terminal: bool,
}

impl Default for CustomSym {
    fn default() -> Self {
        Self { key: String::new(), name: String::new(), library: String::new(), block: String::new(), like: String::new(), class: "UnknownItem".into(), cat: Cat::Mark, w: 10.0, h: 10.0, draw: String::new(), ports: vec![], prefix: String::new(), inline: false, terminal: false }
    }
}

/// A library file (`.dschemalib`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Library {
    pub format: u32,
    pub name: String,
    /// The file it was read from.
    pub source: String,
    pub symbols: Vec<CustomSym>,
}

pub const LIB_FORMAT: u32 = 1;
const MAX_SYMS: usize = 2000;
const MAX_DRAW: usize = 200_000;

/// A name made safe for a key: letters, digits and dashes, lower case.
pub fn slug(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            o.push(c);
        } else if !o.ends_with('-') {
            o.push('-');
        }
    }
    let o = o.trim_matches('-').to_string();
    crate::clip(if o.is_empty() { "symbol" } else { &o }, 60)
}

// ---------------------------------------------------------------- the registry

struct Entry {
    sym: &'static Sym,
    def: CustomSym,
}

static REG: RwLock<Vec<Entry>> = RwLock::new(Vec::new());

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// Makes a library symbol known to the engine (and so drawable). The same definition twice is
/// kept once; a changed one replaces the old.
pub fn register(c: &CustomSym) {
    let mut reg = REG.write().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = reg.iter().find(|e| e.def.key == c.key) {
        if e.def == *c {
            return;
        }
    }
    let ports: Vec<Port> = c.ports.iter().map(|q| Port { x: q.x, y: q.y, dir: q.dir % 4, name: leak(&q.name), signal: q.signal }).collect();
    let sym: &'static Sym = Box::leak(Box::new(Sym { key: leak(&c.key), name: leak(&c.name), cat: c.cat, w: c.w, h: c.h, ports, draw: leak(&c.draw), prefix: leak(&c.prefix), inline: c.inline, terminal: c.terminal, rare: false }));
    reg.retain(|e| e.def.key != c.key);
    reg.push(Entry { sym, def: c.clone() });
}

/// A registered library symbol.
pub fn get(key: &str) -> Option<&'static Sym> {
    if !key.starts_with("x.") {
        return None;
    }
    REG.read().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e.def.key == key).map(|e| e.sym)
}

/// A registered library symbol's definition.
pub fn definition(key: &str) -> Option<CustomSym> {
    REG.read().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e.def.key == key).map(|e| e.def.clone())
}

/// Every registered library symbol.
pub fn registered() -> Vec<&'static Sym> {
    REG.read().unwrap_or_else(|e| e.into_inner()).iter().map(|e| e.sym).collect()
}

// ---------------------------------------------------------------- drawing language

fn n(v: f64) -> String {
    crate::fmt_num((v * 1000.0).round() / 1000.0)
}

/// Primitives to the symbol language.
pub fn to_draw(ps: &[Prim]) -> String {
    let mut out: Vec<String> = vec![];
    for p in ps {
        match p {
            Prim::Poly { pts, closed, fill, dash, .. } if pts.len() >= 2 => {
                let op = if *fill { "f" } else if *dash { "d" } else if *closed { "p" } else { "l" };
                out.push(format!("{op} {}", pts.iter().map(|q| format!("{} {}", n(q[0]), n(q[1]))).collect::<Vec<_>>().join(" ")));
            }
            Prim::Circle { c, r, fill, .. } => out.push(format!("{} {} {} {}", if *fill { "fc" } else { "c" }, n(c[0]), n(c[1]), n(*r))),
            Prim::Arc { c, r, a0, a1, .. } => out.push(format!("a {} {} {} {} {}", n(c[0]), n(c[1]), n(*r), n(*a0), n(*a1))),
            Prim::Text { p, h, s, anchor, .. } => {
                let t: String = s.chars().filter(|c| *c != ';' && !c.is_control()).collect();
                if !t.trim().is_empty() {
                    let op = ["tl", "t", "tr"][(*anchor as usize).min(2)];
                    out.push(format!("{op} {} {} {} {}", n(p[0]), n(p[1]), n(*h), t.trim()));
                }
            }
            _ => {}
        }
    }
    out.join("; ")
}

// ---------------------------------------------------------------- checking a library

/// A library made sound: names clipped, keys unique and namespaced, sizes and ports in range,
/// drawings that parse. Returns it with what was changed, in words.
pub fn validate(mut lib: Library) -> (Library, Vec<String>) {
    let mut w = vec![];
    lib.format = LIB_FORMAT;
    lib.name = crate::clip(lib.name.trim(), 80);
    if lib.name.is_empty() {
        lib.name = "Library".into();
    }
    if lib.symbols.len() > MAX_SYMS {
        w.push(format!("Only the first {MAX_SYMS} symbols were kept."));
        lib.symbols.truncate(MAX_SYMS);
    }
    let ls = slug(&lib.name);
    let mut keys = std::collections::HashSet::new();
    let before = lib.symbols.len();
    lib.symbols.retain(|s| !s.draw.trim().is_empty() && s.draw.len() <= MAX_DRAW && !symbols::parse(&s.draw).is_empty());
    if lib.symbols.len() < before {
        w.push(format!("{} symbols with no drawing were left out.", before - lib.symbols.len()));
    }
    for s in &mut lib.symbols {
        s.library = lib.name.clone();
        let base = format!("x.{ls}.{}", slug(if s.block.is_empty() { &s.name } else { &s.block }));
        let mut key = base.clone();
        let mut k = 2;
        while !keys.insert(key.clone()) {
            key = format!("{base}-{k}");
            k += 1;
        }
        s.key = key;
        clean(s);
    }
    (lib, w)
}

/// One symbol's names, sizes, class and ports brought into range.
fn clean(s: &mut CustomSym) {
    s.name = crate::clip(s.name.trim(), 120);
    s.block = crate::clip(&s.block, 120);
    s.library = crate::clip(&s.library, 80);
    if !s.like.is_empty() && symbols::builtin(&s.like).is_none() {
        s.like.clear();
    }
    if crate::classes::get(&s.class).is_none() {
        s.class = if s.like.is_empty() { "UnknownItem".into() } else { crate::classes::class_name_for(&s.like) };
    }
    let fix = |v: f64, d: f64| if v.is_finite() && v > 0.5 && v < 400.0 { v } else { d };
    s.w = fix(s.w, 10.0);
    s.h = fix(s.h, 10.0);
    s.prefix = crate::clip(s.prefix.trim(), 8);
    s.ports.truncate(16);
    for q in &mut s.ports {
        q.x = if q.x.is_finite() { q.x.clamp(-s.w / 2.0, s.w / 2.0) } else { 0.0 };
        q.y = if q.y.is_finite() { q.y.clamp(-s.h / 2.0, s.h / 2.0) } else { 0.0 };
        q.dir %= 4;
        q.name = crate::clip(q.name.trim(), 30);
        if q.name.is_empty() {
            q.name = "port".into();
        }
    }
    // In-line means exactly two process ports.
    if s.inline && s.ports.iter().filter(|q| !q.signal).count() != 2 {
        s.inline = false;
    }
}

/// A project's copies of library symbols checked, keeping their keys: those without a library
/// key or a drawing are dropped (said in words), duplicates kept once.
pub fn check_copies(v: Vec<CustomSym>) -> (Vec<CustomSym>, Vec<String>) {
    let mut keys = std::collections::HashSet::new();
    let n = v.len();
    let mut out: Vec<CustomSym> = v.into_iter().take(MAX_SYMS).filter(|s| s.key.starts_with("x.") && s.key.len() <= 200 && !s.draw.trim().is_empty() && s.draw.len() <= MAX_DRAW && !symbols::parse(&s.draw).is_empty() && keys.insert(s.key.clone())).collect();
    out.iter_mut().for_each(clean);
    let w = if out.len() < n { vec![format!("{} library symbols in the file could not be read and were left out.", n - out.len())] } else { vec![] };
    (out, w)
}

/// The direction a port at (x, y) on a w × h box leaves it: towards the nearest side.
pub fn side_dir(x: f64, y: f64, w: f64, h: f64) -> u8 {
    let d = [(w / 2.0 - x, 0u8), (h / 2.0 - y, 1), (x + w / 2.0, 2), (y + h / 2.0, 3)];
    d.iter().min_by(|a, b| a.0.total_cmp(&b.0)).map_or(0, |x| x.1)
}

// ---------------------------------------------------------------- reading a legend

/// Words of a description or block name, with the abbreviations of legends spelled out.
fn words(s: &str) -> Vec<String> {
    let mut v = vec![];
    for w in s.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()) {
        let exp: &[&str] = match w {
            "vlv" | "valv" | "vv" | "valves" => &["valve"],
            "cv" => &["control", "valve"],
            "psv" | "prv" | "srv" | "relief" | "safety" => &["safety", "valve"],
            "pmp" | "pumps" => &["pump"],
            "cent" | "centr" => &["centrifugal"],
            "comp" | "compr" | "cmp" => &["compressor"],
            "exch" | "hx" | "hex" | "exchanger" | "heatexchanger" => &["exchanger"],
            "ko" | "knockout" => &["knock", "out"],
            "inst" | "instr" | "instruments" => &["instrument"],
            "xmtr" | "tx" | "transmitter" => &["transmitter", "instrument"],
            "chk" | "nrv" | "nonreturn" | "ckv" => &["check"],
            "bfv" | "bfly" => &["butterfly"],
            "glb" => &["globe"],
            "gv" => &["gate", "valve"],
            "bv" => &["ball", "valve"],
            "rd" | "burst" | "bursting" => &["rupture", "disc"],
            "disk" => &["disc"],
            "str" | "strnr" => &["strainer"],
            "red" | "reducer" | "conc" => &["reducer", "concentric"],
            "ecc" => &["eccentric"],
            "dcs" | "shared" => &["shared", "display"],
            "plc" => &["programmable", "logic"],
            "field" | "fld" => &["field", "mounted"],
            "loc" | "local" => &["local", "panel"],
            "vsl" | "vessel" | "vertical" => &["vessel"],
            "tk" | "tank" => &["tank"],
            "col" | "tower" | "column" => &["column"],
            "blower" => &["fan"],
            "mtr" | "motor" => &["motor"],
            "trb" => &["turbine"],
            "aircooler" | "fin" | "fan-cooled" => &["air", "cooler"],
            "opc" | "offpage" | "continuation" | "cont" => &["off", "page"],
            "bl" | "battery" => &["battery", "limit"],
            "sg" => &["sight", "glass"],
            "ro" => &["restriction", "orifice"],
            "fe" | "orifice" => &["orifice"],
            "mag" | "magnetic" => &["magnetic"],
            "tw" | "thermowell" => &["thermowell"],
            "ps" => &["pressure"],
            "with" | "and" | "the" | "of" | "type" | "symbol" | "sym" | "blk" | "block" | "pid" | "p" | "id" | "general" | "typ" => &[],
            other => {
                v.push(other.trim_end_matches('s').to_string());
                continue;
            }
        };
        v.extend(exp.iter().map(|x| x.to_string()));
    }
    v.sort();
    v.dedup();
    v
}

/// The Datum Schema symbol a description and block name most likely stand for, with a score
/// from 0 to 1.
pub fn suggest(desc: &str, block: &str) -> Option<(&'static str, f64)> {
    let mine: Vec<String> = words(&format!("{desc} {block}"));
    if mine.is_empty() {
        return None;
    }
    let mut best: Option<(&'static str, f64)> = None;
    for s in symbols::all() {
        let class = crate::classes::for_symbol(s.key).map(|c| c.label.clone()).unwrap_or_default();
        let theirs = words(&format!("{} {} {}", s.name, s.key.replace('_', " "), class));
        let common = mine.iter().filter(|w| theirs.contains(w)).count() as f64;
        if common == 0.0 {
            continue;
        }
        let score = common / ((mine.len() as f64) * (theirs.len() as f64)).sqrt();
        if best.is_none_or(|b| score > b.1) {
            best = Some((s.key, score));
        }
    }
    best.filter(|b| b.1 >= 0.25)
}

/// Snaps a point to the nearest place on a drawing (a vertex or a point on a line), within `tol`.
fn snap(ps: &[Prim], q: [f64; 2], tol: f64) -> [f64; 2] {
    let mut best = (tol, q);
    let mut try_pt = |p: [f64; 2]| {
        let d = (p[0] - q[0]).hypot(p[1] - q[1]);
        if d < best.0 {
            best = (d, p);
        }
    };
    for p in ps {
        match p {
            Prim::Poly { pts, .. } => {
                for w in pts.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                    let l2 = dx * dx + dy * dy;
                    let t = if l2 > 0.0 { (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
                    try_pt([a[0] + t * dx, a[1] + t * dy]);
                }
                pts.iter().for_each(|p| try_pt(*p));
            }
            Prim::Circle { c, r, .. } => {
                let (dx, dy) = (q[0] - c[0], q[1] - c[1]);
                let d = dx.hypot(dy).max(1e-9);
                try_pt([c[0] + dx / d * r, c[1] + dy / d * r]);
            }
            _ => {}
        }
    }
    best.1
}

/// Ports for a drawing of w × h that stands for `like`: the built-in symbol's ports, placed at the
/// same relative places and snapped onto the client's drawing.
pub fn ports_like(like: &Sym, ps: &[Prim], w: f64, h: f64) -> Vec<CPort> {
    like.ports
        .iter()
        .map(|q| {
            let at = [q.x / like.w.max(1e-9) * w, q.y / like.h.max(1e-9) * h];
            let p = snap(ps, at, 0.2 * w.max(h));
            let (x, y) = (p[0].clamp(-w / 2.0, w / 2.0), p[1].clamp(-h / 2.0, h / 2.0));
            CPort { x, y, dir: q.dir, name: q.name.to_string(), signal: q.signal }
        })
        .collect()
}

/// A symbol drafted from a drawing: centred, sized, with a class and ports from `like`.
pub fn draft(mut ps: Vec<Prim>, name: &str, block: &str, like: Option<&str>) -> Option<CustomSym> {
    let b = dxfin::bbox(&ps.iter().filter(|p| !matches!(p, Prim::Text { .. })).cloned().collect::<Vec<_>>())?;
    let (cw, ch) = (b[2] - b[0], b[3] - b[1]);
    let like_sym = like.and_then(symbols::builtin);
    // Legends are drawn at any scale: a symbol much larger or smaller than ours is brought to
    // the size of the one it stands for (12 mm across without one).
    let target = like_sym.map_or(12.0, |s| s.w.max(s.h));
    let size = cw.max(ch);
    let k = if !(2.0..=80.0).contains(&size) && size > 1e-9 { target / size } else { 1.0 };
    dxfin::transform(&mut ps, [-(b[0] + b[2]) / 2.0, -(b[1] + b[3]) / 2.0], k);
    let (w, h) = ((cw * k).max(1.0), (ch * k).max(1.0));
    let mut s = CustomSym { name: name.to_string(), block: block.to_string(), w, h, draw: to_draw(&ps), ..CustomSym::default() };
    if let Some(l) = like_sym {
        s.like = l.key.to_string();
        s.class = crate::classes::class_name_for(l.key);
        s.cat = l.cat;
        s.prefix = l.prefix.to_string();
        s.inline = l.inline;
        s.terminal = l.terminal;
        s.ports = ports_like(l, &ps, w, h);
    }
    Some(s)
}

/// What a legend scan found.
#[derive(Clone, Debug, Serialize, Default)]
pub struct Scan {
    pub library: Library,
    pub warnings: Vec<String>,
}

fn pretty(block: &str) -> String {
    let s: String = block.chars().map(|c| if c == '_' || c == '-' || c == '.' { ' ' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

/// Reads a legend sheet (DXF): every block placed on it becomes a symbol, described by the text
/// beside it (or its attributes, or its block name), with a suggested Datum Schema symbol.
pub fn scan_legend(text: &str, name: &str) -> Result<Scan, String> {
    let d = dxfin::read(text)?;
    let mut warnings = vec![];
    // Texts of the sheet, in millimetres with y down.
    let texts: Vec<([f64; 2], f64, String)> = d
        .ents
        .iter()
        .filter_map(|e| match e {
            Ent::Text { p, h, s, .. } => Some(([p[0] * d.mm, -p[1] * d.mm], h * d.mm, s.clone())),
            _ => None,
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut symbols_out = vec![];
    let mut big = 0;
    for e in &d.ents {
        let Ent::Insert { name: block, p, sx, sy, attribs, .. } = e else { continue };
        if block.starts_with('*') || !seen.insert(block.clone()) {
            continue;
        }
        let Some(ents) = d.blocks.get(block) else { continue };
        let mut ps = vec![];
        dxfin::flatten(&d, ents, Tf::insert([0.0, 0.0], sx.abs().max(1e-9), sy.abs().max(1e-9), 0.0), &mut ps, 0);
        let Some(b) = dxfin::bbox(&ps.iter().filter(|p| !matches!(p, Prim::Text { .. })).cloned().collect::<Vec<_>>()) else { continue };
        let size = (b[2] - b[0]).max(b[3] - b[1]);
        if size > 150.0 {
            // A frame or title block, not a symbol.
            big += 1;
            continue;
        }
        // The text beside it: to its right, on its row.
        let at = [p[0] * d.mm, -p[1] * d.mm];
        let (x1, ymid, hh) = (at[0] + b[2], at[1] + (b[1] + b[3]) / 2.0, (b[3] - b[1]).max(2.0));
        let beside = texts
            .iter()
            .filter(|t| t.0[0] >= x1 - 1.0 && (t.0[1] - ymid).abs() <= hh.max(t.1 * 1.5) && t.0[0] - x1 < 25.0 * size.max(5.0))
            .min_by(|a, b2| (a.0[0] - x1).total_cmp(&(b2.0[0] - x1)))
            .map(|t| t.2.clone());
        let attr = attribs.iter().map(|a| a.value.trim()).filter(|v| v.len() > 3).collect::<Vec<_>>().join(" ");
        let desc = beside.or((!attr.is_empty()).then_some(attr)).unwrap_or_else(|| pretty(block));
        let like = suggest(&desc, block).map(|x| x.0);
        if let Some(s) = draft(ps, &desc, block, like) {
            symbols_out.push(s);
        }
    }
    if big > 0 {
        warnings.push(format!("{big} large blocks (frames or title blocks) were left out."));
    }
    if symbols_out.is_empty() {
        return Err("No symbols were found: a legend sheet has its symbols as blocks placed on the sheet, each with its description beside it.".into());
    }
    if !d.skipped.is_empty() {
        warnings.push(format!("Not read: {}.", d.skipped.iter().map(|(k, v)| format!("{v} {k}")).collect::<Vec<_>>().join(", ")));
    }
    let unmatched = symbols_out.iter().filter(|s| s.like.is_empty()).count();
    if unmatched > 0 {
        warnings.push(format!("{unmatched} symbols could not be matched to a Datum Schema symbol: choose what each stands for."));
    }
    let (library, mut w) = validate(Library { format: LIB_FORMAT, name: name.to_string(), source: String::new(), symbols: symbols_out });
    warnings.append(&mut w);
    Ok(Scan { library, warnings })
}

// ---------------------------------------------------------------- library files

/// A library as the window shows it: its file name, the library, and each symbol's picture.
#[derive(Clone, Debug, Serialize, Default)]
pub struct LibView {
    pub file: String,
    pub library: Library,
    pub svgs: Vec<String>,
    pub warnings: Vec<String>,
}

/// A symbol's drawing as SVG (shapes, then its letters).
pub fn svg_of(c: &CustomSym) -> String {
    symbols::svg(&symbols::parse(&c.draw))
}

pub fn view(file: &str, library: Library, warnings: Vec<String>) -> LibView {
    let svgs = library.symbols.iter().map(svg_of).collect();
    LibView { file: file.to_string(), library, svgs, warnings }
}

/// Reads a library file: checked, its symbols registered.
pub fn read_file(path: &std::path::Path) -> Result<Library, String> {
    let text = crate::doc::read_limited(path)?;
    let lib: Library = serde_json::from_str(&text).map_err(|e| format!("{} is not a Datum Schema symbol library ({e}).", path.display()))?;
    if lib.format > LIB_FORMAT {
        return Err(format!("{} was saved by a newer Datum Schema; update to use it.", path.display()));
    }
    let (lib, _) = validate(lib);
    lib.symbols.iter().for_each(register);
    Ok(lib)
}

/// Every library in a folder (files ending .dschemalib), read and registered; files that cannot
/// be read are skipped.
pub fn load_dir(dir: &std::path::Path) -> Vec<LibView> {
    let mut v: Vec<LibView> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "dschemalib"))
        .filter_map(|e| read_file(&e.path()).ok().map(|l| view(&e.file_name().to_string_lossy(), l, vec![])))
        .collect();
    v.sort_by_key(|a| a.library.name.to_lowercase());
    v
}

/// Saves a library in a folder as "<name>.dschemalib" (replacing `old`, its previous file, when
/// renamed). A different library already saved under the name is not overwritten.
pub fn save(dir: &std::path::Path, lib: Library, old: Option<&str>) -> Result<LibView, String> {
    let (lib, w) = validate(lib);
    let file = format!("{}.dschemalib", slug(&lib.name));
    let path = dir.join(&file);
    if path.exists() && old != Some(file.as_str()) {
        return Err(format!("There is already a library called {}; give this one another name.", lib.name));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not save the library: {e}"))?;
    let text = serde_json::to_string_pretty(&lib).map_err(|e| e.to_string())?;
    crate::doc::write_atomic(&path, text.as_bytes())?;
    if let Some(o) = old.filter(|o| *o != file && !o.contains(['/', '\\'])) {
        let _ = std::fs::remove_file(dir.join(o));
    }
    lib.symbols.iter().for_each(register);
    Ok(view(&file, lib, w))
}

/// A library symbol made to stand for another of our symbols (or none): its class, category,
/// prefix and in-line flag follow, and its ports are suggested afresh from the new one.
pub fn restyle(mut c: CustomSym, like: &str) -> CustomSym {
    let ps = symbols::parse(&c.draw);
    match symbols::builtin(like) {
        Some(l) => {
            c.like = l.key.to_string();
            c.class = crate::classes::class_name_for(l.key);
            c.cat = l.cat;
            c.prefix = l.prefix.to_string();
            c.inline = l.inline;
            c.terminal = l.terminal;
            c.ports = ports_like(l, &ps, c.w, c.h);
        }
        None => {
            c.like.clear();
            c.class = "UnknownItem".into();
            c.inline = false;
        }
    }
    c
}

/// A connection point placed by hand on a symbol: snapped onto its drawing, inside its box, and
/// leaving towards the nearest side.
pub fn port_at(draw: &str, w: f64, h: f64, x: f64, y: f64) -> CPort {
    let ps = symbols::parse(draw);
    let q = snap(&ps, [x, y], 0.12 * w.max(h));
    let (x, y) = (q[0].clamp(-w / 2.0, w / 2.0), q[1].clamp(-h / 2.0, h / 2.0));
    CPort { x, y, dir: side_dir(x, y, w, h), name: String::new(), signal: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_own_legend_sheet_reads_as_a_library() {
        // The pump example's legend sheet, exported as DXF: our symbols as blocks with their names.
        let mut p = crate::examples::get("pump").unwrap();
        let id = p.id();
        p.sheets.push(crate::model::Sheet { id, number: "LEG-1".into(), legend: true, ..Default::default() });
        let s = p.sheets.last().unwrap().clone();
        let scan = scan_legend(&crate::dxf::write(&p, &s), "Client A").unwrap();
        let lib = &scan.library;
        assert!(lib.symbols.len() >= 8, "{}", lib.symbols.len());
        // Most come back as what they are.
        let right = lib.symbols.iter().filter(|c| c.block.trim_start_matches("SYM_").eq_ignore_ascii_case(&c.like)).count();
        assert!(right * 10 >= lib.symbols.len() * 8, "{right} of {}: {:?}", lib.symbols.len(), lib.symbols.iter().map(|c| (&c.block, &c.name, &c.like)).collect::<Vec<_>>());
        let gate = lib.symbols.iter().find(|c| c.like == "gate").unwrap();
        assert_eq!((gate.class.as_str(), gate.inline, gate.ports.len()), ("GateValve", true, 2));
        assert!(gate.key.starts_with("x.client-a."));
        // Ports sit on the drawing's left and right ends.
        assert!(gate.ports[0].x < -gate.w / 2.0 + 0.5 && gate.ports[1].x > gate.w / 2.0 - 0.5, "{:?}", gate.ports);
    }

    #[test]
    fn a_client_legend_by_hand() {
        let scan = scan_legend(crate::dxfin::tests::SAMPLE, "Hand").unwrap();
        let s = &scan.library.symbols[0];
        assert_eq!(s.name, "GATE VALVE FLANGED");
        assert_eq!(s.like, "gate");
        assert!(s.w > 9.0 && s.w < 11.0, "inches to millimetres: {}", s.w);
        assert!(!symbols::parse(&s.draw).is_empty());
    }

    #[test]
    fn suggestions_understand_legend_words() {
        for (desc, want) in [("GATE VALVE", "gate"), ("CENTRIFUGAL PUMP", "pump_cent"), ("CONTROL VALVE PNEUMATIC ACTUATOR", "cv_pneu"), ("PRESSURE SAFETY VALVE", "psv"), ("SHARED DISPLAY DCS", "inst_dcs"), ("Y-TYPE STRAINER", "strainer_y"), ("NRV", "check"), ("KO DRUM", "ko_drum")] {
            assert_eq!(suggest(desc, "").map(|x| x.0), Some(want), "{desc}");
        }
        assert!(suggest("REVISION TRIANGLE", "").is_none_or(|x| x.1 < 0.5));
    }

    #[test]
    fn registered_symbols_draw_and_validate() {
        let (lib, _) = validate(Library { name: "Test lib".into(), symbols: vec![CustomSym { block: "B1".into(), name: "Thing".into(), draw: "c 0 0 4".into(), w: 8.0, h: 8.0, ports: vec![CPort { x: 99.0, y: 0.0, dir: 9, name: String::new(), signal: false }], inline: true, ..Default::default() }], ..Default::default() });
        let s = &lib.symbols[0];
        assert_eq!(s.key, "x.test-lib.b1");
        assert_eq!((s.ports[0].x, s.ports[0].dir, s.ports[0].name.as_str(), s.inline), (4.0, 1, "port", false));
        register(s);
        let r = symbols::get("x.test-lib.b1").unwrap();
        assert_eq!(r.name, "Thing");
        assert!(definition("x.test-lib.b1").is_some());
        assert_eq!(side_dir(4.0, 0.0, 8.0, 8.0), 0);
        assert_eq!(side_dir(0.0, -4.0, 8.0, 8.0), 3);
    }

    #[test]
    fn a_drawing_standard_reskins_and_restores_exactly() {
        // A client library from a legend: every symbol of the pump example, drawn by the client.
        let mut p = crate::examples::get("pump").unwrap();
        let id = p.id();
        p.sheets.push(crate::model::Sheet { id, number: "LEG-1".into(), legend: true, ..Default::default() });
        let s = p.sheets.last().unwrap().clone();
        let lib = scan_legend(&crate::dxf::write(&p, &s), "Client B").unwrap().library;
        lib.symbols.iter().for_each(register);
        let original = crate::examples::get("pump").unwrap();
        let issues = |p: &crate::model::Project| { let net = crate::graph::Net::new(p); let h = crate::hydraulics::all(p, &net); let mut v: Vec<(u8, String)> = crate::checks::run(p, &net, &h).into_iter().map(|i| (i.rule, i.msg)).collect(); v.sort(); v };
        let mut q = original.clone();
        q.settings.standard = crate::model::Standard { library: lib.name.clone(), map: lib.symbols.iter().filter(|c| !c.like.is_empty()).map(|c| (c.like.clone(), c.key.clone())).collect() };
        assert!(q.normalize().is_empty());
        let gates: Vec<_> = q.items.iter().filter(|i| i.base == "gate").collect();
        assert!(!gates.is_empty() && gates.iter().all(|i| i.sym.starts_with("x.client-b.")), "{:?}", q.items.iter().map(|i| (&i.sym, &i.base)).collect::<Vec<_>>());
        // Every line still ends where it did: on a port of the same item.
        let ends = |p: &crate::model::Project| p.lines.iter().map(|l| [&l.a, &l.b].map(|e| match e { crate::model::End::Port { item, .. } => *item, _ => 0 })).collect::<Vec<_>>();
        assert_eq!(ends(&q), ends(&original));
        assert_eq!(issues(&q), issues(&original), "the checks see the same plant");
        assert!(!q.custom_syms.is_empty());
        // Saved and opened again, with the library symbols carried in the file.
        let path = std::env::temp_dir().join(format!("std-{}.dschema", std::process::id()));
        crate::file::write(&path, &q, &[]).unwrap();
        let (back, _, w) = crate::file::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(back.items, q.items);
        assert_eq!(back.custom_syms, q.custom_syms);
        // DXF of the re-skinned sheet: the client's own block names.
        let dxf = crate::dxf::write(&q, &q.sheets[0]);
        let gate_block = lib.symbols.iter().find(|c| c.like == "gate").unwrap().block.to_uppercase();
        assert!(dxf.contains(&format!("\r\n{gate_block}\r\n")), "{gate_block}");
        crate::dxf::read_back(&dxf).unwrap();
        // The standard switched off: our symbols again, exactly as before.
        let mut r = back.clone();
        r.settings.standard = Default::default();
        assert!(r.normalize().is_empty());
        assert_eq!(r.items, original.items);
        assert_eq!(r.lines, original.lines);
        assert!(r.custom_syms.is_empty());
    }

    #[test]
    fn libraries_save_load_and_refuse_a_clash() {
        let dir = std::env::temp_dir().join(format!("datum-libs-{}", std::process::id()));
        let lib = scan_legend(crate::dxfin::tests::SAMPLE, "Saved lib").unwrap().library;
        let v = save(&dir, lib.clone(), None).unwrap();
        assert_eq!(v.file, "saved-lib.dschemalib");
        assert_eq!(v.svgs.len(), v.library.symbols.len());
        assert!(save(&dir, lib.clone(), None).unwrap_err().contains("already"));
        // Saving it again in place, renamed: the old file goes.
        let mut renamed = v.library.clone();
        renamed.name = "Renamed lib".into();
        let r = save(&dir, renamed, Some(&v.file)).unwrap();
        let all = load_dir(&dir);
        assert_eq!(all.iter().map(|l| l.file.as_str()).collect::<Vec<_>>(), [r.file.as_str()]);
        assert!(all[0].library.symbols[0].key.starts_with("x.renamed-lib."));
        assert!(get(&all[0].library.symbols[0].key).is_some());
        let _ = std::fs::remove_dir_all(&dir);
        // A hand-placed port snaps onto the drawing and points out of the nearer side.
        let q = port_at("l -5 0 5 0", 10.0, 4.0, 4.6, 0.3);
        assert_eq!((q.x, q.y, q.dir), (4.6, 0.0, 0));
    }

    #[test]
    fn restyling_a_symbol_suggests_its_ports_again() {
        let c = draft(symbols::parse("l -5 -2 -5 2 5 -2 5 2 -5 -2"), "Thing", "T", None).unwrap();
        assert!(c.ports.is_empty() && c.like.is_empty());
        let g = restyle(c, "gate");
        assert_eq!((g.class.as_str(), g.inline, g.ports.len(), g.prefix.as_str()), ("GateValve", true, 2, "HV"));
        assert!(g.ports[0].x < -4.9 && g.ports[1].x > 4.9);
        assert!(restyle(g, "nothing").ports.len() == 2, "ports stay when it stands for nothing");
    }
}
