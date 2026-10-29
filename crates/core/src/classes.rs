//! The class library: object types as data. Each class has a name (following the DEXPI P&ID data
//! model where a matching class is known), a parent it inherits attributes from, the symbols that
//! draw it, its tag prefix, and its attributes (key, label, quantity for units, type, choices,
//! DEXPI name, required). The built-in library is `classes.json`, compiled into the app; a project
//! adds its own attributes to any class (Options › Classes), saved with the project.

use crate::model::{Project, LINE_CLASS};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Attr {
    pub key: String,
    pub label: String,
    /// The quantity, for units (`units::QUANTITIES`); empty for text.
    pub q: String,
    /// number, text, choice or bool.
    #[serde(rename = "type")]
    pub kind: String,
    pub choices: Vec<String>,
    /// The attribute's DEXPI name, where one is known.
    pub dexpi: String,
    pub required: bool,
    /// The datasheet section it is printed in (empty: the class's own section).
    pub group: String,
    /// The class that defines it (filled when a class's attributes are listed).
    pub from: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Class {
    pub name: String,
    pub label: String,
    pub parent: Option<String>,
    pub symbols: Vec<String>,
    pub prefix: String,
    pub attributes: Vec<Attr>,
    /// Attributes (by key) that must be filled; check 6 reports them.
    pub required: Vec<String>,
    /// A line-number class.
    pub line: bool,
}

/// An attribute a project adds to a class (and so to its subclasses).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct UserAttr {
    pub class: String,
    pub attr: Attr,
}

#[derive(Deserialize)]
struct File {
    classes: Vec<Class>,
}

static LIB: std::sync::OnceLock<Vec<Class>> = std::sync::OnceLock::new();

/// Every built-in class.
pub fn all() -> &'static [Class] {
    LIB.get_or_init(|| serde_json::from_str::<File>(include_str!("../classes.json")).map(|f| f.classes).unwrap_or_default())
}

pub fn get(name: &str) -> Option<&'static Class> {
    all().iter().find(|c| c.name == name)
}

/// The class a symbol draws by default.
pub fn for_symbol(sym: &str) -> Option<&'static Class> {
    all().iter().find(|c| c.symbols.iter().any(|s| s == sym))
}

/// The class and its ancestors, the root first.
pub fn chain(name: &str) -> Vec<&'static Class> {
    let mut v = vec![];
    let mut at = get(name);
    while let Some(c) = at {
        if v.iter().any(|x: &&Class| x.name == c.name) || v.len() > 20 {
            break;
        }
        v.push(c);
        at = c.parent.as_deref().and_then(get);
    }
    v.reverse();
    v
}

/// A class's attributes: inherited ones first, then its own, then the project's additions; a key
/// defined again lower down replaces the one above. `required` is set from the class lists.
pub fn attrs(p: Option<&Project>, class: &str) -> Vec<Attr> {
    let mut out: Vec<Attr> = vec![];
    let chain = chain(class);
    let names: Vec<&str> = if chain.is_empty() { vec![class] } else { chain.iter().map(|c| c.name.as_str()).collect() };
    let put = |a: &Attr, from: &str, out: &mut Vec<Attr>| {
        let mut a = a.clone();
        a.from = from.to_string();
        if a.kind.is_empty() {
            a.kind = if !a.choices.is_empty() { "choice".into() } else if a.q.is_empty() { "text".into() } else { "number".into() };
        }
        match out.iter_mut().find(|x| x.key == a.key) {
            Some(x) => *x = a,
            None => out.push(a),
        }
    };
    for c in &chain {
        for a in &c.attributes {
            put(a, &c.name, &mut out);
        }
    }
    if let Some(p) = p {
        for u in p.settings.user_attrs.iter().filter(|u| names.contains(&u.class.as_str())) {
            put(&u.attr, &u.class, &mut out);
        }
    }
    for c in &chain {
        for k in &c.required {
            if let Some(a) = out.iter_mut().find(|a| a.key == *k) {
                a.required = true;
            }
        }
    }
    out
}

/// The class a new object drawn with this symbol gets.
pub fn class_name_for(sym: &str) -> String {
    for_symbol(sym).map_or_else(|| sym.to_string(), |c| c.name.clone())
}

/// The line-number class.
pub fn line_class() -> &'static Class {
    get(LINE_CLASS).expect("the class library has the line-number class")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_loads_and_covers_every_symbol() {
        assert!(all().len() > 50);
        for s in crate::symbols::all() {
            assert!(for_symbol(s.key).is_some(), "no class draws {}", s.key);
        }
        for c in all() {
            if let Some(p) = &c.parent {
                assert!(get(p).is_some(), "{} has an unknown parent {p}", c.name);
            }
        }
        let names: std::collections::HashSet<&str> = all().iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names.len(), all().len(), "class names are unique");
    }

    #[test]
    fn inheritance_and_project_attributes() {
        let pump = attrs(None, "CentrifugalPump");
        let keys: Vec<&str> = pump.iter().map(|a| a.key.as_str()).collect();
        // Equipment's attributes, then the pump's.
        assert!(keys.contains(&"design_p") && keys.contains(&"head"));
        assert_eq!(pump.iter().find(|a| a.key == "design_p").unwrap().from, "Equipment");
        assert_eq!(pump.iter().find(|a| a.key == "head").unwrap().q, "head");
        let line = attrs(None, LINE_CLASS);
        assert!(line.iter().find(|a| a.key == "size").unwrap().required);
        // A project attribute on Pump reaches every pump class, marked required.
        let mut p = Project::new();
        p.settings.user_attrs.push(UserAttr { class: "Pump".into(), attr: Attr { key: "seal_plan".into(), label: "Seal plan".into(), required: true, ..Default::default() } });
        let a = attrs(Some(&p), "CentrifugalPump");
        let seal = a.iter().find(|a| a.key == "seal_plan").unwrap();
        assert!(seal.required && seal.kind == "text" && seal.from == "Pump");
        assert!(!attrs(Some(&p), "Vessel").iter().any(|a| a.key == "seal_plan"));
    }
}
