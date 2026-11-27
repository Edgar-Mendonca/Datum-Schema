//! `datum-schema-cli mcp`: a Model Context Protocol server over stdio (one JSON-RPC message per
//! line), so an AI assistant on the user's own computer can open or start a project, read its
//! drawing, data, checks and lists, plan an isolation, change it only through the named operations
//! (`list_operations`) with undo, and save it. Nothing is written unless `save_project` is called,
//! and then only a project or export file type. A tool that fails changes nothing; a panic inside a
//! tool is caught and reported.

use crate::{info, issues, load, write};
use datum_core::doc::{Doc, Op};
use datum_core::model::{End, Project, Val};
use datum_core::{examples, graph, isolation, lists, ops, symbols};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn obj(props: Value, req: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": req })
}

fn tools() -> Value {
    let ids = json!({ "type": "array", "items": { "type": "integer" } });
    json!([
        { "name": "open_project", "description": "Open a Datum Schema project (.dschema) or a DEXPI P&ID (.xml) to work on. Returns its facts.", "inputSchema": obj(json!({ "path": { "type": "string" } }), &["path"]) },
        { "name": "new_project", "description": "Start a new empty project, or one of the examples (see list_examples).", "inputSchema": obj(json!({ "example": { "type": "string" } }), &[]) },
        { "name": "list_examples", "description": "The example projects.", "inputSchema": obj(json!({}), &[]) },
        { "name": "project_info", "description": "Facts about the open project: sheets, tags, line numbers and how many checks are open.", "inputSchema": obj(json!({}), &[]) },
        { "name": "get_drawing", "description": "One sheet's symbols (id, symbol key, tag, x, y in mm) and lines (id, kind, line number, what each end connects to). Without sheet: the first P&ID.", "inputSchema": obj(json!({ "sheet": { "type": "string", "description": "sheet number, e.g. PID-100" } }), &[]) },
        { "name": "get_object", "description": "A symbol's or line's object by id or tag: its class, tag, every data value (shown in the project's units, and raw in SI base units) and where it is drawn.", "inputSchema": obj(json!({ "id": { "type": "integer" }, "tag": { "type": "string" } }), &[]) },
        { "name": "run_checks", "description": "Every open check: rule, level (error, warning, note), sheet, the ids it is about, and the message.", "inputSchema": obj(json!({}), &[]) },
        { "name": "list_fixes", "description": "The fixes offered for one check (its rule and ids from run_checks); apply one with apply_operation {\"Fix\":{\"rule\":r,\"ids\":[..],\"choice\":n}}.", "inputSchema": obj(json!({ "rule": { "type": "integer" }, "ids": ids }), &["rule", "ids"]) },
        { "name": "get_list", "description": "A list as rows: lines, equipment, instruments, valves, register, safety, loads, testpacks, hazop… (see `datum-schema-cli lists`).", "inputSchema": obj(json!({ "kind": { "type": "string" } }), &["kind"]) },
        { "name": "find", "description": "Find anything with a plain query: \"control valves without a bypass\", \"lines with velocity above 3 m/s\", \"pumps without a check valve\", \"instruments without a range\", \"equipment with errors\", a tag pattern like FT-1*. Says what it understood and any words it did not.", "inputSchema": obj(json!({ "query": { "type": "string" } }), &["query"]) },
        { "name": "plan_isolation", "description": "The valves and blinds that isolate symbols or lines (by id), the vents and drains inside, and what stops a safe isolation. A planning aid.", "inputSchema": obj(json!({ "ids": ids }), &["ids"]) },
        { "name": "convert_dxf", "description": "Turn an old P&ID (DXF, DWG, or a PDF drawn by a CAD program) into a new live sheet of the open project (or a new project): blocks become symbols by their names, and instrument bubbles, bow-tie valves and pumps drawn as loose lines by their shapes (map overrides: {\"BLOCK\": \"gate\"}, \"\" leaves a block out), lines are traced and joined to the symbols, line numbers read. Without convert: true, only lists the blocks and what each would become.", "inputSchema": obj(json!({ "path": { "type": "string" }, "map": { "type": "object" }, "convert": { "type": "boolean" } }), &["path"]) },
        { "name": "list_symbols", "description": "The symbol keys AddItem takes, with their connection points.", "inputSchema": obj(json!({}), &[]) },
        { "name": "list_operations", "description": "The named operations that change a project, each with an example.", "inputSchema": obj(json!({}), &[]) },
        { "name": "apply_operation", "description": "Change the open project with one named operation (see list_operations), e.g. {\"SetTag\":{\"id\":11,\"tag\":\"HV-101\"}}. Numbers are SI base units. A failed operation changes nothing. Returns the ids it created and the checks now open.", "inputSchema": obj(json!({ "operation": { "type": ["object", "string"] } }), &["operation"]) },
        { "name": "undo", "description": "Undo the last change.", "inputSchema": obj(json!({}), &[]) },
        { "name": "save_project", "description": "Save the project (.dschema) or export it by extension: .dxf .xml (DEXPI) .html (web page) .zip (handover package) .json.", "inputSchema": obj(json!({ "path": { "type": "string" }, "sheet": { "type": "string" } }), &["path"]) },
    ])
}

fn end(p: &Project, e: &End) -> Value {
    match e {
        End::Port { item, port } => {
            let it = p.item(*item);
            let name = it.and_then(|i| symbols::get(&i.sym)).and_then(|s| s.ports.get(*port as usize)).map_or("", |q| q.name);
            json!({ "at": "port", "item": item, "tag": it.map(|i| p.tag_of(i)).unwrap_or(""), "port": port, "port_name": name })
        }
        End::Line { line, x, y } => json!({ "at": "line", "line": line, "x": x, "y": y }),
        End::Free { x, y } => json!({ "at": "free", "x": x, "y": y }),
    }
}

fn drawing(p: &Project, sheet: Option<&str>) -> Result<Value, String> {
    let s = match sheet {
        Some(n) => p.sheets.iter().find(|s| s.number.eq_ignore_ascii_case(n)).ok_or_else(|| format!("There is no sheet {n}."))?,
        None => p.sheets.iter().find(|s| !s.legend && !s.pfd).or(p.sheets.first()).ok_or("The project has no sheet.")?,
    };
    let items: Vec<Value> = p.items.iter().filter(|i| i.sheet == s.id).map(|i| json!({ "id": i.id, "sym": i.sym, "name": symbols::get(&i.sym).map_or("", |x| x.name), "tag": p.tag_of(i), "x": i.x, "y": i.y, "rot": i.rot })).collect();
    let lines: Vec<Value> = p.lines.iter().filter(|l| l.sheet == s.id).map(|l| json!({ "id": l.id, "kind": l.kind, "number": graph::number(p, l), "a": end(p, &l.a), "b": end(p, &l.b) })).collect();
    Ok(json!({ "sheet": s.number, "title": s.title, "pfd": s.pfd, "symbols": items, "lines": lines }))
}

fn object(p: &Project, a: &Value) -> Result<Value, String> {
    let us = p.units == "US";
    let obj = if let Some(id) = a["id"].as_u64().map(|x| x as u32) {
        p.item(id).map(|i| i.obj).or_else(|| p.line(id).map(|l| l.obj)).or_else(|| p.obj(id).map(|o| o.id)).ok_or_else(|| format!("There is no symbol, line or object {id}."))?
    } else if let Some(t) = a["tag"].as_str() {
        p.objects.iter().find(|o| o.tag.eq_ignore_ascii_case(t) || graph::obj_number(p, o.id).eq_ignore_ascii_case(t)).map(|o| o.id).ok_or_else(|| format!("There is no tag {t}."))?
    } else {
        return Err("Give id or tag.".into());
    };
    let o = p.obj(obj).ok_or("That object no longer exists.")?;
    let attrs = datum_core::classes::attrs(Some(p), &o.class);
    let data: Vec<Value> = o
        .data
        .iter()
        .filter(|(k, _)| !k.starts_with('~'))
        .map(|(k, v)| {
            let a = attrs.iter().find(|x| &x.key == k);
            let shown = match (v, a) {
                (Val::Num(x), Some(a)) if !a.q.is_empty() => datum_core::units::show(*x, &a.q, us),
                (v, _) => v.text(),
            };
            json!({ "key": k, "label": a.map_or(k.as_str(), |a| a.label.as_str()), "shown": shown, "raw": v })
        })
        .collect();
    let drawn: Vec<Value> = p.items.iter().filter(|i| i.obj == obj).map(|i| json!({ "symbol": i.id, "sheet": p.sheet(i.sheet).map(|s| s.number.clone()) })).chain(p.lines.iter().filter(|l| l.obj == obj).map(|l| json!({ "line": l.id, "sheet": p.sheet(l.sheet).map(|s| s.number.clone()) }))).collect();
    Ok(json!({ "object": obj, "tag": if o.tag.is_empty() { graph::obj_number(p, obj) } else { o.tag.clone() }, "class": datum_core::classes::get(&o.class).map_or(o.class.clone(), |c| c.label.clone()), "data": data, "attributes": attrs.iter().map(|a| json!({"key": a.key, "label": a.label, "quantity": a.q})).collect::<Vec<_>>(), "drawn": drawn }))
}

fn checks_json(p: &Project) -> Value {
    json!(issues(p).into_iter().map(|i| json!({ "rule": i.rule, "level": i.level, "sheet": p.sheet(i.sheet).map(|s| s.number.clone()), "ids": i.ids, "message": i.msg })).collect::<Vec<_>>())
}

/// One tool call on the open project.
fn call(doc: &mut Option<Doc>, name: &str, a: &Value) -> Result<Value, String> {
    let s = |k: &str| a[k].as_str().map(String::from);
    match name {
        "open_project" => {
            let d = load(&s("path").ok_or("path is needed")?)?;
            let v = info(&d.p);
            *doc = Some(d);
            Ok(v)
        }
        "new_project" => {
            let p = match s("example") {
                Some(id) if !id.is_empty() => examples::get(&id).ok_or_else(|| format!("There is no example {id}; see list_examples."))?,
                _ => Project::new(),
            };
            let v = info(&p);
            *doc = Some(Doc::new(p));
            Ok(v)
        }
        "list_examples" => Ok(json!(examples::list())),
        "convert_dxf" => {
            let path = s("path").ok_or("path is needed")?;
            if ![".dxf", ".dwg", ".pdf"].iter().any(|x| path.to_lowercase().ends_with(x)) {
                return Err("Give a .dxf, .dwg or .pdf drawing.".into());
            }
            let text = datum_core::dxfin::source_checked(&std::fs::read(&path).map_err(|e| format!("Could not open {path}: {e}"))?)?;
            let scan = datum_core::convert::scan(&text)?;
            let mut map = datum_core::convert::suggested(&scan);
            if let Some(m) = a["map"].as_object() {
                for (k, v) in m {
                    map.insert(k.clone(), v.as_str().unwrap_or("").to_string());
                }
            }
            if !a["convert"].as_bool().unwrap_or(false) {
                return Ok(json!({ "scan": scan, "map": map }));
            }
            let d = doc.get_or_insert_with(|| Doc::new(Project::new()));
            let name = std::path::Path::new(&path).file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
            let mut probe = d.p.clone();
            let rep = datum_core::convert::convert(&mut probe, &text, &name, &map, &mut vec![])?;
            d.apply(Op::ConvertDxf { text, name, map })?;
            Ok(json!(rep))
        }
        "list_operations" => Ok(json!(ops::catalog())),
        "list_symbols" => Ok(json!(symbols::all().iter().filter(|s| !s.rare).map(|s| json!({"key": s.key, "name": s.name, "category": s.cat.name(), "ports": s.ports.iter().map(|q| q.name).collect::<Vec<_>>()})).collect::<Vec<_>>())),
        _ => {
            let d = doc.as_mut().ok_or("Open or start a project first (open_project or new_project).")?;
            match name {
                "project_info" => Ok(info(&d.p)),
                "get_drawing" => drawing(&d.p, s("sheet").as_deref()),
                "get_object" => object(&d.p, a),
                "run_checks" => Ok(checks_json(&d.p)),
                "list_fixes" => {
                    let ids: Vec<u32> = a["ids"].as_array().map(|v| v.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect()).unwrap_or_default();
                    let labels = datum_core::fixes::labels(&d.p, a["rule"].as_u64().unwrap_or(0) as u8, &ids);
                    Ok(json!(labels.into_iter().enumerate().map(|(k, l)| json!({"choice": k, "fix": l})).collect::<Vec<_>>()))
                }
                "get_list" => {
                    let l = lists::build(&d.p, &s("kind").ok_or("kind is needed")?)?;
                    Ok(json!({ "title": l.title, "columns": l.cols.iter().map(|c| c.label.clone()).collect::<Vec<_>>(), "rows": l.rows.iter().map(|r| json!({"ids": r.ids, "cells": r.cells})).collect::<Vec<_>>() }))
                }
                "find" => Ok(json!(datum_core::query::find(&d.p, &s("query").ok_or("query is needed")?)?)),
                "plan_isolation" => {
                    let ids: Vec<u32> = a["ids"].as_array().map(|v| v.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect()).unwrap_or_default();
                    Ok(json!(isolation::plan(&d.p, &ids)?))
                }
                "apply_operation" => {
                    let op: Op = serde_json::from_value(a["operation"].clone()).map_err(|e| format!("Not an operation ({e}). See list_operations."))?;
                    let created = d.apply(op)?;
                    let is = issues(&d.p);
                    Ok(json!({ "created": created, "errors": is.iter().filter(|i| i.level == "error").count(), "warnings": is.iter().filter(|i| i.level == "warning").count() }))
                }
                "undo" => {
                    d.undo();
                    Ok(info(&d.p))
                }
                "save_project" => {
                    let out = s("path").ok_or("path is needed")?;
                    let mut args = vec![];
                    if let Some(sh) = s("sheet") {
                        args = vec!["--sheet".to_string(), sh];
                    }
                    Ok(json!({ "saved": write(d, &out, &args)? }))
                }
                x => Err(format!("Unknown tool {x}.")),
            }
        }
    }
}

/// One JSON-RPC message per line in, one per line out.
pub fn serve(input: impl BufRead, mut out: impl Write) {
    let mut doc: Option<Doc> = None;
    for line in input.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(out, "{}", json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("Parse error: {e}")}}));
                let _ = out.flush();
                continue;
            }
        };
        let Some(id) = msg.get("id").cloned() else { continue }; // a notification: no answer
        let p = &msg["params"];
        let result = match msg["method"].as_str().unwrap_or("") {
            "initialize" => Ok(json!({
                "protocolVersion": p["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "datum-schema", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Datum Schema draws and checks P&IDs and process flow diagrams. Open or start a project, read it with get_drawing, get_object, run_checks and get_list, change it only with apply_operation (see list_operations and list_symbols; numbers in SI base units), and save it with save_project. Nothing is written to disk unless save_project is called. Hydraulic, sizing, isolation, HAZOP and test-pack results are planning aids for the engineer to check."
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = p["name"].as_str().unwrap_or("");
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&mut doc, name, &p["arguments"]))).unwrap_or_else(|_| Err("Internal error; the project is unchanged.".into()));
                Ok(match r {
                    Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }], "isError": false }),
                    Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
                })
            }
            m => Err(json!({ "code": -32601, "message": format!("Method not found: {m}") })),
        };
        let reply = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        };
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(lines: &[Value]) -> Vec<Value> {
        let input: String = lines.iter().map(|l| l.to_string() + "\n").collect::<String>() + "not json at all\n";
        let mut out = vec![];
        serve(input.as_bytes(), &mut out);
        String::from_utf8_lossy(&out).lines().map(|l| serde_json::from_str(l).unwrap_or(Value::Null)).collect()
    }
    fn tool(id: u64, name: &str, args: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}})
    }
    fn text(r: &Value) -> Value {
        serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap_or("")).unwrap_or(Value::Null)
    }

    #[test]
    fn an_assistant_can_read_check_change_and_save_a_project() {
        let dir = std::env::temp_dir().join(format!("dsmcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.dschema").display().to_string();
        let p = examples::get("pump").unwrap();
        let hv = p.items.iter().find(|i| p.tag_of(i) == "HV-101").unwrap().id;
        let pump = p.items.iter().find(|i| p.tag_of(i) == "P-101A").unwrap().id;
        let r = session(&[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            tool(3, "run_checks", json!({})),
            tool(4, "new_project", json!({"example": "pump"})),
            tool(5, "get_drawing", json!({})),
            tool(6, "get_object", json!({"tag": "P-101A"})),
            tool(7, "apply_operation", json!({"operation": {"SetTag": {"id": hv, "tag": "HV-901"}}})),
            tool(8, "apply_operation", json!({"operation": {"SetTag": {"id": 999_999, "tag": "X-1"}}})),
            tool(9, "plan_isolation", json!({"ids": [pump]})),
            tool(91, "find", json!({"query": "gate valves tagged HV-90*"})),
            tool(10, "get_list", json!({"kind": "valves"})),
            tool(11, "save_project", json!({"path": path})),
            tool(12, "save_project", json!({"path": dir.join("x.exe").display().to_string()})),
            tool(13, "run_checks", json!({})),
            json!({"jsonrpc": "2.0", "id": 14, "method": "nope"}),
        ]);
        assert_eq!(r[0]["result"]["serverInfo"]["name"], "datum-schema");
        assert_eq!(r.len(), 16, "the notification gets no answer");
        assert_eq!(r[1]["result"]["tools"].as_array().map(|t| t.len()), Some(17));
        assert_eq!(r[2]["result"]["isError"], true, "no project open yet");
        assert_eq!(text(&r[3])["checks"]["errors"], 0);
        let dwg = text(&r[4]);
        assert!(dwg["symbols"].as_array().unwrap().iter().any(|s| s["tag"] == "P-101A") && dwg["lines"].as_array().unwrap().iter().any(|l| l["a"]["at"] == "port"));
        let pa = text(&r[5]);
        assert!(pa["data"].as_array().unwrap().iter().any(|d| d["key"] == "power" && d["shown"] == "7.5 kW"), "{pa}");
        assert_eq!(r[6]["result"]["isError"], false);
        assert_eq!(text(&r[6])["errors"], 0);
        assert_eq!(r[7]["result"]["isError"], true);
        assert!(text(&r[8])["points"].as_array().unwrap().iter().any(|x| x["tag"] == "HV-901"), "the isolation sees the renamed valve");
        assert_eq!(text(&r[9])["rows"][0]["tag"], "HV-901", "{}", r[9]);
        assert!(text(&r[10])["rows"].as_array().unwrap().iter().any(|x| x["cells"][0] == "HV-901"));
        assert_eq!(r[11]["result"]["isError"], false);
        let saved = crate::load(&path).unwrap();
        assert!(saved.p.items.iter().any(|i| saved.p.tag_of(i) == "HV-901"));
        assert_eq!(r[12]["result"]["isError"], true, "only project and export types are written");
        assert!(!dir.join("x.exe").exists());
        assert!(text(&r[13]).as_array().is_some());
        assert_eq!(r[14]["error"]["code"], -32601);
        assert_eq!(r[15]["error"]["code"], -32700);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
