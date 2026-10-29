//! Datum Schema desktop shell: owns the open project, files, options and recovery, and talks to
//! the window. All calculation is in `datum-core`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod update;

use datum_core::doc::{self, Doc, Op};
use datum_core::model::{self, Project};
use datum_core::{api, dxf, examples, lists, units, view};
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
    /// Project settings (line number format, checks switched off, hydraulic limits) for new projects.
    defaults: Option<model::Settings>,
}

impl Default for Options {
    fn default() -> Self {
        Self { autosave: 5, beta: false, theme: "auto".into(), open_last: false, units: "SI".into(), grid: true, paper: "sheet".into(), company: String::new(), defaults: None }
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

/// Saves a picture or a file the window made (SVG text, PNG data URL, CSV text).
#[tauri::command]
async fn save_file(app: AppHandle, name: String, data: String) -> R<Option<String>> {
    let ext = name.rsplit('.').next().unwrap_or("txt").to_string();
    let Some(path) = app.dialog().file().add_filter(ext.to_uppercase(), &[&ext]).set_file_name(&name).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let bytes = match data.strip_prefix("data:").and_then(|d| d.split_once(";base64,")) {
        Some((_, b64)) => base64_decode(b64)?,
        None => data.into_bytes(),
    };
    doc::write_atomic(&path, &bytes)?;
    Ok(Some(path.display().to_string()))
}

/// Exports a list (CSV or Excel) or the drawing (DXF) chosen by `kind`.
#[tauri::command]
async fn export(app: AppHandle, st: State<'_, St>, kind: String, what: String, sheet: u32) -> R<Option<String>> {
    let name = st.lock().doc.p.name.clone();
    let (label, ext) = match kind.as_str() {
        "csv" => ("CSV", "csv"),
        "xlsx" => ("Excel workbook", "xlsx"),
        "dxf" => ("DXF drawing", "dxf"),
        k => return Err(format!("Unknown export type {k}.")),
    };
    let file = if kind == "dxf" { format!("{name}.dxf") } else { format!("{name} {what}.{ext}") };
    let Some(path) = app.dialog().file().add_filter(label, &[ext]).set_file_name(file).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let bytes = guard(|| {
        let a = st.lock();
        let p = &a.doc.p;
        match kind.as_str() {
            "csv" => Ok(lists::csv(&lists::build(p, &what)?).into_bytes()),
            "xlsx" => lists::xlsx(p, &what),
            _ => {
                let s = p.sheet(sheet).or(p.sheets.first()).ok_or("There is no sheet.")?;
                Ok(dxf::write(p, s).into_bytes())
            }
        }
    })?;
    doc::write_atomic(&path, &bytes)?;
    Ok(Some(path.display().to_string()))
}

fn base64_decode(s: &str) -> R<Vec<u8>> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        acc = acc << 6 | val(c).ok_or("The picture data is damaged.")? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
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
            let opts: Options = options_file(app.handle()).and_then(|f| doc::read_limited(&f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            let a = App { doc: Doc::new(Project::new()), opts };
            let p = fresh(&a, Project::new());
            app.manage(St(Mutex::new(App { doc: Doc::new(p), ..a })));
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
            state, apply, history, derived, list, views, examples_list, new_project, library, unit_table, open, save, save_file, export,
            options, set_options, autosave, recovery, recover, recent, startup_file, open_url, version, quit, check_update, install_update
        ])
        .run(tauri::generate_context!())
        .expect("Datum Schema failed to start");
}
