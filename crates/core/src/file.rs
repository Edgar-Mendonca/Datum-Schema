//! The project file (`.dschema`): one SQLite database. Plant objects and their attribute values are
//! tables of their own, so the file can be queried by any SQLite tool; the views (sheets, symbols,
//! line pieces, notes) are rows with their geometry; issued datasheet revisions keep their snapshot;
//! every change to an object is a row of the history. Files from 0.1.0–0.1.4 (JSON) open and
//! convert.
//!
//! Writing is atomic: the database is built in a temporary file next to the target, which then
//! replaces it in one step. Reading takes each table on its own, so a damaged one is reported and
//! left out instead of the whole file failing.

use crate::doc::{read_limited, Change, MAX_FILE};
use crate::model::{self, Data, Issue, Item, Line, Node, Note, Obj, Project, Sheet, Val};
use rusqlite::{params, Connection, OpenFlags};
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE nodes (id INTEGER PRIMARY KEY, parent INTEGER NOT NULL, level TEXT NOT NULL, name TEXT NOT NULL);
CREATE TABLE objects (id INTEGER PRIMARY KEY, uid TEXT NOT NULL, class TEXT NOT NULL, tag TEXT NOT NULL, node INTEGER NOT NULL);
CREATE TABLE attributes (object INTEGER NOT NULL, name TEXT NOT NULL, number REAL, text TEXT, PRIMARY KEY (object, name));
CREATE TABLE sheets (id INTEGER PRIMARY KEY, position INTEGER NOT NULL, sheet TEXT NOT NULL);
CREATE TABLE items (id INTEGER PRIMARY KEY, sheet INTEGER NOT NULL, object INTEGER NOT NULL, symbol TEXT NOT NULL, x REAL NOT NULL, y REAL NOT NULL, rot INTEGER NOT NULL, flip INTEGER NOT NULL, scale REAL NOT NULL, link INTEGER NOT NULL, base TEXT NOT NULL DEFAULT '');
CREATE TABLE lines (id INTEGER PRIMARY KEY, sheet INTEGER NOT NULL, object INTEGER NOT NULL, line TEXT NOT NULL);
CREATE TABLE underlays (sheet INTEGER PRIMARY KEY, draw TEXT NOT NULL);
CREATE TABLE notes (id INTEGER PRIMARY KEY, sheet INTEGER NOT NULL, note TEXT NOT NULL);
CREATE TABLE issues (seq INTEGER PRIMARY KEY, object INTEGER NOT NULL, rev TEXT NOT NULL, status TEXT NOT NULL, date TEXT NOT NULL, issued_by TEXT NOT NULL, note TEXT NOT NULL, snapshot TEXT NOT NULL);
CREATE TABLE history (seq INTEGER PRIMARY KEY, time INTEGER NOT NULL, user TEXT NOT NULL, what TEXT NOT NULL, object INTEGER NOT NULL, uid TEXT NOT NULL, name TEXT NOT NULL, old TEXT NOT NULL, new TEXT NOT NULL);
CREATE VIEW object_attributes AS SELECT o.tag, o.class, a.name, coalesce(a.number, a.text) AS value FROM objects o JOIN attributes a ON a.object = o.id;
";

fn sql(e: rusqlite::Error) -> String {
    format!("The project file could not be written or read: {e}")
}

/// Writes the project and its history to `path` as a SQLite database, atomically.
pub fn write(path: &Path, p: &Project, history: &[Change]) -> Result<(), String> {
    let tmp = path.with_extension("dschema.saving");
    let _ = std::fs::remove_file(&tmp);
    let res = (|| -> Result<(), String> {
        let mut c = Connection::open(&tmp).map_err(sql)?;
        c.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA application_id = 1146308451;").map_err(sql)?;
        c.execute_batch(SCHEMA).map_err(sql)?;
        let t = c.transaction().map_err(sql)?;
        let js = |v: &dyn erased::Ser| v.json();
        for (k, v) in [("format", model::FORMAT.to_string()), ("app", format!("Datum Schema {}", env!("CARGO_PKG_VERSION"))), ("name", p.name.clone()), ("units", p.units.clone()), ("next", p.next.to_string()), ("title", js(&p.title)), ("settings", js(&p.settings)), ("symbols", js(&p.custom_syms))] {
            t.execute("INSERT INTO meta VALUES (?1, ?2)", params![k, v]).map_err(sql)?;
        }
        for n in &p.nodes {
            t.execute("INSERT INTO nodes VALUES (?1, ?2, ?3, ?4)", params![n.id, n.parent, n.level, n.name]).map_err(sql)?;
        }
        for o in &p.objects {
            t.execute("INSERT INTO objects VALUES (?1, ?2, ?3, ?4, ?5)", params![o.id, o.uid, o.class, o.tag, o.node]).map_err(sql)?;
            for (k, v) in &o.data {
                let (num, text) = match v { Val::Num(x) => (Some(*x), None), Val::Text(s) => (None, Some(s.as_str())) };
                t.execute("INSERT INTO attributes VALUES (?1, ?2, ?3, ?4)", params![o.id, k, num, text]).map_err(sql)?;
            }
        }
        for (i, s) in p.sheets.iter().enumerate() {
            t.execute("INSERT INTO sheets VALUES (?1, ?2, ?3)", params![s.id, i as i64, js(s)]).map_err(sql)?;
            if let Some(u) = &s.underlay {
                t.execute("INSERT INTO underlays VALUES (?1, ?2)", params![s.id, u.draw.as_str()]).map_err(sql)?;
            }
        }
        for it in &p.items {
            t.execute("INSERT INTO items VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)", params![it.id, it.sheet, it.obj, it.sym, it.x, it.y, it.rot, it.flip, it.scale, it.link, it.base]).map_err(sql)?;
        }
        for l in &p.lines {
            t.execute("INSERT INTO lines VALUES (?1, ?2, ?3, ?4)", params![l.id, l.sheet, l.obj, js(l)]).map_err(sql)?;
        }
        for n in &p.notes {
            t.execute("INSERT INTO notes VALUES (?1, ?2, ?3)", params![n.id, n.sheet, js(n)]).map_err(sql)?;
        }
        for (i, s) in p.issues.iter().enumerate() {
            t.execute("INSERT INTO issues VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![i as i64, s.obj, s.rev, s.status, s.date, s.by, s.note, js(&s.values)]).map_err(sql)?;
        }
        for h in history {
            t.execute("INSERT INTO history VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)", params![h.seq as i64, h.time as i64, h.user, h.what, h.obj, h.uid, h.key, h.old, h.new]).map_err(sql)?;
        }
        t.commit().map_err(sql)?;
        c.close().map_err(|(_, e)| sql(e))?;
        // Make sure the bytes are on disk before the rename.
        // (Windows refuses to flush a read-only handle, so open it for writing.)
        std::fs::OpenOptions::new().write(true).open(&tmp).and_then(|f| f.sync_all()).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
        Ok(())
    })();
    if let Err(e) = res {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not save {}: {e}", path.display())
    })
}

/// Serialising any value to JSON text without naming its type (for the JSON columns).
mod erased {
    pub trait Ser {
        fn json(&self) -> String;
    }
    impl<T: serde::Serialize> Ser for T {
        fn json(&self) -> String {
            serde_json::to_string(self).unwrap_or_default()
        }
    }
}

/// Reads a project file: SQLite (0.1.5 on), or JSON (0.1.0–0.1.4, and recovery copies).
pub fn read(path: &Path) -> Result<(Project, Vec<Change>, Vec<String>), String> {
    let len = std::fs::metadata(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?.len();
    if len > MAX_FILE {
        return Err(format!("{} is {} MB; Datum Schema opens files up to {} MB.", path.display(), len / 1_048_576, MAX_FILE / 1_048_576));
    }
    let mut head = [0u8; 16];
    let n = std::io::Read::read(&mut std::fs::File::open(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?, &mut head).unwrap_or(0);
    if n == 16 && &head == b"SQLite format 3\0" {
        return read_sqlite(path);
    }
    let (p, w) = model::from_json(&read_limited(path)?)?;
    Ok((p, vec![], w))
}

fn read_sqlite(path: &Path) -> Result<(Project, Vec<Change>, Vec<String>), String> {
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(|e| format!("This is not a Datum Schema project ({e})."))?;
    let mut w: Vec<String> = vec![];
    let meta = |k: &str| -> Option<String> { c.query_row("SELECT value FROM meta WHERE key = ?1", params![k], |r| r.get::<_, String>(0)).ok() };
    let format: u32 = meta("format").and_then(|f| f.parse().ok()).ok_or("This is not a Datum Schema project (it has no format).")?;
    if format > model::FORMAT {
        return Err(format!("This file was saved by a newer Datum Schema (format {format}). Update Datum Schema to open it."));
    }
    let mut p = Project { sheets: vec![], nodes: vec![], ..Project::new() };
    p.name = meta("name").unwrap_or_default();
    p.units = meta("units").unwrap_or_else(|| "SI".into());
    p.next = meta("next").and_then(|n| n.parse().ok()).unwrap_or(1);
    p.title = meta("title").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    p.settings = meta("settings").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    p.custom_syms = meta("symbols").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // Each table on its own; rows that cannot be read are counted and left out.
    fn table<T>(c: &Connection, q: &str, what: &str, w: &mut Vec<String>, f: impl Fn(&rusqlite::Row) -> rusqlite::Result<Option<T>>) -> Vec<T> {
        let mut out = vec![];
        let mut bad = 0;
        match c.prepare(q) {
            Ok(mut st) => match st.query_map([], |r| f(r)) {
                Ok(rows) => {
                    for r in rows.take(model::MAX_COUNT) {
                        match r {
                            Ok(Some(x)) => out.push(x),
                            _ => bad += 1,
                        }
                    }
                }
                Err(_) => w.push(format!("The file's {what} could not be read.")),
            },
            Err(_) => w.push(format!("The file's {what} could not be read.")),
        }
        if bad > 0 {
            w.push(format!("{bad} {what} could not be read and were left out."));
        }
        out
    }
    p.nodes = table(&c, "SELECT id, parent, level, name FROM nodes", "hierarchy levels", &mut w, |r| Ok(Some(Node { id: r.get(0)?, parent: r.get(1)?, level: r.get(2)?, name: r.get(3)? })));
    let mut objects: Vec<Obj> = table(&c, "SELECT id, uid, class, tag, node FROM objects", "objects", &mut w, |r| Ok(Some(Obj { id: r.get(0)?, uid: r.get(1)?, class: r.get(2)?, tag: r.get(3)?, node: r.get(4)?, data: Data::new() })));
    let attrs: Vec<(u32, String, Val)> = table(&c, "SELECT object, name, number, text FROM attributes", "attribute values", &mut w, |r| {
        let (num, text): (Option<f64>, Option<String>) = (r.get(2)?, r.get(3)?);
        Ok(num.map(Val::Num).or(text.map(Val::Text)).map(|v| (r.get::<_, u32>(0).unwrap_or(0), r.get::<_, String>(1).unwrap_or_default(), v)))
    });
    let index: std::collections::HashMap<u32, usize> = objects.iter().enumerate().map(|(i, o)| (o.id, i)).collect();
    for (o, k, v) in attrs {
        if let Some(&i) = index.get(&o) {
            objects[i].data.insert(k, v);
        }
    }
    p.objects = objects;
    let mut sheets: Vec<(i64, Sheet)> = table(&c, "SELECT position, sheet FROM sheets", "sheets", &mut w, |r| Ok(serde_json::from_str::<Sheet>(&r.get::<_, String>(1)?).ok().map(|s| (r.get::<_, i64>(0).unwrap_or(0), s))));
    sheets.sort_by_key(|x| x.0);
    p.sheets = sheets.into_iter().map(|x| x.1).collect();
    // Old drawings behind sheets (from format 4).
    if p.sheets.iter().any(|s| s.underlay.is_some()) {
        let draws: std::collections::HashMap<u32, String> = table(&c, "SELECT sheet, draw FROM underlays", "background drawings", &mut w, |r| Ok(Some((r.get(0)?, r.get(1)?)))).into_iter().collect();
        for s in &mut p.sheets {
            if let Some(u) = s.underlay.as_mut() {
                u.draw = std::sync::Arc::new(draws.get(&s.id).cloned().unwrap_or_default());
            }
        }
    }
    // The drawing standard's base symbol is in the items table from format 4.
    let base = if format >= 4 { ", base" } else { ", ''" };
    p.items = table(&c, &format!("SELECT id, sheet, object, symbol, x, y, rot, flip, scale, link{base} FROM items"), "symbols", &mut w, |r| {
        Ok(Some(Item { id: r.get(0)?, sheet: r.get(1)?, obj: r.get(2)?, sym: r.get(3)?, x: r.get(4)?, y: r.get(5)?, rot: r.get::<_, i64>(6)?.rem_euclid(4) as u8, flip: r.get(7)?, scale: r.get(8)?, link: r.get(9)?, base: r.get(10)? }))
    });
    p.lines = table(&c, "SELECT line FROM lines", "lines", &mut w, |r| Ok(serde_json::from_str::<Line>(&r.get::<_, String>(0)?).ok()));
    p.notes = table(&c, "SELECT note FROM notes", "notes", &mut w, |r| Ok(serde_json::from_str::<Note>(&r.get::<_, String>(0)?).ok()));
    // Datasheet revisions (format 3 on; format 2 files have none).
    if format >= 3 {
        p.issues = table(&c, "SELECT object, rev, status, date, issued_by, note, snapshot FROM issues ORDER BY seq", "datasheet revisions", &mut w, |r| {
            Ok(serde_json::from_str(&r.get::<_, String>(6)?).ok().map(|values| Issue { obj: r.get(0).unwrap_or(0), rev: r.get(1).unwrap_or_default(), status: r.get(2).unwrap_or_default(), date: r.get(3).unwrap_or_default(), by: r.get(4).unwrap_or_default(), note: r.get(5).unwrap_or_default(), values }))
        });
    }
    let history = table(&c, "SELECT seq, time, user, what, object, uid, name, old, new FROM history ORDER BY seq", "history entries", &mut w, |r| {
        Ok(Some(Change { seq: r.get::<_, i64>(0)?.max(0) as u64, time: r.get::<_, i64>(1)?.max(0) as u64, user: r.get(2)?, what: r.get(3)?, obj: r.get(4)?, uid: r.get(5)?, key: r.get(6)?, old: r.get(7)?, new: r.get(8)? }))
    });
    w.extend(p.normalize());
    Ok((p, history, w))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("datum-schema-test-{}-{}", std::process::id(), name));
        let _ = std::fs::create_dir_all(&d);
        d.join("p.dschema")
    }

    #[test]
    fn format_2_files_have_no_datasheet_revisions_and_open_cleanly() {
        let path = tmp("f2");
        crate::doc::Doc::new(crate::examples::get("pump").unwrap()).save(&path).unwrap();
        let c = Connection::open(&path).unwrap();
        c.execute_batch("DROP TABLE issues; UPDATE meta SET value = '2' WHERE key = 'format';").unwrap();
        drop(c);
        let (p, _, w) = read(&path).unwrap();
        assert!(w.is_empty() && p.issues.is_empty(), "{w:?}");
        // A file from a newer version says so.
        let c = Connection::open(&path).unwrap();
        c.execute("UPDATE meta SET value = ?1 WHERE key = 'format'", [(model::FORMAT + 1).to_string()]).unwrap();
        drop(c);
        assert!(read(&path).unwrap_err().contains("newer"));
    }

    #[test]
    fn sqlite_round_trip_with_history() {
        let mut d = crate::doc::Doc::new(crate::examples::get("pump").unwrap());
        d.user = "Tester".into();
        let pump = d.p.objects.iter().find(|o| o.tag == "P-101A").unwrap().id;
        d.apply(crate::doc::Op::SetData { ids: vec![pump], key: "service".into(), val: Some(Val::Text("Transfer «duty» 'quoted'".into())) }).unwrap();
        let path = tmp("rt");
        d.save(&path).unwrap();
        let (back, hist, w) = read(&path).unwrap();
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(back, d.p);
        assert_eq!(hist, d.history);
        assert!(hist.iter().any(|h| h.what == "set" && h.key == "service" && h.user == "Tester" && h.new.contains("«duty»")));
        // The file is plain SQLite: its objects can be queried.
        let c = Connection::open(&path).unwrap();
        let n: i64 = c.query_row("SELECT count(*) FROM object_attributes WHERE tag = 'P-101A' AND name = 'service'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn json_files_from_0_1_convert() {
        // A 0.1 file: tags and data on the symbols and line pieces.
        let old = r#"{"format":1,"name":"Old","sheets":[{"id":1,"number":"PID-001"}],
          "items":[{"id":2,"sheet":1,"sym":"vvessel","x":50,"y":100,"tag":"V-1","data":{"service":"Old drum"}},
                   {"id":3,"sheet":1,"sym":"gate","x":150,"y":108,"tag":"HV-1"},
                   {"id":4,"sheet":1,"sym":"vvessel","x":250,"y":100,"tag":"V-2"}],
          "lines":[{"id":5,"sheet":1,"kind":"major","a":{"at":"port","item":2,"port":5},"b":{"at":"port","item":3,"port":0},"data":{"size":"3","seq":"7"}},
                   {"id":6,"sheet":1,"kind":"major","a":{"at":"port","item":3,"port":1},"b":{"at":"port","item":4,"port":3},"data":{"size":"3","seq":"7"}}],
          "next":7}"#;
        let path = tmp("old");
        std::fs::write(&path, old).unwrap();
        let (p, _, w) = read(&path).unwrap();
        assert!(w.iter().any(|x| x.contains("0.1.4 or earlier")));
        let v1 = p.items.iter().find(|i| i.id == 2).unwrap();
        assert_eq!(p.tag_of(v1), "V-1");
        assert_eq!(p.data_of(v1.obj).get("service"), Some(&Val::Text("Old drum".into())));
        let (a, b) = (p.line(5).unwrap(), p.line(6).unwrap());
        assert_eq!(a.obj, b.obj, "one line number through the valve: one object");
        assert_eq!(p.data_of(a.obj).get("seq"), Some(&Val::Text("7".into())));
    }

    #[test]
    fn damaged_databases_say_why() {
        let mut d = crate::doc::Doc::new(crate::examples::get("compressor").unwrap());
        let path = tmp("dmg");
        d.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        for cut in [16usize, 100, 1000, bytes.len() / 2, bytes.len() - 10] {
            std::fs::write(&path, &bytes[..cut.min(bytes.len())]).unwrap();
            match read(&path) {
                Ok((p, _, _)) => {
                    let mut q = p.clone();
                    assert!(q.normalize().is_empty());
                }
                Err(e) => assert!(!e.is_empty()),
            }
        }
        // Garbled pages.
        let mut g = bytes.clone();
        for i in (200..g.len()).step_by(97) {
            g[i] ^= 0x5A;
        }
        std::fs::write(&path, &g).unwrap();
        let _ = read(&path);
    }
}
