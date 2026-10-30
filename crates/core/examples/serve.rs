//! The test server for the UI tests: the window (ui/dist) and the engine behind one local HTTP
//! port, answering the same commands as the desktop shell (`src-tauri/src/main.rs`) with the same
//! engine calls, so the Playwright tests drive the real engine. Run:
//! `cargo run -p datum-core --example serve -- 8765` (then open http://127.0.0.1:8765/).
use datum_core::doc::{Doc, Op};
use datum_core::model::{self, Project};
use datum_core::{api, dxf, examples, lists, units, view};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

struct St {
    doc: Doc,
    opts: Value,
    exports: Vec<Value>,
}

fn out(s: &St, created: Vec<u32>, warnings: Vec<String>) -> Value {
    json!({ "p": s.doc.p, "path": s.doc.path, "dirty": s.doc.dirty, "undo": s.doc.can_undo(), "redo": s.doc.can_redo(), "created": created, "warnings": warnings })
}

fn call(s: &mut St, cmd: &str, a: &Value) -> Result<Value, String> {
    let str_ = |k: &str| a[k].as_str().unwrap_or("").to_string();
    Ok(match cmd {
        "state" => out(s, vec![], vec![]),
        "apply" => {
            let op: Op = serde_json::from_value(a["op"].clone()).map_err(|e| format!("Bad operation: {e}"))?;
            let c = s.doc.apply(op)?;
            out(s, c, vec![])
        }
        "history" => {
            if a["redo"].as_bool().unwrap_or(false) { s.doc.redo() } else { s.doc.undo() }
            out(s, vec![], vec![])
        }
        "changes" => {
            let id = a["id"].as_u64().unwrap_or(0) as u32;
            let obj = s.doc.p.obj_id(id).unwrap_or(0);
            json!(s.doc.history.iter().rev().filter(|c| id == 0 || c.obj == obj).take(500).cloned().collect::<Vec<_>>())
        }
        "datasheet" => serde_json::to_value(datum_core::datasheets::build(&s.doc.p, a["id"].as_u64().unwrap_or(0) as u32)?).unwrap(),
        "datasheets_all" => serde_json::to_value(datum_core::datasheets::all(&s.doc.p)).unwrap(),
        "derived" => serde_json::to_value(api::derived(&s.doc.p, a["sheet"].as_u64().unwrap_or(0) as u32)).unwrap(),
        "list" => serde_json::to_value(lists::build(&s.doc.p, &str_("kind"))?).unwrap(),
        "views" => serde_json::to_value(view::all(&s.doc.p)).unwrap(),
        "examples_list" => serde_json::to_value(examples::list()).unwrap(),
        "new_project" => {
            let p = match a["example"].as_str() { Some(id) => examples::get(id).ok_or("That example does not exist.")?, None => Project::new() };
            s.doc = Doc::new(p);
            out(s, vec![], vec![])
        }
        "library" => serde_json::to_value(api::library()).unwrap(),
        "unit_table" => json!([units::shown(false), units::shown(true)]),
        "options" => s.opts.clone(),
        "set_options" => {
            s.opts = a["opts"].clone();
            out(s, vec![], vec![])
        }
        "open" => {
            let path = str_("path");
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let (p, w) = model::from_json(&text)?;
            s.doc = Doc::new(p);
            s.doc.path = Some(path);
            out(s, vec![], w)
        }
        "save" => {
            let json = model::to_json(&s.doc.p);
            // The real project file (SQLite), written and read back.
            let path = std::env::temp_dir().join(format!("datum-schema-serve-{}.dschema", std::process::id()));
            let sqlite = s.doc.save(&path).and_then(|_| datum_core::file::read(&path)).map(|(p, h, _)| p == s.doc.p && h == s.doc.history).unwrap_or(false);
            s.exports.push(json!({ "kind": "dschema", "text": json, "sqlite": sqlite }));
            s.doc.dirty = false;
            s.doc.path = Some("test.dschema".into());
            out(s, vec![], vec![])
        }
        "save_file" => {
            s.exports.push(json!({ "kind": "file", "name": str_("name"), "text": str_("data") }));
            json!(str_("name"))
        }
        "export" => {
            let (kind, what) = (str_("kind"), str_("what"));
            let text = match kind.as_str() {
                "csv" => lists::csv(&lists::build(&s.doc.p, &what)?),
                "xlsx" => format!("{} bytes", lists::xlsx(&s.doc.p, &what)?.len()),
                "cause" => format!("{} bytes", datum_core::cause::xlsx(&s.doc.p)?.len()),
                "loopdxf" => {
                    let d = datum_core::loops::diagram(&s.doc.p, &what)?;
                    dxf::write_prims(&d.prims, d.w, d.h)
                }
                "dexpi" => {
                    let sh = a["sheet"].as_u64().unwrap_or(0) as u32;
                    let sheet = s.doc.p.sheet(sh).or(s.doc.p.sheets.first()).ok_or("There is no sheet.")?.clone();
                    datum_core::dexpi::write(&s.doc.p, &sheet)
                }
                "datasheets" => {
                    // Kept as the file the next datasheet import reads (the test cannot pick one).
                    let bytes = datum_core::datasheets::xlsx(&s.doc.p)?;
                    std::fs::write(import_file(), &bytes).map_err(|e| e.to_string())?;
                    format!("{} bytes", bytes.len())
                }
                _ => {
                    let sh = a["sheet"].as_u64().unwrap_or(0) as u32;
                    let sheet = s.doc.p.sheet(sh).or(s.doc.p.sheets.first()).ok_or("There is no sheet.")?.clone();
                    dxf::write(&s.doc.p, &sheet)
                }
            };
            s.exports.push(json!({ "kind": kind, "what": what, "text": text }));
            json!(format!("export.{kind}"))
        }
        "import_datasheets" => {
            let bytes = std::fs::read(import_file()).map_err(|_| "Export the datasheets first: the test imports the last export.".to_string())?;
            serde_json::to_value(datum_core::import::plan(&s.doc.p, &bytes)?).unwrap()
        }
        "cause_effect" => serde_json::to_value(datum_core::cause::matrix(&s.doc.p)).unwrap(),
        "loops" => serde_json::to_value(datum_core::loops::all(&s.doc.p)).unwrap(),
        "loop_diagram" => serde_json::to_value(datum_core::loops::diagram(&s.doc.p, &str_("key"))?).unwrap(),
        "layout_rows" => serde_json::to_value(datum_core::datasheets::layout_rows(&s.doc.p)).unwrap(),
        "exports" => json!(s.exports),
        "recent" => json!([]),
        "recovery" | "startup_file" | "check_update" => Value::Null,
        "version" => json!("test"),
        _ => Value::Null,
    })
}

fn import_file() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("datum-schema-ui-{}.xlsx", std::process::id()))
}

fn main() {
    let port: u16 = std::env::args().nth(1).and_then(|p| p.parse().ok()).unwrap_or(8765);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist");
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("port in use");
    let opts = json!({ "autosave": 0, "beta": false, "theme": "light", "openLast": false, "units": "SI", "grid": true, "paper": "sheet", "company": "", "defaults": null });
    let mut st = St { doc: Doc::new(Project::new()), opts, exports: vec![] };
    println!("ready on {port}");
    for stream in listener.incoming().flatten() {
        let mut r = BufReader::new(&stream);
        let mut first = String::new();
        if r.read_line(&mut first).is_err() {
            continue;
        }
        let mut len = 0usize;
        loop {
            let mut h = String::new();
            if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                break;
            }
            if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                len = v.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0u8; len];
        let _ = r.read_exact(&mut body);
        let path = first.split_whitespace().nth(1).unwrap_or("/").split('?').next().unwrap_or("/").to_string();
        let (status, ctype, bytes) = if path == "/invoke" {
            let v: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let cmd = v["cmd"].as_str().unwrap_or("").to_string();
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&mut st, &cmd, &v["args"]))).unwrap_or_else(|_| Err("internal error".into()));
            let reply = match res { Ok(x) => json!({ "ok": x }), Err(e) => json!({ "err": e }) };
            ("200 OK", "application/json", serde_json::to_vec(&reply).unwrap())
        } else if path == "/quit" {
            let _ = (&stream).write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            break;
        } else {
            let f = root.join(if path == "/" { "index.html" } else { path.trim_start_matches('/') });
            let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("");
            let ct = match ext { "js" => "text/javascript", "css" => "text/css", "svg" => "image/svg+xml", "woff2" => "font/woff2", "woff" => "font/woff", "html" => "text/html", _ => "application/octet-stream" };
            match std::fs::read(&f) { Ok(b) if f.starts_with(&root) => ("200 OK", ct, b), _ => ("404 Not Found", "text/plain", b"not found".to_vec()) }
        };
        let mut w = &stream;
        let _ = write!(w, "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len());
        let _ = w.write_all(&bytes);
    }
}
