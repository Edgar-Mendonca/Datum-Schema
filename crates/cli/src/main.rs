//! `datum-schema-cli`: Datum Schema without the window. Reads a project (.dschema, or a DEXPI P&ID),
//! reports its facts, checks and lists, changes it through the named operations the window uses
//! (`ops`), and writes it or any export. Nothing is written unless `--save` or `export` names a
//! file; only project and export file types are written.

use datum_core::doc::{Doc, Op};
use datum_core::model::Project;
use datum_core::{checks, dxf, examples, file, graph, handover, hydraulics, lists, ops, symbols, webpage};
use serde_json::{json, Value};
use std::path::Path;

const HELP: &str = "datum-schema-cli: Datum Schema on the command line

  info <project>                       facts: sheets, tags, lines, checks
  check <project> [--csv]              every open check (rule, level, sheet, message)
  objects <project> [--csv]            every symbol and line with its id, sheet and tag
  list <project> <kind> [--csv]        a list (see `lists`), as a table or CSV
  lists                                the list kinds
  symbols                              the symbol keys AddItem takes
  ops                                  the named operations, each with an example
  apply <project> --op <json> [--op <json>…] --save <out.dschema>
                                       change a project and save it (a failed operation changes nothing)
  export <project> <out> [--sheet <number>] [--list <kind>]
                                       by the extension: .dschema .dxf .xml (DEXPI) .html (web page)
                                       .zip (handover package) .xlsx or .csv (needs --list) .json
  new <out.dschema> [--example <id>]   a new project, or one of the examples (see `examples`)
  examples                             the examples

Projects are .dschema files; a DEXPI P&ID (.xml) can be read too. Numbers in operations are in SI
base units (Pa, K, m, m³/s). Hydraulic and sizing values are a sizing aid checked by the engineer.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(out) => println!("{out}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

/// Reads a project: a .dschema file, or a DEXPI P&ID.
pub fn load(path: &str) -> Result<Doc, String> {
    let p = Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "xml" {
        let text = std::fs::read_to_string(p).map_err(|e| format!("Could not open {path}: {e}"))?;
        let (proj, _) = datum_core::dexpi::read(&text)?;
        return Ok(Doc::new(proj));
    }
    let (proj, history, _) = file::read(p)?;
    let mut d = Doc::new(proj);
    d.history = history;
    Ok(d)
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn flags<'a>(args: &'a [String], name: &str) -> Vec<&'a str> {
    args.iter().enumerate().filter(|(_, a)| *a == name).filter_map(|(i, _)| args.get(i + 1)).map(|s| s.as_str()).collect()
}

fn need<'a>(args: &'a [String], i: usize, what: &str) -> Result<&'a str, String> {
    args.get(i).map(|s| s.as_str()).filter(|s| !s.starts_with("--")).ok_or_else(|| format!("Give {what}. `datum-schema-cli help` lists the commands."))
}

fn table(head: &[&str], rows: &[Vec<String>], csv: bool) -> String {
    if csv {
        let q = |s: &str| {
            let s = lists::safe(s);
            if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
        };
        let mut out = head.iter().map(|h| q(h)).collect::<Vec<_>>().join(",");
        for r in rows {
            out.push('\n');
            out.push_str(&r.iter().map(|c| q(c)).collect::<Vec<_>>().join(","));
        }
        return out;
    }
    let w: Vec<usize> = (0..head.len()).map(|i| rows.iter().map(|r| r.get(i).map_or(0, |c| c.chars().count())).max().unwrap_or(0).max(head[i].chars().count()).min(60)).collect();
    let line = |cells: Vec<&str>| cells.iter().enumerate().map(|(i, c)| { let c: String = c.chars().take(60).collect(); format!("{c:<width$}", width = w[i]) }).collect::<Vec<_>>().join("  ").trim_end().to_string();
    let mut out = line(head.to_vec());
    for r in rows {
        out.push('\n');
        out.push_str(&line(r.iter().map(|s| s.as_str()).collect()));
    }
    out
}

fn issues(p: &Project) -> Vec<checks::Issue> {
    let net = graph::Net::new(p);
    let hyd = hydraulics::all(p, &net);
    checks::run(p, &net, &hyd)
}

/// Facts about a project.
pub fn info(p: &Project) -> Value {
    let is = issues(p);
    let count = |lvl: &str| is.iter().filter(|i| i.level == lvl).count();
    let tags = p.objects.iter().filter(|o| !o.tag.is_empty()).count();
    json!({
        "name": p.name,
        "project": p.title.project,
        "units": p.units,
        "sheets": p.sheets.iter().map(|s| json!({"id": s.id, "number": s.number, "title": s.title, "kind": if s.legend { "Legend" } else if s.pfd { "PFD" } else { "P&ID" }, "rev": s.rev, "status": if s.status.is_empty() { "Draft" } else { s.status.as_str() }})).collect::<Vec<_>>(),
        "symbols": p.items.len(),
        "lines": p.lines.len(),
        "tags": tags,
        "line_numbers": graph::Net::new(p).runs.len(),
        "checks": {"errors": count("error"), "warnings": count("warning"), "notes": count("note")},
        "hazop_nodes": p.settings.hazop.nodes.len(),
        "test_packs": p.settings.testpacks.packs.len(),
    })
}

fn objects(p: &Project) -> Vec<Vec<String>> {
    let sheet = |id: u32| p.sheet(id).map(|s| s.number.clone()).unwrap_or_default();
    let mut rows: Vec<Vec<String>> = p.items.iter().map(|i| vec![i.id.to_string(), "symbol".into(), sheet(i.sheet), i.sym.clone(), p.tag_of(i).to_string(), i.obj.to_string()]).collect();
    rows.extend(p.lines.iter().map(|l| vec![l.id.to_string(), "line".into(), sheet(l.sheet), format!("{:?}", l.kind).to_lowercase(), graph::number(p, l), l.obj.to_string()]));
    rows
}

fn sheet_of<'a>(p: &'a Project, args: &[String]) -> Result<&'a datum_core::model::Sheet, String> {
    match flag(args, "--sheet") {
        Some(n) => p.sheets.iter().find(|s| s.number.eq_ignore_ascii_case(n) || s.id.to_string() == n).ok_or_else(|| format!("There is no sheet {n}.")),
        None => p.sheets.iter().find(|s| !s.legend && !s.pfd).or(p.sheets.first()).ok_or_else(|| "The project has no sheet.".into()),
    }
}

/// Writes a project or an export, chosen by the file's extension.
pub fn write(d: &Doc, out: &str, args: &[String]) -> Result<String, String> {
    let p = &d.p;
    let path = Path::new(out);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let today = flag(args, "--date").unwrap_or("").to_string();
    let bytes: Vec<u8> = match ext.as_str() {
        "dschema" => {
            file::write(path, p, &d.history)?;
            return Ok(format!("Saved {out}."));
        }
        "dxf" => dxf::write(p, sheet_of(p, args)?).into_bytes(),
        "xml" => {
            let s = sheet_of(p, args)?;
            if s.pfd {
                return Err(datum_core::dexpi::NOT_PFD.into());
            }
            datum_core::dexpi::write(p, s).into_bytes()
        }
        "html" => webpage::page(p, &today).into_bytes(),
        "zip" => handover::zip(p, &today)?,
        "json" => datum_core::model::to_json(p).into_bytes(),
        "csv" => lists::csv(&lists::build(p, flag(args, "--list").ok_or("A CSV export needs --list <kind> (see `lists`).")?)?).into_bytes(),
        "xlsx" => match flag(args, "--list") {
            Some("datasheets-all") => datum_core::datasheets::xlsx(p)?,
            Some(k) => lists::xlsx(p, k)?,
            None => return Err("An Excel export needs --list <kind> (see `lists`), or --list datasheets-all for every datasheet.".into()),
        },
        _ => return Err(format!("{out}: write .dschema, .dxf, .xml, .html, .zip, .json, .csv or .xlsx.")),
    };
    datum_core::doc::write_atomic(path, &bytes)?;
    Ok(format!("Saved {out} ({} bytes).", bytes.len()))
}

/// Runs one command; the text to print, or why not.
pub fn run(args: &[String]) -> Result<String, String> {
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");
    let csv = args.iter().any(|a| a == "--csv");
    let pretty = |v: Value| serde_json::to_string_pretty(&v).unwrap_or_default();
    match cmd {
        "help" | "--help" | "-h" => Ok(HELP.into()),
        "version" | "--version" => Ok(format!("datum-schema-cli {}", env!("CARGO_PKG_VERSION"))),
        "info" => Ok(pretty(info(&load(need(args, 1, "a project file")?)?.p))),
        "check" => {
            let d = load(need(args, 1, "a project file")?)?;
            let rows: Vec<Vec<String>> = issues(&d.p).into_iter().map(|i| vec![i.rule.to_string(), i.level.into(), d.p.sheet(i.sheet).map(|s| s.number.clone()).unwrap_or_default(), i.ids.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" "), i.msg]).collect();
            if rows.is_empty() && !csv {
                return Ok("No checks open.".into());
            }
            Ok(table(&["RULE", "LEVEL", "SHEET", "IDS", "MESSAGE"], &rows, csv))
        }
        "objects" => {
            let d = load(need(args, 1, "a project file")?)?;
            Ok(table(&["ID", "WHAT", "SHEET", "SYMBOL OR KIND", "TAG OR NUMBER", "OBJECT"], &objects(&d.p), csv))
        }
        "list" => {
            let d = load(need(args, 1, "a project file")?)?;
            let l = lists::build(&d.p, need(args, 2, "a list kind (see `lists`)")?)?;
            if csv {
                return Ok(lists::csv(&l).trim_end().to_string());
            }
            let head: Vec<&str> = l.cols.iter().map(|c| c.label.as_str()).collect();
            let rows: Vec<Vec<String>> = l.rows.iter().map(|r| r.cells.clone()).collect();
            Ok(format!("{}\n\n{}", l.title, table(&head, &rows, false)))
        }
        "lists" => Ok(table(&["KIND", "LIST"], &lists::KINDS.iter().map(|k| vec![k.0.to_string(), k.1.to_string()]).chain([vec!["testpack:<id>".into(), "One test pack (TP-001…)".into()], vec!["report:<name>".into(), "A report designed in the project".into()]]).collect::<Vec<_>>(), csv)),
        "symbols" => Ok(table(&["KEY", "NAME", "CATEGORY", "PORTS"], &symbols::all().iter().filter(|s| !s.rare).map(|s| vec![s.key.to_string(), s.name.to_string(), s.cat.name().to_string(), s.ports.iter().map(|q| q.name).collect::<Vec<_>>().join(", ")]).collect::<Vec<_>>(), csv)),
        "ops" => Ok(pretty(json!(ops::catalog()))),
        "examples" => Ok(table(&["ID", "NAME", "WHAT"], &examples::list().into_iter().map(|e| vec![e.id.to_string(), e.name.to_string(), e.what.to_string()]).collect::<Vec<_>>(), csv)),
        "apply" => {
            let mut d = load(need(args, 1, "a project file")?)?;
            let ops = flags(args, "--op");
            if ops.is_empty() {
                return Err("Give at least one --op '<json>' (see `ops`).".into());
            }
            let out = flag(args, "--save").ok_or("Give --save <out.dschema>: the project is changed only in the file you name.")?;
            let mut created = vec![];
            for (k, text) in ops.iter().enumerate() {
                let op: Op = serde_json::from_str(text).map_err(|e| format!("Operation {} is not one Datum Schema knows ({e}); `ops` lists them.", k + 1))?;
                created.extend(d.apply(op).map_err(|e| format!("Operation {}: {e} Nothing was saved.", k + 1))?);
            }
            let saved = write(&d, out, args)?;
            Ok(if created.is_empty() { saved } else { format!("{saved} Created ids: {}.", created.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")) })
        }
        "export" => {
            let d = load(need(args, 1, "a project file")?)?;
            write(&d, need(args, 2, "the file to write")?, args)
        }
        "new" => {
            let out = need(args, 1, "the new project's file name (.dschema)")?;
            if !out.to_lowercase().ends_with(".dschema") {
                return Err("A new project is a .dschema file.".into());
            }
            let p = match flag(args, "--example") {
                Some(id) => examples::get(id).ok_or_else(|| format!("There is no example {id}; `examples` lists them."))?,
                None => Project::new(),
            };
            file::write(Path::new(out), &p, &[])?;
            Ok(format!("Saved {out}."))
        }
        c => Err(format!("{c} is not a command. `datum-schema-cli help` lists them.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_command_line_reads_checks_changes_and_exports() {
        let dir = std::env::temp_dir().join(format!("dscli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = |n: &str| dir.join(n).display().to_string();
        assert!(run(&a(&["new", &f("pump.dschema"), "--example", "pump"])).unwrap().starts_with("Saved"));
        let info: Value = serde_json::from_str(&run(&a(&["info", &f("pump.dschema")])).unwrap()).unwrap();
        assert_eq!(info["checks"]["errors"], 0);
        assert!(info["tags"].as_u64().unwrap() > 10 && info["sheets"][0]["number"] == "PID-100");
        assert_eq!(run(&a(&["check", &f("pump.dschema")])).unwrap().lines().next().unwrap().split_whitespace().next(), Some("RULE").or(Some("No")));
        let objs = run(&a(&["objects", &f("pump.dschema"), "--csv"])).unwrap();
        assert!(objs.starts_with("ID,WHAT,SHEET") && objs.contains("P-101A"));
        let lines = run(&a(&["list", &f("pump.dschema"), "lines", "--csv"])).unwrap();
        assert!(lines.starts_with("Line number,") && lines.contains("P-1003"));
        // A change through a named operation: a tag on HV-101's valve, saved to a new file.
        let p = load(&f("pump.dschema")).unwrap().p;
        let hv = p.items.iter().find(|i| p.tag_of(i) == "HV-101").unwrap().id;
        let op = format!("{{\"SetTag\":{{\"id\":{hv},\"tag\":\"HV-901\"}}}}");
        let set = format!("{{\"SetData\":{{\"ids\":[{hv}],\"key\":\"service\",\"val\":\"Suction isolation\"}}}}");
        assert!(run(&a(&["apply", &f("pump.dschema"), "--op", &op, "--op", &set, "--save", &f("b.dschema")])).unwrap().starts_with("Saved"));
        let q = load(&f("b.dschema")).unwrap().p;
        assert!(q.items.iter().any(|i| q.tag_of(i) == "HV-901"));
        // A bad operation saves nothing; no --save, no change.
        assert!(run(&a(&["apply", &f("pump.dschema"), "--op", "{\"SetTag\":{\"id\":999999,\"tag\":\"X-1\"}}", "--save", &f("c.dschema")])).is_err());
        assert!(!dir.join("c.dschema").exists());
        assert!(run(&a(&["apply", &f("pump.dschema"), "--op", &op])).is_err());
        // Every export by extension.
        for out in ["e.dxf", "e.xml", "e.html", "e.zip", "e.json"] {
            assert!(run(&a(&["export", &f("pump.dschema"), &f(out), "--sheet", "PID-100"])).unwrap().starts_with("Saved"), "{out}");
            assert!(std::fs::metadata(dir.join(out)).unwrap().len() > 100, "{out}");
        }
        assert!(run(&a(&["export", &f("pump.dschema"), &f("e.xlsx"), "--list", "equipment"])).is_ok());
        assert!(run(&a(&["export", &f("pump.dschema"), &f("e.csv")])).is_err());
        assert!(run(&a(&["export", &f("pump.dschema"), &f("e.exe")])).is_err());
        // DEXPI read back as a project.
        let back: Value = serde_json::from_str(&run(&a(&["info", &f("e.xml")])).unwrap()).unwrap();
        assert!(back["tags"].as_u64().unwrap() > 10);
        // The catalogues.
        assert!(run(&a(&["ops"])).unwrap().contains("\"AddItem\""));
        assert!(run(&a(&["symbols"])).unwrap().contains("pump_cent"));
        assert!(run(&a(&["lists"])).unwrap().contains("testpacks"));
        assert!(run(&a(&["examples"])).unwrap().contains("worked"));
        assert!(run(&a(&["nonsense"])).is_err() && run(&a(&["help"])).unwrap().contains("apply"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
