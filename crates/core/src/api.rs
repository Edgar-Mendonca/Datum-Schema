//! What the shell's commands return, built here so the desktop shell and the test server
//! (`examples/serve.rs`) give the window exactly the same answers.

use crate::graph::Net;
use crate::model::{self, Project};
use crate::{checks, classes, hydraulics, pipes, symbols, view};
use serde::Serialize;
use std::collections::HashMap;

/// Everything the window shows that the engine works out from the project: the open sheet's
/// drawing, the checks, each run's hydraulics, the runs and the line numbers.
#[derive(Serialize)]
pub struct Derived {
    pub view: Option<view::SheetView>,
    pub issues: Vec<checks::Issue>,
    pub hyd: Vec<Option<hydraulics::Hyd>>,
    pub runs: Vec<Vec<u32>>,
    pub numbers: HashMap<u32, String>,
    /// The attributes of every class the project uses, with the project's own additions.
    pub attrs: HashMap<String, Vec<classes::Attr>>,
}

pub fn derived(p: &Project, sheet: u32) -> Derived {
    let net = Net::new(p);
    let hyd = hydraulics::all(p, &net);
    let view = p.sheet(sheet).or(p.sheets.first()).map(|s| view::sheet(p, s, &net, &hyd));
    let issues = checks::run(p, &net, &hyd);
    let mut attrs = HashMap::new();
    for c in p.objects.iter().map(|o| o.class.as_str()).chain([model::LINE_CLASS]) {
        if !attrs.contains_key(c) {
            attrs.insert(c.to_string(), classes::attrs(Some(p), c));
        }
    }
    Derived { view, issues, hyd, runs: net.runs.clone(), numbers: net.numbers.clone(), attrs }
}

#[derive(Serialize)]
pub struct ClassInfo {
    pub name: String,
    pub label: String,
    pub parent: Option<String>,
    pub symbols: Vec<String>,
    pub attrs: Vec<classes::Attr>,
}

#[derive(Serialize)]
pub struct Library {
    pub symbols: Vec<symbols::SymInfo>,
    pub kinds: Vec<(model::Kind, &'static str)>,
    pub line_fields: Vec<classes::Attr>,
    /// Every built-in class with its inherited attributes (Options › Classes).
    pub classes: Vec<ClassInfo>,
    pub sizes: Vec<pipes::Size>,
    pub sizes_source: &'static str,
    pub limits: Vec<model::Limit>,
}

/// The symbol library, line types, the fields of every kind of thing, and the pipe sizes.
pub fn library() -> Library {
    Library { symbols: symbols::info(), kinds: model::Kind::ALL.iter().map(|k| (*k, k.name())).collect(), line_fields: classes::attrs(None, model::LINE_CLASS), classes: classes::all().iter().map(|c| ClassInfo { name: c.name.clone(), label: c.label.clone(), parent: c.parent.clone(), symbols: c.symbols.clone(), attrs: classes::attrs(None, &c.name) }).collect(), sizes: pipes::sizes(), sizes_source: pipes::SIZE_SOURCE, limits: model::default_limits() }
}
