//! Datum Schema desktop shell: owns the open project, files, options and recovery, and talks to
//! the window. All calculation is in `datum-core`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod update;

use datum_core::doc::{self, Doc, Op};
use datum_core::model::{self, Project};
use datum_core::{api, datasheets, dxf, examples, lists, units, view};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Options {
    /// Minutes between recovery saves; 0 turns them off.
    autosave: u32,
    beta: bool,
    /// auto, light or dark.
    theme: String,
    open_last: bool,
    /// Display units for new projects: SI or US.
    units: String,
    /// Show the grid on the sheet.
    grid: bool,
    paper: String,
    company: String,
    /// The name written in the history of changes (empty: the computer's user name).
    user: String,
    /// Project settings (line number format, checks switched off, hydraulic limits) for new projects.
    defaults: Option<model::Settings>,
}

impl Default for Options {
    fn default() -> Self {
        Self { autosave: 5, beta: false, theme: "auto".into(), open_last: false, units: "SI".into(), grid: true, paper: "sheet".into(), company: String::new(), user: String::new(), defaults: None }
    }
}

struct App {
    doc: Doc,
    opts: Options,
}

struct St(Mutex<App>);

impl St {
    /// A command that panicked must not lock every later one out, so a poisoned lock is reused.
    fn lock(&self) -> std::sync::MutexGuard<'_, App> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
type R<T> = Result<T, String>;

/// Runs an action so that an internal error (a panic) becomes an error message instead of closing
/// the app. Edits work on a copy of the project, so the project is left as it was.
fn guard<T>(f: impl FnOnce() -> R<T>) -> R<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| Err("Datum Schema met an internal error and stopped that action; your drawing is as it was. Details are in crash.log; please report it (Help › Report an Issue).".into()))
}

/// What the window needs after every change: the project and the document's state.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Out {
    p: Project,
    path: Option<String>,
    dirty: bool,
    undo: bool,
    redo: bool,
    created: Vec<u32>,
    /// What a damaged file lost or changed when it was opened.
    warnings: Vec<String>,
}

fn out(a: &App) -> Out {
    Out { p: a.doc.p.clone(), path: a.doc.path.clone(), dirty: a.doc.dirty, undo: a.doc.can_undo(), redo: a.doc.can_redo(), created: vec![], warnings: vec![] }
}

fn dir(app: &AppHandle, data: bool) -> Option<PathBuf> {
    let d = if data { app.path().app_data_dir() } else { app.path().app_config_dir() }.ok()?;
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// A new project gets the user's defaults: units, company and project settings.
fn fresh(a: &App, mut p: Project) -> Project {
    if p.items.is_empty() && (a.opts.units == "SI" || a.opts.units == "US") {
        p.units = a.opts.units.clone();
    }
    if p.title.company.is_empty() {
        p.title.company = a.opts.company.clone();
    }
    if let (Some(s), true) = (&a.opts.defaults, p.items.is_empty()) {
        p.settings = s.clone();
    }
    // The company datasheet layout and title block go with every new project, examples too.
    if let Some(s) = &a.opts.defaults {
        p.settings.ds_layout = s.ds_layout.clone();
        if s.title_block.on {
            p.settings.title_block = s.title_block.clone();
        }
    }
    p
}

// ---------------------------------------------------------------- the project

#[tauri::command]
fn state(st: State<St>) -> Out {
    out(&st.lock())
}

#[tauri::command]
fn apply(st: State<St>, op: Op) -> R<Out> {
    guard(|| {
        let mut a = st.lock();
        let created = a.doc.apply(op)?;
        let mut o = out(&a);
        o.created = created;
        Ok(o)
    })
}

/// Who is editing: the name in Options, else the computer's user name.
fn user_name(o: &Options) -> String {
    if !o.user.trim().is_empty() {
        return o.user.trim().to_string();
    }
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "unknown".into())
}

/// The recorded changes of one object (or, with id 0, of the whole project), newest first.
#[tauri::command]
fn changes(st: State<St>, id: u32) -> Vec<doc::Change> {
    let a = st.lock();
    let obj = a.doc.p.obj_id(id).unwrap_or(0);
    a.doc.history.iter().rev().filter(|c| id == 0 || c.obj == obj).take(500).cloned().collect()
}

/// What importing a workbook of datasheets would change (nothing changes until the window applies
/// the plan as one edit).
#[tauri::command]
async fn import_datasheets(app: AppHandle, st: State<'_, St>) -> R<Option<datum_core::import::Plan>> {
    let Some(path) = pick(&app, "Excel workbook", &["xlsx"]) else { return Ok(None) };
    let len = std::fs::metadata(&path).map_err(|e| format!("Could not open {}: {e}", path.display()))?.len();
    if len > doc::MAX_FILE {
        return Err(format!("{} is too large to read.", path.display()));
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    guard(|| datum_core::import::plan(&st.lock().doc.p, &bytes)).map(Some)
}

/// The cause-and-effect matrix of the project's interlocks.
#[tauri::command]
fn cause_effect(st: State<St>) -> R<datum_core::cause::Matrix> {
    guard(|| Ok(datum_core::cause::matrix(&st.lock().doc.p)))
}

/// Every instrument loop with two or more members.
#[tauri::command]
fn loops(st: State<St>) -> R<Vec<datum_core::loops::LoopInfo>> {
    guard(|| Ok(datum_core::loops::all(&st.lock().doc.p)))
}

/// The loop diagram of a loop ("L-101", or an object's id).
#[tauri::command]
fn loop_diagram(st: State<St>, key: String) -> R<datum_core::loops::LoopView> {
    guard(|| datum_core::loops::diagram(&st.lock().doc.p, &key))
}

/// The sections and rows a datasheet layout can show, hide and order.
#[tauri::command]
fn layout_rows(st: State<St>) -> R<Vec<(String, Vec<String>)>> {
    guard(|| Ok(datasheets::layout_rows(&st.lock().doc.p)))
}

/// The datasheet of an instrument, control valve or relief device (by object or symbol id).
#[tauri::command]
fn datasheet(st: State<St>, id: u32) -> R<datasheets::Datasheet> {
    guard(|| datasheets::build(&st.lock().doc.p, id))
}

/// Every datasheet in the project, in tag order (for printing).
#[tauri::command]
fn datasheets_all(st: State<St>) -> R<Vec<datasheets::Datasheet>> {
    guard(|| Ok(datasheets::all(&st.lock().doc.p)))
}

#[tauri::command]
fn history(st: State<St>, redo: bool) -> Out {
    let mut a = st.lock();
    if redo {
        a.doc.redo();
    } else {
        a.doc.undo();
    }
    out(&a)
}

#[tauri::command]
fn derived(st: State<St>, sheet: u32) -> R<api::Derived> {
    guard(|| Ok(api::derived(&st.lock().doc.p, sheet)))
}

/// The four lists (line list, equipment, instruments, valves), as rows in display units.
#[tauri::command]
fn list(st: State<St>, kind: String) -> R<lists::List> {
    guard(|| lists::build(&st.lock().doc.p, &kind))
}

/// Every sheet's drawing, for printing.
#[tauri::command]
fn views(st: State<St>) -> R<Vec<view::SheetView>> {
    guard(|| Ok(view::all(&st.lock().doc.p)))
}

#[tauri::command]
fn examples_list() -> Vec<examples::Example> {
    examples::list()
}

/// A new project: a worked example by id, or a blank one.
#[tauri::command]
fn new_project(st: State<St>, example: Option<String>) -> R<Out> {
    let mut a = st.lock();
    let p = match example.as_deref() {
        Some(id) => examples::get(id).ok_or("That example does not exist.")?,
        None => Project::new(),
    };
    let p = fresh(&a, p);
    a.doc = Doc::new(p);
    a.doc.user = user_name(&a.opts);
    Ok(out(&a))
}

/// The symbol library, line types, the fields of every kind of thing, and the pipe sizes.
#[tauri::command]
fn library() -> api::Library {
    api::library()
}

/// How every quantity is shown in SI and US units (from the engine's unit table).
#[tauri::command]
fn unit_table() -> (Vec<units::Shown>, Vec<units::Shown>) {
    (units::shown(false), units::shown(true))
}

// ---------------------------------------------------------------- files

fn pick(app: &AppHandle, name: &str, exts: &[&str]) -> Option<PathBuf> {
    app.dialog().file().add_filter(name, exts).blocking_pick_file().and_then(|f| f.into_path().ok())
}

#[tauri::command]
async fn open(app: AppHandle, st: State<'_, St>, path: Option<String>) -> R<Option<Out>> {
    let Some(path) = path.map(PathBuf::from).or_else(|| pick(&app, "Datum Schema project", &["dschema"])) else { return Ok(None) };
    let (d, warnings) = guard(|| Doc::open(&path))?;
    remember(&app, &path);
    let mut a = st.lock();
    a.doc = d;
    a.doc.user = user_name(&a.opts);
    let mut o = out(&a);
    o.warnings = warnings;
    Ok(Some(o))
}

// ---------------------------------------------------------------- symbol libraries

use datum_core::symlib::{self, LibView, Library};

/// The folder of the user's symbol libraries.
fn lib_dir(app: &AppHandle) -> R<PathBuf> {
    dir(app, true).map(|d| d.join("libraries")).ok_or_else(|| "The folder for symbol libraries could not be made.".to_string())
}

/// Every symbol library on this computer (read and made drawable).
#[tauri::command]
fn libraries(app: AppHandle) -> R<Vec<LibView>> {
    let d = lib_dir(&app)?;
    guard(|| Ok(symlib::load_dir(&d)))
}

/// Reads a client's legend sheet (DXF) into a library to review; nothing is saved yet.
#[tauri::command]
async fn legend_scan(app: AppHandle, path: Option<String>) -> R<Option<LibView>> {
    let Some(path) = path.map(PathBuf::from).or_else(|| pick(&app, "DXF drawing (save a DWG as DXF from any CAD program)", &["dxf"])) else { return Ok(None) };
    let text = String::from_utf8_lossy(&std::fs::read(&path).map_err(|e| format!("Could not open {}: {e}", path.display()))?).into_owned();
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Library").to_string();
    let scan = guard(|| symlib::scan_legend(&text, &name))?;
    let mut lib = scan.library;
    lib.source = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(Some(symlib::view("", lib, scan.warnings)))
}

/// Puts an old drawing (DXF) behind a sheet.
#[tauri::command]
async fn underlay_import(app: AppHandle, st: State<'_, St>, sheet: u32) -> R<Option<Out>> {
    let Some(path) = pick(&app, "DXF drawing (save a DWG as DXF from any CAD program)", &["dxf"]) else { return Ok(None) };
    let text = String::from_utf8_lossy(&std::fs::read(&path).map_err(|e| format!("Could not open {}: {e}", path.display()))?).into_owned();
    let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    guard(|| {
        let mut a = st.lock();
        let mut s = a.doc.p.sheets.iter().find(|s| s.id == sheet).cloned().ok_or("That sheet no longer exists.")?;
        let (u, warnings) = datum_core::dxfin::underlay(&text, &name, &s)?;
        s.underlay = Some(u);
        a.doc.apply(Op::SetSheet { sheet: s })?;
        let mut o = out(&a);
        o.warnings = warnings;
        Ok(Some(o))
    })
}

/// Takes in the review comments of another copy of the project (a reviewer's).
#[tauri::command]
async fn comments_import(app: AppHandle, st: State<'_, St>) -> R<Option<(Out, datum_core::review::Merged)>> {
    let Some(path) = pick(&app, "Datum Schema project (a reviewer's copy)", &["dschema"]) else { return Ok(None) };
    let (theirs, _, _) = datum_core::file::read(&path)?;
    guard(|| {
        let mut a = st.lock();
        let m = datum_core::review::merge(&mut a.doc.p.clone(), theirs.comments.clone());
        a.doc.apply(Op::MergeComments { comments: theirs.comments })?;
        Ok(Some((out(&a), m)))
    })
}

/// A title block on its own, drawn for the editor (with the open sheet's values).
#[tauri::command]
fn title_preview(st: State<St>, block: model::TitleBlock, sheet: u32) -> R<datum_core::frame::Preview> {
    let a = st.lock();
    let p = &a.doc.p;
    let s = p.sheet(sheet).or(p.sheets.first()).ok_or("There is no sheet.")?;
    Ok(datum_core::frame::preview(p, s, &block))
}

/// The standard title block, to start a company template from.
#[tauri::command]
fn title_standard() -> model::TitleBlock {
    datum_core::frame::standard()
}

/// What has changed on a sheet since it was issued at a revision.
#[tauri::command]
fn sheet_compare(st: State<St>, sheet: u32, rev: String) -> R<datum_core::revs::Compare> {
    let a = st.lock();
    let s = a.doc.p.sheet(sheet).ok_or("That sheet no longer exists.")?;
    datum_core::revs::compare(&a.doc.p, s, &rev)
}

/// The old drawing behind a sheet, as SVG in its own millimetres.
#[tauri::command]
fn underlay_svg(st: State<St>, sheet: u32) -> String {
    let a = st.lock();
    a.doc.p.sheets.iter().find(|s| s.id == sheet).and_then(|s| s.underlay.as_ref()).map(|u| datum_core::symbols::svg(&datum_core::symbols::parse(&u.draw))).unwrap_or_default()
}

#[tauri::command]
fn library_save(app: AppHandle, lib: Library, old: Option<String>) -> R<LibView> {
    let d = lib_dir(&app)?;
    guard(|| symlib::save(&d, lib, old.as_deref()))
}

#[tauri::command]
fn library_delete(app: AppHandle, file: String) -> R<()> {
    if file.contains(['/', '\\']) || !file.ends_with(".dschemalib") {
        return Err("That is not a library file.".into());
    }
    std::fs::remove_file(lib_dir(&app)?.join(file)).map_err(|e| format!("Could not delete the library: {e}"))
}

/// Saves a copy of a library file elsewhere (to send to a colleague).
#[tauri::command]
async fn library_export(app: AppHandle, file: String) -> R<Option<String>> {
    if file.contains(['/', '\\']) {
        return Err("That is not a library file.".into());
    }
    let src = lib_dir(&app)?.join(&file);
    let Some(to) = app.dialog().file().add_filter("Datum Schema symbol library", &["dschemalib"]).set_file_name(&file).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    std::fs::copy(&src, &to).map_err(|e| format!("Could not save {}: {e}", to.display()))?;
    Ok(Some(to.display().to_string()))
}

/// Adds a library file someone sent to this computer's libraries.
#[tauri::command]
async fn library_import(app: AppHandle) -> R<Option<LibView>> {
    let Some(path) = pick(&app, "Datum Schema symbol library", &["dschemalib"]) else { return Ok(None) };
    let lib = guard(|| symlib::read_file(&path))?;
    let d = lib_dir(&app)?;
    guard(|| symlib::save(&d, lib, None)).map(Some)
}

/// A library symbol made to stand for another of our symbols, its ports suggested afresh.
#[tauri::command]
fn symbol_like(sym: symlib::CustomSym, like: String) -> symlib::CustomSym {
    symlib::restyle(sym, &like)
}

/// Where a connection point clicked on a symbol goes: onto its drawing, leaving the nearer side.
#[tauri::command]
fn port_at(draw: String, w: f64, h: f64, x: f64, y: f64) -> symlib::CPort {
    symlib::port_at(&draw, w, h, x, y)
}

/// Opens a DEXPI P&ID exchange file as a new, unsaved project (Save keeps it as a .dschema).
#[tauri::command]
async fn import_dexpi(app: AppHandle, st: State<'_, St>, path: Option<String>) -> R<Option<Out>> {
    let Some(path) = path.map(PathBuf::from).or_else(|| pick(&app, "DEXPI P&ID exchange file", &["xml"])) else { return Ok(None) };
    let text = doc::read_limited(&path)?;
    let (mut p, warnings) = guard(|| datum_core::dexpi::read(&text))?;
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        p.name = datum_core::clip(stem, 200);
    }
    let mut a = st.lock();
    let p = fresh(&a, p);
    a.doc = Doc::new(p);
    a.doc.user = user_name(&a.opts);
    a.doc.dirty = true;
    let mut o = out(&a);
    o.warnings = warnings;
    Ok(Some(o))
}

#[tauri::command]
async fn save(app: AppHandle, st: State<'_, St>, as_new: bool) -> R<Option<Out>> {
    let (known, name) = {
        let a = st.lock();
        (a.doc.path.clone().filter(|_| !as_new), a.doc.p.name.clone())
    };
    let path = match known {
        Some(p) => PathBuf::from(p),
        None => match app.dialog().file().add_filter("Datum Schema project", &["dschema"]).set_file_name(format!("{name}.dschema")).blocking_save_file() {
            Some(f) => f.into_path().map_err(|e| e.to_string())?,
            None => return Ok(None),
        },
    };
    let mut a = st.lock();
    a.doc.save(&path)?;
    remember(&app, &path);
    clear_recovery(&app);
    Ok(Some(out(&a)))
}

/// What was sent to another app: where it was saved, and what was assumed.
#[derive(Serialize)]
struct SendOut {
    path: String,
    notes: Vec<String>,
}

/// Sends a line to Datum Pipe (`kind` "pipe", `what` a line's id) or a loop to Datum Sim ("sim",
/// a loop's key) as a file of that app.
#[tauri::command]
async fn send(app: AppHandle, st: State<'_, St>, kind: String, what: String) -> R<Option<SendOut>> {
    let s = guard(|| {
        let a = st.lock();
        match kind.as_str() {
            "pipe" => datum_core::links::to_pipe(&a.doc.p, what.parse().map_err(|_| "Choose a process line.".to_string())?),
            "sim" => datum_core::links::to_sim(&a.doc.p, &what),
            k => Err(format!("Unknown app {k}.")),
        }
    })?;
    let (label, ext) = if kind == "pipe" { ("Datum Pipe model", "dpipe") } else { ("Datum Sim model", "dsim") };
    let Some(path) = app.dialog().file().add_filter(label, &[ext]).set_file_name(&s.file).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    doc::write_atomic(&path, s.text.as_bytes())?;
    Ok(Some(SendOut { path: path.display().to_string(), notes: s.notes }))
}

/// Saves a picture or a file the window made (SVG text, PNG data URL, CSV text).
#[tauri::command]
async fn save_file(app: AppHandle, name: String, data: String) -> R<Option<String>> {
    let ext = name.rsplit('.').next().unwrap_or("txt").to_string();
    let Some(path) = app.dialog().file().add_filter(ext.to_uppercase(), &[&ext]).set_file_name(&name).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let bytes = match data.strip_prefix("data:").and_then(|d| d.split_once(";base64,")) {
        Some((_, b64)) => datum_core::base64_decode(b64)?,
        None => data.into_bytes(),
    };
    doc::write_atomic(&path, &bytes)?;
    Ok(Some(path.display().to_string()))
}

/// Exports a list (CSV or Excel) or the drawing (DXF) chosen by `kind`.
#[tauri::command]
async fn export(app: AppHandle, st: State<'_, St>, kind: String, what: String, sheet: u32) -> R<Option<String>> {
    let name = st.lock().doc.p.name.clone();
    if kind == "dexpi" && st.lock().doc.p.sheet(sheet).is_some_and(|s| s.pfd) {
        return Err(datum_core::dexpi::NOT_PFD.into());
    }
    let (label, ext) = match kind.as_str() {
        "csv" => ("CSV", "csv"),
        "xlsx" | "datasheets" | "cause" => ("Excel workbook", "xlsx"),
        "dexpi" => ("DEXPI P&ID exchange file", "xml"),
        "loopdxf" => ("DXF drawing", "dxf"),
        "dxf" => ("DXF drawing", "dxf"),
        k => return Err(format!("Unknown export type {k}.")),
    };
    let file = if kind == "dxf" { format!("{name}.dxf") } else if kind == "datasheets" { format!("{name} datasheets.xlsx") } else if kind == "cause" { format!("{name} cause and effect.xlsx") } else if kind == "loopdxf" { format!("{name} loop {what}.dxf") } else if kind == "dexpi" { format!("{name}.xml") } else { format!("{name} {what}.{ext}") };
    let Some(path) = app.dialog().file().add_filter(label, &[ext]).set_file_name(file).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let bytes = guard(|| {
        let a = st.lock();
        let p = &a.doc.p;
        match kind.as_str() {
            "csv" => Ok(lists::csv(&lists::build(p, &what)?).into_bytes()),
            "xlsx" => lists::xlsx(p, &what),
            "datasheets" => datasheets::xlsx(p),
            "cause" => datum_core::cause::xlsx(p),
            "loopdxf" => datum_core::loops::diagram(p, &what).map(|d| dxf::write_prims(&d.prims, d.w, d.h).into_bytes()),
            "dexpi" => {
                let s = p.sheet(sheet).or(p.sheets.first()).ok_or("There is no sheet.")?;
                Ok(datum_core::dexpi::write(p, s).into_bytes())
            }
            _ => {
                let s = p.sheet(sheet).or(p.sheets.first()).ok_or("There is no sheet.")?;
                Ok(dxf::write(p, s).into_bytes())
            }
        }
    })?;
    doc::write_atomic(&path, &bytes)?;
    Ok(Some(path.display().to_string()))
}


// ---------------------------------------------------------------- options, recovery, recent

fn options_file(app: &AppHandle) -> Option<PathBuf> {
    dir(app, false).map(|d| d.join("options.json"))
}

#[tauri::command]
fn options(st: State<St>) -> Options {
    st.lock().opts.clone()
}

#[tauri::command]
fn set_options(app: AppHandle, st: State<St>, opts: Options) -> Out {
    if let Some(f) = options_file(&app) {
        let _ = doc::write_atomic(&f, serde_json::to_string(&opts).unwrap_or_default().as_bytes());
    }
    let mut a = st.lock();
    a.opts = opts;
    a.doc.user = user_name(&a.opts);
    out(&a)
}

#[derive(Serialize, Deserialize)]
struct Recovery {
    name: String,
    path: Option<String>,
    saved: String,
}

fn recovery_files(app: &AppHandle) -> Option<(PathBuf, PathBuf)> {
    dir(app, true).map(|d| (d.join("recovery.dschema"), d.join("recovery.json")))
}

fn clear_recovery(app: &AppHandle) {
    if let Some((a, b)) = recovery_files(app) {
        let _ = std::fs::remove_file(a);
        let _ = std::fs::remove_file(b);
    }
}

#[tauri::command]
fn autosave(app: AppHandle, st: State<St>) {
    let a = st.lock();
    if let (true, Some((file, meta))) = (a.doc.dirty, recovery_files(&app)) {
        let info = Recovery { name: a.doc.p.name.clone(), path: a.doc.path.clone(), saved: chrono::Local::now().format("%d-%b-%y %H:%M").to_string() };
        let _ = doc::write_atomic(&file, model::to_json(&a.doc.p).as_bytes());
        let _ = doc::write_atomic(&meta, serde_json::to_string(&info).unwrap_or_default().as_bytes());
    }
}

#[tauri::command]
fn recovery(app: AppHandle) -> Option<Recovery> {
    let (file, meta) = recovery_files(&app)?;
    file.exists().then_some(())?;
    serde_json::from_str(&doc::read_limited(&meta).ok()?).ok()
}

#[tauri::command]
fn recover(app: AppHandle, st: State<St>, keep: bool) -> R<Option<Out>> {
    let info = recovery(app.clone());
    let files = recovery_files(&app);
    if !keep {
        clear_recovery(&app);
        return Ok(None);
    }
    let (file, _) = files.ok_or("No recovered drawing was found.")?;
    let (p, warnings) = guard(|| model::from_json(&doc::read_limited(&file)?))?;
    let mut a = st.lock();
    a.doc = Doc::new(p);
    a.doc.user = user_name(&a.opts);
    a.doc.path = info.and_then(|i| i.path);
    a.doc.dirty = true;
    let mut o = out(&a);
    o.warnings = warnings;
    Ok(Some(o))
}

fn recent_paths(app: &AppHandle) -> Vec<String> {
    options_file(app).map(|f| f.with_file_name("recent.json")).and_then(|f| doc::read_limited(&f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn remember(app: &AppHandle, path: &Path) {
    let p = path.display().to_string();
    let mut v = recent_paths(app);
    v.retain(|x| *x != p);
    v.insert(0, p);
    v.truncate(12);
    if let Some(f) = options_file(app).map(|f| f.with_file_name("recent.json")) {
        let _ = doc::write_atomic(&f, serde_json::to_string(&v).unwrap_or_default().as_bytes());
    }
}

#[derive(Serialize)]
struct Recent {
    path: String,
    name: String,
    modified: String,
}

#[tauri::command]
fn recent(app: AppHandle) -> Vec<Recent> {
    recent_paths(&app)
        .into_iter()
        .filter_map(|p| {
            let m = std::fs::metadata(&p).ok()?.modified().ok()?;
            let modified = chrono::DateTime::<chrono::Local>::from(m).format("%d-%b-%y %H:%M").to_string();
            let name = Path::new(&p).file_stem()?.to_string_lossy().into_owned();
            Some(Recent { path: p, name, modified })
        })
        .collect()
}

#[tauri::command]
fn startup_file(app: AppHandle, st: State<St>) -> Option<String> {
    let last = || if st.lock().opts.open_last { recent_paths(&app).into_iter().find(|p| Path::new(p).exists()) } else { None };
    std::env::args().skip(1).find(|a| a.to_lowercase().ends_with(".dschema")).or_else(last)
}

/// Opens one of the project's GitHub pages or the family site in the browser (only those).
#[tauri::command]
fn open_url(url: String) -> R<()> {
    let ok = ["https://github.com/Edgar-Mendonca/Datum-Schema", "https://edgar-mendonca.github.io/Datum"].iter().any(|p| url.starts_with(p)) && url.chars().all(|c| c.is_ascii_alphanumeric() || "-._~:/?#[]@!$&'()*+,;=%".contains(c));
    if !ok {
        return Err("Only the Datum GitHub pages can be opened.".into());
    }
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(&url).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&url).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
fn quit(app: AppHandle) {
    clear_recovery(&app);
    app.exit(0);
}

#[tauri::command]
async fn check_update(st: State<'_, St>) -> R<update::Info> {
    let beta = st.lock().opts.beta;
    tauri::async_runtime::spawn_blocking(move || update::check(beta)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn install_update(app: AppHandle, url: String, sums: Option<String>) -> R<()> {
    tauri::async_runtime::spawn_blocking(move || update::install(&app, &url, sums.as_deref())).await.map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Anything that ever panics is written to crash.log (version, place, message) for an issue report.
            if let Some(log) = dir(app.handle(), true).map(|d| d.join("crash.log")) {
                let default = std::panic::take_hook();
                std::panic::set_hook(Box::new(move |info| {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log) {
                        let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
                        let what = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
                        let _ = writeln!(f, "{} Datum Schema {} panicked at {at}: {what}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), env!("CARGO_PKG_VERSION"));
                    }
                    default(info);
                }));
            }
            // The user's symbol libraries are drawable from the start (projects may use them).
            if let Ok(d) = lib_dir(app.handle()) {
                let _ = std::panic::catch_unwind(|| symlib::load_dir(&d));
            }
            let opts: Options = options_file(app.handle()).and_then(|f| doc::read_limited(&f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            let mut a = App { doc: Doc::new(Project::new()), opts };
            a.doc.user = user_name(&a.opts);
            let p = fresh(&a, Project::new());
            let mut doc = Doc::new(p);
            doc.user = a.doc.user.clone();
            app.manage(St(Mutex::new(App { doc, ..a })));
            Ok(())
        })
        .on_window_event(|w, e| {
            if let WindowEvent::CloseRequested { api, .. } = e {
                if w.state::<St>().lock().doc.dirty {
                    api.prevent_close();
                    let _ = w.emit("close-requested", ());
                } else {
                    clear_recovery(w.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            state, apply, history, changes, datasheet, datasheets_all, import_datasheets, loops, loop_diagram, layout_rows, cause_effect, import_dexpi, libraries, legend_scan, library_save, library_delete, library_export, library_import, port_at, symbol_like, underlay_import, underlay_svg, send, sheet_compare, title_preview, title_standard, comments_import, derived, list, views, examples_list, new_project, library, unit_table, open, save, save_file, export,
            options, set_options, autosave, recovery, recover, recent, startup_file, open_url, version, quit, check_update, install_update
        ])
        .run(tauri::generate_context!())
        .expect("Datum Schema failed to start");
}
