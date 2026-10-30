//! Datum Schema engine: the P&ID model and its edits, the symbol library, connectivity, tags,
//! checks, line hydraulics, the lists, the sheet frame, routing and the DXF writer. Pure Rust, no
//! I/O apart from reading and writing files the shell names.

pub mod api;
pub mod checks;
pub mod classes;
pub mod datasheets;
pub mod doc;
pub mod dxf;
pub mod examples;
pub mod file;
pub mod frame;
pub mod graph;
pub mod hydraulics;
pub mod import;
pub mod lists;
pub mod loops;
pub mod model;
pub mod pipes;
pub mod route;
pub mod symbols;
pub mod tags;
pub mod units;
pub mod view;

#[cfg(test)]
mod hardening;

pub use model::{End, Item, Kind, Line, Note, Obj, Project, Sheet, Val};

/// A number for people: no trailing zeros, at most 6 decimals.
pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "—".into();
    }
    let s = format!("{:.6}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// A number to `sig` significant digits, no trailing zeros, no exponent for everyday sizes.
pub fn fmt_sig(v: f64, sig: i32) -> String {
    if !v.is_finite() {
        return "—".into();
    }
    if v == 0.0 {
        return "0".into();
    }
    let mag = v.abs().log10().floor() as i32;
    if !(-4..9).contains(&mag) {
        return format!("{:.*e}", (sig - 1).max(0) as usize, v);
    }
    let dec = (sig - 1 - mag).max(0) as usize;
    fmt_num(format!("{:.*}", dec, v).parse().unwrap_or(v))
}

/// At most `n` characters (on a character boundary).
pub fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Text made safe inside XML or SVG.
pub fn xml(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    #[test]
    fn numbers_and_text() {
        assert_eq!(super::fmt_num(1.5), "1.5");
        assert_eq!(super::fmt_num(-0.0000001), "0");
        assert_eq!(super::fmt_sig(1.787243, 4), "1.787");
        assert_eq!(super::fmt_sig(274863.3, 4), "274863");
        assert_eq!(super::xml("<a href=\"x\">&</a>"), "&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;");
        assert_eq!(super::clip("ab€cd", 3), "ab€");
    }
}
