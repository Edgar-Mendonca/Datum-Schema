//! A PDF page read as a drawing: its vector paths and its text, the way the DXF reader gives them
//! (`dxfin::Dxf`), so an old P&ID printed or exported to PDF can be put behind a sheet or turned
//! into a live sheet. Objects, object streams and Flate-compressed streams are read; the page's
//! content is interpreted (graphics state, transforms, paths, curves, rectangles, forms, text with
//! its font's ToUnicode map). Images are skipped: a page that is only an image is a scan.
//! Each path's layer says how it was drawn: "PDF SIG" dashed, "PDF MINOR" thin, "PDF" otherwise.

use crate::dxfin::{Dxf, Ent};
use std::collections::HashMap;
use std::io::Read;

const MAX_STREAM: usize = 64 << 20;
const MAX_TOTAL: usize = 256 << 20;
const MAX_OPS: usize = 2_000_000;

#[derive(Clone, Debug, PartialEq)]
enum V {
    Num(f64),
    Name(String),
    Str(Vec<u8>),
    Arr(Vec<V>),
    Dict(Vec<(String, V)>),
    Ref(u32),
    Bool(bool),
    Null,
    Op(String),
}

impl V {
    fn get(&self, k: &str) -> Option<&V> {
        match self {
            V::Dict(d) => d.iter().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }
    fn num(&self) -> Option<f64> {
        match self {
            V::Num(x) => Some(*x),
            _ => None,
        }
    }
    fn name(&self) -> Option<&str> {
        match self {
            V::Name(x) => Some(x),
            _ => None,
        }
    }
}

struct Lex<'a> {
    b: &'a [u8],
    i: usize,
}

fn delim(c: u8) -> bool {
    matches!(c, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

impl<'a> Lex<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c == b'%' {
                while self.i < self.b.len() && self.b[self.i] != b'\n' && self.b[self.i] != b'\r' {
                    self.i += 1;
                }
            } else if c.is_ascii_whitespace() || c == 0 {
                self.i += 1;
            } else {
                break;
            }
        }
    }
    fn word(&mut self) -> String {
        let s = self.i;
        while self.i < self.b.len() && !self.b[self.i].is_ascii_whitespace() && !delim(self.b[self.i]) {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.b[s..self.i]).into_owned()
    }
    /// One value; a number followed by "g R" is a reference.
    fn val(&mut self, depth: usize) -> Option<V> {
        self.ws();
        if self.i >= self.b.len() || depth > 64 {
            return None;
        }
        let c = self.b[self.i];
        match c {
            b'/' => {
                self.i += 1;
                let w = self.word();
                // #xx escapes in names.
                let mut out = String::new();
                let mut it = w.chars().peekable();
                while let Some(ch) = it.next() {
                    if ch == '#' {
                        let h: String = it.by_ref().take(2).collect();
                        if let Ok(x) = u8::from_str_radix(&h, 16) {
                            out.push(x as char);
                        }
                    } else {
                        out.push(ch);
                    }
                }
                Some(V::Name(out))
            }
            b'(' => {
                self.i += 1;
                let mut depth_p = 1;
                let mut s = vec![];
                while self.i < self.b.len() {
                    let c = self.b[self.i];
                    self.i += 1;
                    match c {
                        b'\\' if self.i < self.b.len() => {
                            let e = self.b[self.i];
                            self.i += 1;
                            match e {
                                b'n' => s.push(b'\n'),
                                b'r' => s.push(b'\r'),
                                b't' => s.push(b'\t'),
                                b'b' => s.push(8),
                                b'f' => s.push(12),
                                b'0'..=b'7' => {
                                    let mut v = (e - b'0') as u32;
                                    for _ in 0..2 {
                                        if self.i < self.b.len() && (b'0'..=b'7').contains(&self.b[self.i]) {
                                            v = v * 8 + (self.b[self.i] - b'0') as u32;
                                            self.i += 1;
                                        }
                                    }
                                    s.push(v as u8);
                                }
                                b'\r' | b'\n' => {}
                                x => s.push(x),
                            }
                        }
                        b'(' => {
                            depth_p += 1;
                            s.push(c);
                        }
                        b')' => {
                            depth_p -= 1;
                            if depth_p == 0 {
                                break;
                            }
                            s.push(c);
                        }
                        _ => s.push(c),
                    }
                }
                Some(V::Str(s))
            }
            b'<' if self.b.get(self.i + 1) == Some(&b'<') => {
                self.i += 2;
                let mut d = vec![];
                loop {
                    self.ws();
                    if self.i + 1 < self.b.len() && self.b[self.i] == b'>' && self.b[self.i + 1] == b'>' {
                        self.i += 2;
                        break;
                    }
                    if self.i >= self.b.len() {
                        break;
                    }
                    let Some(V::Name(k)) = self.val(depth + 1) else { break };
                    let v = self.val(depth + 1).unwrap_or(V::Null);
                    d.push((k, v));
                }
                Some(V::Dict(d))
            }
            b'<' => {
                self.i += 1;
                let mut hex = vec![];
                while self.i < self.b.len() && self.b[self.i] != b'>' {
                    if self.b[self.i].is_ascii_hexdigit() {
                        hex.push(self.b[self.i]);
                    }
                    self.i += 1;
                }
                self.i += 1;
                if hex.len() % 2 == 1 {
                    hex.push(b'0');
                }
                Some(V::Str(hex.chunks(2).filter_map(|p| u8::from_str_radix(std::str::from_utf8(p).ok()?, 16).ok()).collect()))
            }
            b'[' => {
                self.i += 1;
                let mut a = vec![];
                loop {
                    self.ws();
                    if self.i >= self.b.len() {
                        break;
                    }
                    if self.b[self.i] == b']' {
                        self.i += 1;
                        break;
                    }
                    match self.val(depth + 1) {
                        Some(v) => a.push(v),
                        None => break,
                    }
                }
                Some(V::Arr(a))
            }
            b']' | b'>' | b')' | b'{' | b'}' => {
                self.i += 1;
                Some(V::Op(String::new()))
            }
            _ => {
                let w = self.word();
                if w.is_empty() {
                    self.i += 1;
                    return Some(V::Op(String::new()));
                }
                if let Ok(x) = w.parse::<f64>() {
                    // "n g R"?
                    let save = self.i;
                    self.ws();
                    let g = self.word();
                    if g.parse::<u32>().is_ok() {
                        self.ws();
                        if self.b.get(self.i) == Some(&b'R') && self.b.get(self.i + 1).is_none_or(|c| c.is_ascii_whitespace() || delim(*c)) {
                            self.i += 1;
                            return Some(V::Ref(x as u32));
                        }
                    }
                    self.i = save;
                    return Some(V::Num(if x.is_finite() { x } else { 0.0 }));
                }
                Some(match w.as_str() {
                    "true" => V::Bool(true),
                    "false" => V::Bool(false),
                    "null" => V::Null,
                    _ => V::Op(w),
                })
            }
        }
    }
}

fn inflate(data: &[u8], budget: &mut usize) -> Option<Vec<u8>> {
    let mut out = vec![];
    let lim = MAX_STREAM.min(*budget) as u64;
    // Some writers leave the stream without its checksum: take what decoded.
    if flate2::read::ZlibDecoder::new(data).take(lim).read_to_end(&mut out).is_err() && out.is_empty() {
        return None;
    }
    *budget = budget.saturating_sub(out.len());
    Some(out)
}

/// The objects of a PDF: dictionaries and values by number, streams decoded.
struct Doc {
    objs: HashMap<u32, (V, Option<Vec<u8>>)>,
}

impl Doc {
    fn read(b: &[u8]) -> Result<Doc, String> {
        if !b.starts_with(b"%PDF") {
            return Err("That is not a PDF file.".into());
        }
        let mut objs: HashMap<u32, (V, Option<Vec<u8>>)> = HashMap::new();
        let mut budget = MAX_TOTAL;
        let mut i = 0;
        // Every "n g obj" in the file.
        while let Some(k) = find(b, b" obj", i) {
            i = k + 4;
            // The two numbers before it.
            let mut j = k;
            let num_end = j;
            while j > 0 && b[j - 1].is_ascii_digit() {
                j -= 1;
            }
            let gen = std::str::from_utf8(&b[j..num_end]).ok().and_then(|s| s.parse::<u32>().ok());
            let mut s = j;
            while s > 0 && b[s - 1] == b' ' {
                s -= 1;
            }
            let ne = s;
            while s > 0 && b[s - 1].is_ascii_digit() {
                s -= 1;
            }
            let (Some(_), Some(n)) = (gen, std::str::from_utf8(&b[s..ne]).ok().and_then(|x| x.parse::<u32>().ok())) else { continue };
            let mut lx = Lex { b, i };
            let Some(v) = lx.val(0) else { continue };
            lx.ws();
            let mut stream = None;
            lx.i = lx.i.min(b.len());
            if b[lx.i..].starts_with(b"stream") {
                let mut st = lx.i + 6;
                if b.get(st) == Some(&b'\r') {
                    st += 1;
                }
                if b.get(st) == Some(&b'\n') {
                    st += 1;
                }
                let len = v.get("Length").and_then(V::num).map(|x| x as usize).filter(|l| st + l <= b.len() && b[st + l..].windows(9).take(12).any(|w| w == b"endstream"));
                let end = len.map(|l| st + l).or_else(|| find(b, b"endstream", st)).unwrap_or(b.len());
                let raw = b.get(st.min(b.len())..end.min(b.len()).max(st.min(b.len()))).unwrap_or(&[]);
                let filters: Vec<String> = match v.get("Filter") {
                    Some(V::Name(f)) => vec![f.clone()],
                    Some(V::Arr(a)) => a.iter().filter_map(|x| x.name().map(String::from)).collect(),
                    _ => vec![],
                };
                stream = if filters.is_empty() {
                    Some(raw.to_vec())
                } else if filters.iter().all(|f| f == "FlateDecode" || f == "Fl") {
                    let mut data = raw.to_vec();
                    let mut ok = true;
                    for _ in &filters {
                        match inflate(&data, &mut budget) {
                            Some(d) => data = d,
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    ok.then_some(data)
                } else {
                    None
                };
                i = end.max(i);
            } else {
                i = lx.i;
            }
            objs.entry(n).or_insert((v, stream));
            if objs.len() > 1_000_000 {
                return Err("That PDF is too large to read.".into());
            }
        }
        // Objects packed in object streams.
        let packed: Vec<(V, Vec<u8>)> = objs.values().filter(|(v, s)| v.get("Type").and_then(V::name) == Some("ObjStm") && s.is_some()).map(|(v, s)| (v.clone(), s.clone().unwrap_or_default())).collect();
        for (v, data) in packed {
            let (n, first) = (v.get("N").and_then(V::num).unwrap_or(0.0) as usize, v.get("First").and_then(V::num).unwrap_or(0.0) as usize);
            let mut lx = Lex { b: &data, i: 0 };
            let mut heads = vec![];
            for _ in 0..n.min(100_000) {
                let (Some(V::Num(on)), Some(V::Num(off))) = (lx.val(0), lx.val(0)) else { break };
                heads.push((on as u32, off as usize));
            }
            for (on, off) in heads {
                let mut l = Lex { b: &data, i: first + off };
                if let Some(val) = l.val(0) {
                    objs.entry(on).or_insert((val, None));
                }
            }
        }
        Ok(Doc { objs })
    }
    fn resolve<'b>(&'b self, v: &'b V) -> &'b V {
        let mut v = v;
        for _ in 0..16 {
            match v {
                V::Ref(n) => match self.objs.get(n) {
                    Some((x, _)) => v = x,
                    None => return &V::Null,
                },
                _ => return v,
            }
        }
        v
    }
    fn stream(&self, v: &V) -> Option<&[u8]> {
        match v {
            V::Ref(n) => self.objs.get(n).and_then(|x| x.1.as_deref()),
            _ => None,
        }
    }
    /// The pages, in order: (page dictionary, inherited resources, media box).
    fn pages(&self) -> Vec<(V, V, [f64; 4])> {
        let mut out = vec![];
        let root = self.objs.values().find(|(v, _)| v.get("Type").and_then(V::name) == Some("Pages") && v.get("Parent").is_none()).map(|x| x.0.clone());
        fn walk(d: &Doc, node: &V, res: &V, mb: [f64; 4], out: &mut Vec<(V, V, [f64; 4])>, depth: usize) {
            if depth > 32 || out.len() > 10_000 {
                return;
            }
            let res = node.get("Resources").map(|r| d.resolve(r).clone()).unwrap_or_else(|| res.clone());
            let mb = node.get("MediaBox").map(|m| d.resolve(m)).and_then(|m| match m {
                V::Arr(a) if a.len() == 4 => Some([0, 1, 2, 3].map(|k| d.resolve(&a[k]).num().unwrap_or(0.0))),
                _ => None,
            }).unwrap_or(mb);
            match node.get("Type").and_then(V::name) {
                Some("Page") => out.push((node.clone(), res, mb)),
                _ => {
                    if let Some(V::Arr(kids)) = node.get("Kids").map(|k| d.resolve(k)) {
                        for k in kids {
                            let kid = d.resolve(k).clone();
                            walk(d, &kid, &res, mb, out, depth + 1);
                        }
                    }
                }
            }
        }
        match root {
            Some(r) => walk(self, &r, &V::Null, [0.0, 0.0, 612.0, 792.0], &mut out, 0),
            None => {
                // No page tree found: every page object, in object order.
                let mut ps: Vec<(&u32, &V)> = self.objs.iter().filter(|(_, (v, _))| v.get("Type").and_then(V::name) == Some("Page")).map(|(n, (v, _))| (n, v)).collect();
                ps.sort_by_key(|x| *x.0);
                for (_, v) in ps {
                    walk(self, v, &V::Null, [0.0, 0.0, 612.0, 792.0], &mut out, 0);
                }
            }
        }
        out
    }
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

type M = [f64; 6];
const ID: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// a × b (PDF row-vector convention: a point is transformed by a, then by b).
fn mul(a: &M, b: &M) -> M {
    [a[0] * b[0] + a[1] * b[2], a[0] * b[1] + a[1] * b[3], a[2] * b[0] + a[3] * b[2], a[2] * b[1] + a[3] * b[3], a[4] * b[0] + a[5] * b[2] + b[4], a[4] * b[1] + a[5] * b[3] + b[5]]
}

fn ap(m: &M, p: [f64; 2]) -> [f64; 2] {
    [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]]
}

fn scale(m: &M) -> f64 {
    (m[0] * m[3] - m[1] * m[2]).abs().sqrt()
}

/// A font's ToUnicode map: code (as bytes) → text, and the code lengths it uses.
#[derive(Default, Clone)]
struct CMap {
    map: HashMap<Vec<u8>, String>,
    two: bool,
}

fn utf16(b: &[u8]) -> String {
    let u: Vec<u16> = b.chunks(2).map(|c| ((c[0] as u16) << 8) | *c.get(1).unwrap_or(&0) as u16).collect();
    String::from_utf16_lossy(&u)
}

fn cmap(data: &[u8]) -> CMap {
    let mut cm = CMap::default();
    let mut lx = Lex { b: data, i: 0 };
    let mut toks: Vec<V> = vec![];
    while let Some(v) = lx.val(0) {
        toks.push(v);
        if toks.len() > 400_000 {
            break;
        }
    }
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            V::Op(o) if o == "beginbfchar" => {
                i += 1;
                while i + 1 < toks.len() && toks[i] != V::Op("endbfchar".into()) {
                    if let (V::Str(c), V::Str(u)) = (&toks[i], &toks[i + 1]) {
                        cm.two |= c.len() == 2;
                        cm.map.insert(c.clone(), utf16(u));
                    }
                    i += 2;
                }
            }
            V::Op(o) if o == "beginbfrange" => {
                i += 1;
                while i + 2 < toks.len() && toks[i] != V::Op("endbfrange".into()) {
                    if let (V::Str(a), V::Str(z)) = (&toks[i], &toks[i + 1]) {
                        cm.two |= a.len() == 2;
                        let code = |b: &[u8]| b.iter().fold(0u32, |x, y| (x << 8) | *y as u32);
                        let (lo, hi) = (code(a), code(z));
                        let n = a.len();
                        if hi >= lo && hi - lo < 65_536 {
                            match &toks[i + 2] {
                                V::Str(u) => {
                                    let base: Vec<u16> = u.chunks(2).map(|c| ((c[0] as u16) << 8) | *c.get(1).unwrap_or(&0) as u16).collect();
                                    for (k, c) in (lo..=hi).enumerate() {
                                        let mut s = base.clone();
                                        if let Some(l) = s.last_mut() {
                                            *l = l.wrapping_add(k as u16);
                                        }
                                        cm.map.insert(c.to_be_bytes()[4 - n..].to_vec(), String::from_utf16_lossy(&s));
                                    }
                                }
                                V::Arr(arr) => {
                                    for (k, c) in (lo..=hi).enumerate() {
                                        if let Some(V::Str(u)) = arr.get(k) {
                                            cm.map.insert(c.to_be_bytes()[4 - n..].to_vec(), utf16(u));
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    i += 3;
                }
            }
            _ => {}
        }
        i += 1;
    }
    cm
}

struct State {
    ctm: M,
    lw: f64,
    dash: bool,
    font: Option<CMap>,
    size: f64,
}

struct Run<'a> {
    d: &'a Doc,
    ents: Vec<Ent>,
    layers: Vec<String>,
    ops: usize,
    images: usize,
    fonts: HashMap<String, Option<CMap>>,
}

impl Run<'_> {
    fn font(&mut self, res: &V, name: &str) -> Option<CMap> {
        let key = format!("{:p}/{name}", res as *const V);
        if let Some(f) = self.fonts.get(&key) {
            return f.clone();
        }
        let f = res.get("Font").map(|x| self.d.resolve(x)).and_then(|fs| fs.get(name)).and_then(|r| {
            let fd = self.d.resolve(r);
            let tu = fd.get("ToUnicode")?;
            self.d.stream(tu).map(cmap)
        });
        self.fonts.insert(key, f.clone());
        f
    }

    fn text(&self, s: &[u8], f: &Option<CMap>) -> String {
        match f {
            Some(cm) if !cm.map.is_empty() => {
                let step = if cm.two { 2 } else { 1 };
                s.chunks(step).map(|c| cm.map.get(c).cloned().unwrap_or_default()).collect()
            }
            // A simple font: Latin-1 is close enough for tags and numbers.
            _ => s.iter().map(|&c| if c >= 32 { c as char } else { ' ' }).collect(),
        }
    }

    fn content(&mut self, data: &[u8], res: &V, st0: State, depth: usize) {
        if depth > 12 {
            return;
        }
        let mut stack: Vec<State> = vec![];
        let mut st = st0;
        let mut path: Vec<(Vec<[f64; 2]>, bool)> = vec![];
        let mut cur = [0.0, 0.0];
        let mut start = [0.0, 0.0];
        let (mut tm, mut tlm, mut leading) = (ID, ID, 0.0);
        let mut args: Vec<V> = vec![];
        let mut lx = Lex { b: data, i: 0 };
        let n = |a: &[V], k: usize| a.get(k).and_then(V::num).unwrap_or(0.0);
        while let Some(v) = lx.val(0) {
            let V::Op(op) = v else {
                args.push(v);
                continue;
            };
            self.ops += 1;
            if self.ops > MAX_OPS {
                break;
            }
            let a = std::mem::take(&mut args);
            match op.as_str() {
                "q" => stack.push(State { ctm: st.ctm, lw: st.lw, dash: st.dash, font: st.font.clone(), size: st.size }),
                "Q" => {
                    if let Some(s) = stack.pop() {
                        st = s;
                    }
                }
                "cm" if a.len() >= 6 => st.ctm = mul(&[n(&a, 0), n(&a, 1), n(&a, 2), n(&a, 3), n(&a, 4), n(&a, 5)], &st.ctm),
                "w" => st.lw = n(&a, 0).abs(),
                "d" => st.dash = matches!(a.first(), Some(V::Arr(x)) if x.iter().any(|y| y.num().unwrap_or(0.0) > 0.0)),
                "m" => {
                    cur = [n(&a, 0), n(&a, 1)];
                    start = cur;
                    path.push((vec![cur], false));
                }
                "l" => {
                    cur = [n(&a, 0), n(&a, 1)];
                    match path.last_mut() {
                        Some(p) => p.0.push(cur),
                        None => path.push((vec![cur], false)),
                    }
                }
                "c" | "v" | "y" => {
                    let (p1, p2, p3) = match op.as_str() {
                        "c" => ([n(&a, 0), n(&a, 1)], [n(&a, 2), n(&a, 3)], [n(&a, 4), n(&a, 5)]),
                        "v" => (cur, [n(&a, 0), n(&a, 1)], [n(&a, 2), n(&a, 3)]),
                        _ => ([n(&a, 0), n(&a, 1)], [n(&a, 2), n(&a, 3)], [n(&a, 2), n(&a, 3)]),
                    };
                    let p0 = cur;
                    let pts: Vec<[f64; 2]> = (1..=6).map(|k| {
                        let t = k as f64 / 6.0;
                        let u = 1.0 - t;
                        [u * u * u * p0[0] + 3.0 * u * u * t * p1[0] + 3.0 * u * t * t * p2[0] + t * t * t * p3[0], u * u * u * p0[1] + 3.0 * u * u * t * p1[1] + 3.0 * u * t * t * p2[1] + t * t * t * p3[1]]
                    }).collect();
                    match path.last_mut() {
                        Some(p) => p.0.extend(pts),
                        None => path.push((pts, false)),
                    }
                    cur = p3;
                }
                "h" => {
                    if let Some(p) = path.last_mut() {
                        p.1 = true;
                    }
                    cur = start;
                }
                "re" => {
                    let (x, y, w, h) = (n(&a, 0), n(&a, 1), n(&a, 2), n(&a, 3));
                    path.push((vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]], true));
                    cur = [x, y];
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    let fill = op.starts_with(['f', 'F', 'B', 'b']);
                    let close = matches!(op.as_str(), "s" | "b" | "b*") || fill;
                    let wmm = st.lw * scale(&st.ctm) * 25.4 / 72.0;
                    let layer = if fill && !op.starts_with(['B', 'b']) { "PDF FILL" } else if st.dash { "PDF SIG" } else if wmm <= 0.3 { "PDF MINOR" } else { "PDF" };
                    for (pts, closed) in path.drain(..) {
                        if pts.len() < 2 {
                            continue;
                        }
                        let pts: Vec<[f64; 2]> = pts.into_iter().map(|p| ap(&st.ctm, p)).collect();
                        self.ents.push(Ent::Poly { pts, closed: closed || close, fill: fill && op.starts_with(['f', 'F']) });
                        self.layers.push(layer.into());
                    }
                }
                "n" => path.clear(),
                "BT" => {
                    tm = ID;
                    tlm = ID;
                }
                "Tf" => {
                    st.size = n(&a, 1);
                    let name = a.first().and_then(V::name).unwrap_or("").to_string();
                    st.font = self.font(res, &name);
                }
                "TL" => leading = n(&a, 0),
                "Td" | "TD" => {
                    if op == "TD" {
                        leading = -n(&a, 1);
                    }
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, n(&a, 0), n(&a, 1)], &tlm);
                    tm = tlm;
                }
                "Tm" if a.len() >= 6 => {
                    tlm = [n(&a, 0), n(&a, 1), n(&a, 2), n(&a, 3), n(&a, 4), n(&a, 5)];
                    tm = tlm;
                }
                "T*" => {
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -leading], &tlm);
                    tm = tlm;
                }
                "Tj" | "TJ" | "'" | "\"" => {
                    if op == "'" || op == "\"" {
                        tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -leading], &tlm);
                        tm = tlm;
                    }
                    let mut s = String::new();
                    for x in &a {
                        match x {
                            V::Str(b) => s.push_str(&self.text(b, &st.font)),
                            V::Arr(parts) => {
                                for y in parts {
                                    match y {
                                        V::Str(b) => s.push_str(&self.text(b, &st.font)),
                                        V::Num(k) if *k < -200.0 => s.push(' '),
                                        _ => {}
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    let s = s.trim().to_string();
                    if !s.is_empty() {
                        let m = mul(&tm, &st.ctm);
                        let h = (st.size * scale(&m)).abs();
                        self.ents.push(Ent::Text { p: ap(&m, [0.0, 0.0]), h: if h > 0.0 { h } else { 7.0 }, s: s.clone(), rot: m[1].atan2(m[0]).to_degrees(), anchor: 0 });
                        self.layers.push("PDF TEXT".into());
                        // Move along the line by about the text's width.
                        tm = mul(&[1.0, 0.0, 0.0, 1.0, 0.5 * st.size * s.chars().count() as f64, 0.0], &tm);
                    }
                }
                "Do" => {
                    let Some(name) = a.first().and_then(V::name) else { continue };
                    let Some(xr) = res.get("XObject").map(|x| self.d.resolve(x)).and_then(|x| x.get(name)).cloned() else { continue };
                    let xo = self.d.resolve(&xr).clone();
                    match xo.get("Subtype").and_then(V::name) {
                        Some("Image") => self.images += 1,
                        Some("Form") => {
                            let Some(data) = self.d.stream(&xr).map(|s| s.to_vec()) else { continue };
                            let m = match xo.get("Matrix").map(|x| self.d.resolve(x)) {
                                Some(V::Arr(x)) if x.len() == 6 => [0, 1, 2, 3, 4, 5].map(|k| x[k].num().unwrap_or(0.0)),
                                _ => ID,
                            };
                            let r = xo.get("Resources").map(|x| self.d.resolve(x).clone()).unwrap_or_else(|| res.clone());
                            let inner = State { ctm: mul(&m, &st.ctm), lw: st.lw, dash: st.dash, font: st.font.clone(), size: st.size };
                            self.content(&data, &r, inner, depth + 1);
                        }
                        _ => {}
                    }
                }
                "BI" => {
                    // An inline image: skip to its end.
                    self.images += 1;
                    if let Some(k) = find(data, b"EI", lx.i) {
                        lx.i = k + 2;
                    }
                }
                _ => {}
            }
        }
    }
}

/// How many pages a PDF has.
pub fn page_count(bytes: &[u8]) -> Result<usize, String> {
    Ok(Doc::read(bytes)?.pages().len())
}

/// One page of a PDF (from 0) as a drawing, in points (1/72 in) with y up, like a DXF's model
/// space. Warnings say what was skipped.
pub fn read(bytes: &[u8], page: usize) -> Result<(Dxf, Vec<String>), String> {
    let d = Doc::read(bytes)?;
    let pages = d.pages();
    if pages.is_empty() {
        return Err("That PDF has no pages Datum Schema can read.".into());
    }
    let (pg, res, _) = pages.get(page).ok_or_else(|| format!("That PDF has {} pages.", pages.len()))?;
    let mut data: Vec<u8> = vec![];
    match pg.get("Contents") {
        Some(V::Arr(a)) => {
            for x in a {
                if let Some(s) = d.stream(x) {
                    data.extend_from_slice(s);
                    data.push(b'\n');
                }
            }
        }
        Some(r) => {
            if let Some(V::Arr(a)) = Some(d.resolve(r)).filter(|x| matches!(x, V::Arr(_))) {
                for x in a {
                    if let Some(s) = d.stream(x) {
                        data.extend_from_slice(s);
                        data.push(b'\n');
                    }
                }
            } else if let Some(s) = d.stream(r) {
                data.extend_from_slice(s);
            }
        }
        None => {}
    }
    let mut run = Run { d: &d, ents: vec![], layers: vec![], ops: 0, images: 0, fonts: HashMap::new() };
    run.content(&data, res, State { ctm: ID, lw: 1.0, dash: false, font: None, size: 10.0 }, 0);
    let mut warnings = vec![];
    let paths = run.ents.iter().filter(|e| matches!(e, Ent::Poly { .. })).count();
    if paths == 0 && run.images > 0 {
        return Err("That PDF page is a scanned image, not a drawing: save the page as PNG or JPEG and put it behind a sheet (Sheets › Old Drawing) to trace over.".into());
    }
    if paths == 0 {
        return Err("That PDF page has no lines Datum Schema can read.".into());
    }
    if run.images > 0 {
        warnings.push(format!("{} images on the page were skipped.", run.images));
    }
    if run.ops > MAX_OPS {
        warnings.push("The page is very large; only part of it was read.".into());
    }
    if pages.len() > 1 {
        warnings.push(format!("The PDF has {} pages; page {} was read.", pages.len(), page + 1));
    }
    Ok((Dxf { mm: 25.4 / 72.0, blocks: HashMap::new(), ents: run.ents, layers: run.layers, skipped: Default::default() }, warnings))
}

/// A small PDF by hand (for tests and the test server): a page whose content draws a thick line,
/// a dashed line, a circle of four curves with "FIC" and "101" inside, a bow tie of two
/// triangles, and a text in a font with a ToUnicode map; `flate` compresses the content stream.
#[doc(hidden)]
pub fn sample(flate: bool) -> Vec<u8> {
    // 1 mm = 72/25.4 pt; the drawing is laid out in points.
    let k = 0.5523;
    let (cx, cy, r) = (300.0, 400.0, 17.0);
    let circle = format!(
        "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h S",
        cx + r, cy,
        cx + r, cy + k * r, cx + k * r, cy + r, cx, cy + r,
        cx - k * r, cy + r, cx - r, cy + k * r, cx - r, cy,
        cx - r, cy - k * r, cx - k * r, cy - r, cx, cy - r,
        cx + k * r, cy - r, cx + r, cy - k * r, cx + r, cy
    );
    let content = format!(
        "q 2 w 100 200 m 280 200 l S 312 200 m 500 200 l S Q\n\
         q 0.7 w [3 2] 0 d 300 383 m 300 220 l S Q\n\
         q 1 w {circle} Q\n\
         BT /F1 8 Tf 1 0 0 1 292 402 Tm (FIC) Tj ET\n\
         BT /F1 8 Tf 1 0 0 1 292 390 Tm (101) Tj ET\n\
         q 1 w 280 192 m 280 208 l 296 200 l h S 312 192 m 312 208 l 296 200 l h S Q\n\
         BT /F2 9 Tf 120 210 Td <0001> Tj ET\n"
    );
    let (stream, filter) = if flate {
        use std::io::Write;
        let mut e = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
        e.write_all(content.as_bytes()).unwrap();
        (e.finish().unwrap(), " /Filter /FlateDecode")
    } else {
        (content.into_bytes(), "")
    };
    let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap 1 begincodespacerange <0000> <FFFF> endcodespacerange 1 beginbfrange <0001> <0001> <0050002D0031> endbfrange endcmap end end";
    let mut b: Vec<u8> = b"%PDF-1.7\n".to_vec();
    let mut obj = |n: u32, body: &[u8]| {
        b.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
        b.extend_from_slice(body);
        b.extend_from_slice(b"\nendobj\n");
    };
    obj(1, b"<< /Type /Catalog /Pages 2 0 R >>");
    obj(2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 842 595] >>");
    obj(3, b"<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> /Contents 4 0 R >>");
    let mut s4 = format!("<< /Length {}{filter} >>\nstream\n", stream.len()).into_bytes();
    s4.extend_from_slice(&stream);
    s4.extend_from_slice(b"\nendstream");
    obj(4, &s4);
    obj(5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
    obj(6, b"<< /Type /Font /Subtype /Type0 /BaseFont /X /Encoding /Identity-H /ToUnicode 7 0 R >>");
    let mut s7 = format!("<< /Length {} >>\nstream\n", cmap.len()).into_bytes();
    s7.extend_from_slice(cmap.as_bytes());
    s7.extend_from_slice(b"\nendstream");
    obj(7, &s7);
    b.extend_from_slice(b"trailer << /Root 1 0 R >>\n%%EOF\n");
    b
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample(flate: bool) -> Vec<u8> {
        super::sample(flate)
    }

    #[test]
    fn a_hand_written_pdf_reads_as_a_drawing() {
        for flate in [false, true] {
            let (d, w) = read(&sample(flate), 0).unwrap();
            assert!(w.is_empty(), "{w:?}");
            assert!((d.mm - 25.4 / 72.0).abs() < 1e-12);
            let texts: Vec<String> = d.ents.iter().filter_map(|e| match e { Ent::Text { s, .. } => Some(s.clone()), _ => None }).collect();
            assert!(texts.contains(&"FIC".to_string()) && texts.contains(&"101".to_string()), "{texts:?}");
            assert!(texts.contains(&"P-1".to_string()), "the ToUnicode map: {texts:?}");
            // Lines with their layers: thick, dashed; the circle as a closed polyline.
            let layer = |i: usize| d.layers[i].as_str();
            let polys: Vec<(usize, &Vec<[f64; 2]>, bool)> = d.ents.iter().enumerate().filter_map(|(i, e)| match e { Ent::Poly { pts, closed, .. } => Some((i, pts, *closed)), _ => None }).collect();
            assert!(polys.iter().any(|(i, p, _)| layer(*i) == "PDF" && p.len() == 2 && (p[0][0] - 100.0).abs() < 1e-9));
            assert!(polys.iter().any(|(i, _, _)| layer(*i) == "PDF SIG"));
            assert!(polys.iter().any(|(_, p, c)| *c && p.len() >= 20), "the circle");
            assert_eq!(d.layers.len(), d.ents.len());
        }
        assert_eq!(page_count(&sample(true)).unwrap(), 1);
        assert!(read(b"not a pdf", 0).is_err());
        assert!(read(&sample(true), 3).is_err());
    }

    #[test]
    fn hostile_pdfs_never_panic() {
        let good = sample(true);
        for cut in [10, 50, 200, 400, good.len() / 2, good.len() - 20] {
            let _ = read(&good[..cut], 0);
        }
        // Bytes changed at random, in the plain and the compressed file.
        let mut seed = 0x2545_f491_u64;
        for base in [sample(false), good.clone()] {
            for _ in 0..300 {
                let mut b = base.clone();
                for _ in 0..4 {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    let i = (seed >> 33) as usize % b.len();
                    b[i] = (seed >> 13) as u8;
                }
                let _ = read(&b, 0);
            }
        }
        for bad in [&b"%PDF-1.4\n1 0 obj << /Type /Pages /Kids [1 0 R] >> endobj"[..], b"%PDF 9999999999 0 obj << /Length 999999999999 >> stream", b"%PDF-1.4\n((((((((((((((((((((((((", b"%PDF-1.4\n1 0 obj <<<<<<<<<<<<<<<<<<<<<<< endobj"] {
            let _ = read(bad, 0);
        }
    }
}
