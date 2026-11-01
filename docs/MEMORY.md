# Datum Schema: project memory

> **Paste this file at the start of each new session**, or just open the repo: `CLAUDE.md` tells Claude to read it.
> It records what exists, why, and what comes next. Update it at the end of every session.

| | |
| --- | --- |
| **Product** | Datum Schema: intelligent P&IDs and process flow diagrams: draw, check and list (symbols and lines that carry their data and connections; live line list, equipment list, instrument index and valve list; drawing checks; line hydraulics; print, SVG and DXF) |
| **Current version** | 0.5.2 (2026-11-01) |
| **Repo** | `Edgar-Mendonca/Datum-Schema` · branch `claude/vigilant-hamilton-ku7r0w` (the first branch pushed, so the default branch) |
| **Stack** | Rust engine (`crates/core`: model, symbols, graph, tags, checks, hydraulics, lists, frame, routing, DXF), Tauri 2, SolidJS + strict TypeScript; no Python |
| **Identifier** | `com.edgarmendonca.datumschema` · files `.dschema` (SQLite, `format` 4 from 0.2.0 (items.base, `underlays` and `snapshots` tables, meta "symbols"); 3 from 0.1.9; 2 before: no `issues` table; JSON `format` 1 from 0.1.0–0.1.4 converts) · symbol libraries `.dschemalib` (JSON, in app data › libraries) · installer `DatumSchema-x.y.z-setup.exe` + `SHA256SUMS.txt` · publisher "Datum" |
| **Accent** | Magenta `#9C1F84` (`--blue2` `#801A6C`, `--sky` `#EAB6DD`, `--pale` `#FAEBF6`, `--sel` `#F4DCEE`, `--sel2` `#E7B9DB`; dark `#E07BC9`; File sidebar `#4E1043`) |
| **Brief** | `Edgar-Mendonca/Datum` › `briefs/DATUM-SCHEMA.md` (hub session 22): every decision made up front |
| **Reference repos** | `Edgar-Mendonca/Datum` (FAMILY, MEMORY, brain), `Edgar-Mendonca/Datum-Plan` (shell, updater, print, Excel), `Edgar-Mendonca/Datum-Sim` (canvas, router), `Edgar-Mendonca/Datum-Calc` (units, pipe pressure-drop pack), `Edgar-Mendonca/Datum-Pipe` (pipe sizes; its shell is the newest copy of Plan's) |

---

## 1. Architecture

`crates/core` is the engine (pure Rust, no Tauri): the project and its edits, the symbol library, connectivity, tags, checks, hydraulics, lists, the sheet frame, line routing and the DXF writer. `src-tauri` is the shell: commands, files, options, recovery and the updater. `ui` is the window: it draws what the engine returns (`view.rs`: routed lines with hops, arrows and marks, placed symbols with upright texts, the frame) and sends edits as operations.

Coordinates are millimetres on the sheet, y down, so a sheet prints at its true size. Engineering values are SI base units in the file (Pa, K, m³/s, kg/s, kg/m³, Pa·s, m, W); the engine's display table (`units::shown`) gives SI (bar, °C, m³/h, kg/h, kg/m³, cP, m, m/s, kPa, kPa/100 m, kW) and US (psi, °F, gpm, lb/h, lb/ft³, cP, ft, ft/s, psi, psi/100 ft, hp); the window only applies it.

### Files

| File | What it does |
| --- | --- |
| `crates/core/src/file.rs` | The project file: SQLite tables (meta, nodes, objects, attributes, sheets, items, lines, notes, issues (datasheet revisions with their snapshot), history, and an `object_attributes` view), atomic write, table-by-table read with repair, JSON fallback for 0.1 files and recovery copies |
| `crates/core/src/model.rs` | The project: plant objects (`Obj`: id, uid, class, tag, hierarchy node, data; line numbers are `PipingNetworkSystem` objects) and the hierarchy (`Node`: plant › unit › area); items and line pieces are views with `obj`; the 0.1 JSON migration (`migrate_1`); sheets (number, title, size A3/A1/ANSI B/ANSI D, revisions, legend flag), items (symbol, position, quarter turns, mirror, scale, tag, data, off-page partner), lines (type, ends on a port, on another line or free, bend points, flow flip, data), notes (note, leader, Hold cloud), settings (line number format, rules off, hydraulic limits, roughness, grid); `.dschema` read part by part with `normalize` (damaged files open as far as possible, with warnings), and write |
| `crates/core/src/symbols.rs` | The symbol library (about 80 of our own drawings in a small drawing language: polylines, outlines, fills, dashes, circles, arcs, ellipses, rectangles, text), ports with directions, categories, tag prefixes, in-line and terminal flags; placement (turn, mirror, scale); primitives and their SVG |
| `crates/core/classes.json`, `crates/core/src/classes.rs` | The class library (83 classes, DEXPI-style names, inheritance from PlantItem, symbols, prefixes, attributes with quantity, type, choices, DEXPI name, required) and its reader: `attrs(project, class)` merges inherited, own and project (`settings.user_attrs`) attributes |
| `crates/core/src/datasheets.rs` | Datasheets: per instrument, primary element, control valve and relief device; General, Process conditions (from `process_source`: its line, a primary element it is tapped to, the equipment it is mounted on, else its loop's measurement), the class's own attributes, Specification (`group`), company attributes; Excel with one worksheet each. Revisions: `build_in` marks rows changed (Δ) against the last `Issue` snapshot (SI display units), `next_rev` (A… before purchase, 0… from purchase on), `snapshot` |
| `crates/core/src/import.rs` | Datasheet import from Excel (calamine): `read` (sheets as text grids, capped), `plan` (match by tag; a table with a Tag header, or one datasheet per worksheet; labels by `norm`; units by `unit_expr`/`quantity`; kinds set, same, conflict, bad; unmatched tags, unknown labels). The window applies the "set" rows as one `Op::Batch` of `SetData` |
| `crates/core/src/loops.rs` | Loop diagrams: `all` (loops of ≥ 2 members by first letter and number, relief devices left out), `diagram` (A3 sheet with the frame: field devices with their own symbols, JB terminal block, I/O box, control-system functions; wiring from the "Wiring" attributes `jb`, `jb_terminals`, `cable`, `system`, `io`; a To complete list) |
| `crates/core/src/cause.rs` | Cause and effect: `matrix` (each interlock symbol: the items signalled to it, effects = valves, drives, equipment, final elements; the rest are causes; a switch wired straight to a final element is its own group, cell "X"); actions from the fail position (FC close, FO open, FL hold), drives stop, `trip_action` overrides; `xlsx` |
| `crates/core/src/dexpi.rs` | DEXPI (Proteus XML) import (`read`: element tree with limits, units and fit-to-sheet, symbols from DatumSchemaSymbol, the class or a stand-in, nozzles matched by position, segments as pieces From → components → To, branches onto segments, flows as signal or impulse lines, attributes back to keys in SI) and export of a sheet: Equipment with Nozzles, PipingNetworkSystem per line-number object, PipingNetworkSegment per piece with CenterLine, PipingComponent (each once) and Connection; ProcessInstrumentationFunction; InformationFlow for signal and impulse lines; GenericAttributes (DEXPI names in SI, the rest in DatumSchemaAttributes); y up |
| `crates/core/src/dxfin.rs` | The DXF reader: group-code pairs, BLOCKS and ENTITIES (LINE, LWPOLYLINE/POLYLINE with bulges, CIRCLE, ARC, ELLIPSE, TEXT, MTEXT stripped of codes, SOLID and solid HATCH, INSERT with ATTRIBs, nested to depth 8), $INSUNITS, limits; `flatten` to primitives in mm y-down; `underlay` (an old drawing scaled to fit the drawing area, never enlarged) |
| `crates/core/src/symlib.rs` | Client symbol libraries: `scan_legend` (blocks and the text beside each insert; `suggest` by word overlap with synonyms, `draft` size, `ports_like` snapped onto the drawing), `validate`/`clean`, `register` (global registry of leaked `Sym`s, keys `x.<lib>.<block>`), `save`/`load_dir`/`read_file`, `restyle`, `port_at`, `check_copies` for the copies a project carries |
| `crates/core/src/links.rs` | Datum Pipe (`to_pipe`: a run as a `.dpipe` straight model between two anchors with its valves as rigid elements, size, schedule, design T/P, density, length) and Datum Sim (`to_sim`: a loop as a `.dsim` with Step, Sum, PID, valve, process (integrating for level, with an outflow), transmitter and Scope; dynamics as model variables) |
| `crates/core/src/pfd.rs` | Process flow diagrams: the stream table on the sheet (`table`, bands left of the title block), checks 14 (mass) and 15 (heat, duty or shaft power) round each item, and 16 (a P&ID line's `stream` against the stream's mass flow) |
| `crates/core/src/instruments.rs` | `parse_range` (text ranges in their units, or the project's), checks 18 (range covers the line's operating value via `datasheets::process_source`), 19 (control valve fail position), 20 (loop wiring finished once started); `io_of` and `io_summary` (AI/AO/DI/DO/fieldbus per control system) |
| `crates/core/src/review.rs` | Review comments (`Comment`, `Reply` with uids; ops Comment, Reply, Resolve, EditComment, MoveComment, DeleteComment, MergeComments; `merge` by uid with the newest status winning; `normalize`; `pins` for the view; `when` dates) |
| `ui/src/guide.tsx` | The user guide: chapters of help topics (`CHAPTERS`, the rest under "More"), the rules, `SHORTCUTS`, `GLOSSARY` and `GettingStarted` from `dialogs.tsx`; search; A4 print through `#printroot` |
| `ui/src/review.tsx` | The Comment tool's dialog, the Comments panel, Import Review |
| `crates/core/src/propagate.rs` | Values passed along (`run`, from `normalize`: line size and class → component "size", "pipe_class", remembered in hidden "~" keys so typed values are kept) and `todo` (overrides, datasheets and sheets changed since issue); `hidden` keys skipped by history and DEXPI |
| `ui/src/todo.tsx` | The To do tab |
| `crates/core/src/rules.rs` | Company rules (`settings.rules`): tests required, equals, not equals, one of, contains, at least, at most on one attribute of a class and its subclasses; check 24 |
| `crates/core/examples/bench_big.rs` | A 120-sheet project: an edit, the open sheet, lists, To do, save and open timed |
| `ui/src/rules.tsx` | Options › Company rules and Options › Tags |
| `crates/core/src/wiring.rs` | Instrument wiring: JunctionBox, Cable and IOCard objects (classes under WiringItem, added by `Op::AddObject`); devices name them in "jb", "jb_terminals", "cable", "io" ("<card>/<channel>", `channel`); `cables`, `allocation` (lists cables, ioalloc, jbs) and check 25 |
| `ui/src/wiring.tsx` | Adding junction boxes, cables and cards; the wiring tabs |
| `crates/core/src/sizing.rs` | Sizing: `cv_liquid`, `cv_gas` (IEC 60534-2-1), `orifice_c/flow/beta` (ISO 5167-2 corner taps), `api_c`, `psv_gas`, `psv_liquid`, `api526`; `size(p,item)` reads the device and its lines, names what is missing, lists what is assumed, and rejects non-physical results; `shown` for the Inspector (command `sizing`) |
| `crates/core/src/calc.rs`, `crates/core/calcpacks/*.dcalc` | Datum Calc sheets: `sheets_for` (by class chain), `to_calc` (inputs filled with their source, kept ones listed, `NEEDS` refuses a sheet without its essential inputs) |
| `crates/core/src/specs.rs` | Pipe specs (`PipeSpec`, `SpecItem` in `settings.specs`): `normalize`, `find` (case-insensitive), `item_class_size` (terminals by their own size), `entry`, checks 21–23, `import_csv`, `starter` (CS 150#) |
| `ui/src/specs.tsx` | Options › Pipe specs: classes, limits, component table, CSV, files |
| `crates/core/src/revs.rs` | Sheet revisions: `snapshot` (items, lines with routes, notes, objects' tag and data), `compare` (added, removed, changed: moved, rerouted, symbol, tag, data; gathered into areas), `cloud` (scallops and the triangle), `clouds` for the view, `next_rev` |
| `crates/core/src/units.rs` | Calc's unit table (NIST SP 811) with unit expressions and Schema's display table |
| `crates/core/src/pipes.rs` | Pipe's sizes ½"–24" and schedules (ASME B36.10M/B36.19M dimensions); inside diameters |
| `crates/core/src/doc.rs` | The history (`Change`: who, when, what, object, old, new; recorded by diffing objects after every edit, undo and redo). Edit operations (NewLineObject, SetNode, AddNode, SetNode2, DeleteNode, add, set, move, rotate, mirror, delete with merge of in-line items, set data, tags, bend points, split, paste with the next free tags, pair connectors, sheets, title, project, settings, renumber, batch) on a copy, kept only if `normalize` finds nothing to repair; undo/redo; size-limited reads and atomic writes |
| `crates/core/src/graph.rs` | Connectivity: port positions, line numbers from the format, the network (lines per port, branches), runs (one line number through in-line items and off-page pairs), walks along the piping (`reach`), from/to, pump suction/discharge |
| `crates/core/src/tags.rs` | Tags: split, next free per prefix and sheet, copies (P-101A → P-101B), renumber, duplicates, ISA-5.1 letters, controllers, measurements, final elements |
| `crates/core/src/route.rs` | Sim's orthogonal router, extended to ports facing four directions and open ends |
| `crates/core/src/view.rs` | What the window draws for a sheet: items with ports and upright texts (tags, instrument letters and loop numbers, fail positions, connector partners), lines (route, pieces with hops, arrows or signal marks, line number, hydraulic badge), notes and Hold clouds, the legend |
| `crates/core/src/frame.rs` | The sheet frame: border, zones, title block (drawn from cells: `standard()` or `settings.title_block`; `value` of each field; `logo` place; `preview` for the editor; `reserved(p, s)`), revision table spread over the block's width, as primitives |
| `ui/src/titleblock.tsx` | Options › Title block: the cell editor, the engine's preview, save a copy, load, use for new projects |
| `crates/core/src/hydraulics.rs` | Inside diameter, velocity, Reynolds, Colebrook (or 64/Re), Δp per 100 m and over length + fittings, limits by service, status |
| `crates/core/src/checks.rs` (+ `checks/tests.rs`) | The 23 checks (`RULES`; P&ID rules skip PFD sheets; 14–17 in `pfd.rs`, 18–20 in `instruments.rs`, 21–23 in `specs.rs`), one function per rule, each issue with level, reason, ids and place; a passing and a failing test per rule |
| `crates/core/src/lists.rs` | The line list (one row per line number: from/to, hydraulics), equipment list, instrument index, valve list; CSV with a formula guard; Excel (rust_xlsxwriter) |
| `crates/core/src/dxf.rs` | The DXF writer (R12: layers per line type and symbol category, blocks with a TAG attribute, text, frame) and `read_back` for the tests |
| `crates/core/src/examples.rs` | The examples built through the edit operations: pump transfer, compressor package, exchanger loop, Find the faults (tests: 0 errors and warnings; exactly the planted faults) |
| `crates/core/src/hardening.rs` | FAMILY §11: random projects and edits, failed edits change nothing, damaged files, hostile values, speed gate (500 symbols) |
| `crates/core/src/api.rs` | What the shell's commands return (`derived`: the sheet's view, issues, hydraulics, runs, numbers; `library`), shared with the test server |
| `crates/core/examples/serve.rs` | The test server: the window and the engine behind one local HTTP port, answering the shell's commands, for the UI tests |
| `src-tauri/src/main.rs` | Commands: state, apply, history, derived, list, views, examples, new project, library, unit table, open/save, save file, export (CSV, Excel, DXF), options, recovery, recent, startup file, open_url, version, updates; `guard`, crash.log, atomic saves |
| `src-tauri/src/update.rs` | The updater (Plan's, `REPO` = Datum-Schema) |
| `ui/src/main.tsx` | Window layout (palette, sheet with its tab bar, inspector, bottom panel), status bar, keys, start-up and recovery |
| `ui/src/store.ts` | Window state: the project, the open sheet, the engine's derived data, the library, the selection, the drawing tool, layers, panel, menus, dialogs |
| `ui/src/canvas.tsx` | The drawing canvas (from Sim's): pan, zoom, snap, marquee, move, draw lines from ports, drag segments and ends, drop symbols (in-line ones into lines), the symbol sprite, primitives |
| `ui/src/palette.tsx` | The left panel: the symbol palette (categories, search, common instruments) and the Plant tab |
| `ui/src/plant.tsx` | The Plant panel: hierarchy tree with every object, drawn or not; drag to a sheet to draw, onto an area to move; add, rename, delete levels; delete objects |
| `ui/src/inspector.tsx` | The Inspector: item, line (with hydraulics), note, or the sheet and title block |
| `ui/src/panels.tsx` | The bottom panel: Checks and the four lists (sort, filter, group, click to find, edit, export) |
| `ui/src/ribbon.tsx`, `ui/src/cmds.ts` | Title bar, quick access, Tell Me, the ribbon; every command |
| `ui/src/backstage.tsx`, `ui/src/print.tsx`, `ui/src/roadmap.tsx` | File page (examples, export, options with the check rules and limits, updates), print (sheets at size or A3, lists, legend; the SVG writer), roadmap |
| `ui/src/help.tsx`, `ui/src/dialogs.tsx` | The `?` cards and the rule list; dialogs (save changes, recovery, add a symbol by name, pair connectors, shortcuts, getting started, glossary) |
| `ui/src/libraries.tsx` | File › Libraries (import a legend, review table, connection-point editor, share) and Options › Drawing standard |
| `ui/src/revisions.tsx` | Issue a sheet at a revision; compare with an issued revision (click to go; Cloud them) |
| `ui/src/styles.css`, `ui/src/ui.tsx`, `ui/index.html` | Plan's styles (magenta accent) and components; the icon sprite |
| `tests_ui/run.cjs` | UI checks of the built window against the real engine (through the test server) |
| `tests_ui/screens.cjs` | The four README and site screenshots (1440×900, `docs/images/`) |
| `crates/core/examples/bench.rs` | Where the time goes on a 500-symbol sheet |
| `docs/PLAN-0.1.md`, `docs/site.json`, `CHANGELOG.md` | The plan, the public summary and roadmap, the change log |

### Release flow

`release.yml` (windows-latest): checks a pushed tag matches the version, takes the notes from CHANGELOG, runs the engine tests, builds the NSIS installer, writes `SHA256SUMS.txt` and publishes the GitHub Release. Run it from the Actions tab (tag pushes are blocked from Claude sessions).

### How we work

1. The user reviews a release and sends screenshots.
2. Reproduce first.
3. Big asks get a plan file and questions; build after the answers.
4. Engine first (with a test), then the UI.
5. Check every change in the real app (the window against the real engine in Chromium; Xvfb screenshots where the shell matters).
6. Add a UI test for every bug.
7. Run all checks: clippy `-D warnings`, `cargo test` (with the hardening tests), the UI type-check and UI tests, `cargo deny check`.
8. Release.
9. Report in plain language: what changed, what was checked, what is not done.

## 2. Decisions

| Version | Decision | Why |
| --- | --- | --- |
| 0.5.1 | Calculation sheets reuse Datum Calc's own packs (copied as data into `calcpacks/`, with their references), filled by replacing the input lines' values; a filled value gets its own words (the pack's comment describes its sample) and its source; a sheet whose essential inputs are missing is refused rather than mixing sample values with the object's (the first run made three "failed" pressure-drop sheets that way) | The user asked to use the Datum family as the suites use their own tools; never link another app |
| 0.5.2 | Sizing runs live in the Inspector (the engine sizes on every change) and is written to the data only by the user ("Write to the datasheet"), never automatically: a datasheet value must be the engineer's choice. Results that are not positive and finite are refused, not shown | The window never calculates; a sizing aid checked by the engineer |
| 0.5.0 | Wiring objects are ordinary plant objects (classes JunctionBox, Cable, IOCard), not drawn; devices keep naming them in the existing text attributes, so loop diagrams and old projects keep working; the I/O channel is "<card>/<channel>" when it names a card. Check 25 speaks only once the project has wiring objects. A card channel tells nothing about the device's own kind (`io_of` ignores "AI-01/3"), so a valve on an AI card is found | No migration; free-text wiring is left alone; the check compares the device with the card |
| 0.4.4 | Tag formats (`settings.tag_format`: separator, digit-only unit code before the number, per-sheet or project numbering, digits) always keep the prefix letters first and the number last, so `tags::split`/`loop_of` and every check keep working; "<unit>-<prefix>" forms were left out for that reason | Loops group by number; a format that put digits first would break them |
| 0.4.4 | `Project::obj` is a binary search over objects kept in id order (sorted in `normalize` only when out of order), with a linear fallback; check 20 builds only loops with wiring typed; To do builds only issued datasheets | The open sheet took 1.9 s on 120 sheets; 39 ms after |
| 0.4.3 | An assembly (`settings.assemblies`) keeps its items, lines and notes round (0, 0) and copies of their objects; `Op::PlaceAssembly` reuses `paste` with those copies (tags kept where free, else bumped). Data labels (`settings.labels`: class and up to six keys) are drawn by the engine under the item's other texts, in display units, for the class and its subclasses | One paste path; labels follow the class library |
| 0.4.2 | Implied items come from rules on the pipe class (`PipeSpec.implied`: what, end type, per end, sizes, description, code), applied to each component's ends of that type (its process ports); an end type left blank is read from the description. Each flanged end counts as its own joint (a valve bolted straight to another's flange is counted twice) | Simple and explainable; good enough for a requisition estimate, checked by the engineer |
| 0.4.1 | Propagation runs in `normalize` (every edit and open), writing the line's size and class into the component's data, and remembers what it wrote in hidden "~size"/"~pipe_class" keys: a value still equal to the last written one is the engine's to change, anything else is the user's override (kept, listed on To do). The pipe class goes to "pipe_class" because a valve's "class" attribute already holds its rating (the examples' 300#). Control and relief valves, reducers and terminals are not sized from the line | Lists and datasheets show real values; typed values are never lost; no false overrides on valves sized below the line |
| 0.4.0 | Specs live in the project's settings; a component is matched by its symbol's base key (so a client symbol counts as what it stands for) and size range; a class's limits are one design pressure and temperature, not the B16.5 P-T table; checks 21–23 run only when the project has specs; vents, drains and sample points are judged by their own size | Simple, explainable checks; no false alarms in projects without specs |
| 0.3.4 | After 0.3.4 the user asked for a detailed comparison with Smart P&ID, COMOS P&ID and AVEVA Diagrams (given in chat only; commercial names stay out of public text). Agreed: not 1.0 yet; the gaps are piping rules (pipe specs, propagation, implied items, user rules), instrument depth (wiring, sizing, equipment datasheets) and teams (status, shared projects). Roadmap 0.4.x–0.6.x set from it; 3D, electrical and operations stay out of scope (meet them through DEXPI). Brainstorm again after a few releases | The user's direction |
| 0.3.4 | Family integration as the suites do it: use Datum Calc (its packs) for calculation sheets from an object's data and Datum Sim for dynamics, by writing their own files (never linking their code) | The user's suggestion; packs exist for pump power, NPSH, pipe Δp, compressor, HX LMTD, pressure vessel, tank, pipe wall and more |
| 0.3.4 | The user will share the repository of an open-source CAD project to borrow ideas or code from. Only MIT/BSD/Apache/MPL code may be copied (it becomes Schema's own, recorded in NOTICE); otherwise ideas only | Licences rule in CLAUDE.md |
| 0.3.4 | The user guide is built from the help topics (`HELP` in `help.tsx`) grouped into chapters, not written separately; a topic left out of every chapter still appears under "More", and the UI test counts the topics and rules | One source of truth: the guide, the ? cards and Tell Me never disagree |
| 0.3.3 | Review without a server: each reviewer works on a copy of the file and the comments are merged back by uid (comments and replies added, the newest resolved/reopened status wins, a missing sheet or target falls back). Comments live in `Project.comments` (meta "comments"); author from the Options name (`Doc::apply` fills `by`); pins are window overlays, never printed on sheets | Several people review one project with no shared database; a review must not reach an issued drawing |
| 0.3.2 | The standard title block is data (`frame::standard()`); a company block is the same cells in `settings.title_block`, with own fields in `title.extra` ("f:<name>") and fixed text ("text:…"). The logo is the datasheet logo, drawn by the window and print at the engine's `logo` place and left out of DXF (R12 has no images) | One drawing path for both; a company has one logo |
| 0.3.1 | The user asked to step the 0.4 roadmap as 0.3.1, 0.3.2, … (one item per release) | The user's call |
| 0.3.1 | Take-off groups by (category, type, size, line pipe class, own rating), one count per object, P&ID sheets only; size is the item's own, else its lines' (largest first, joined with ×) | A requisition counts components, not drawings; a reducer or relief valve spans two sizes |
| 0.3.0 | Compositions are mass fractions in each stream's data under "x:<component>", with the component list in `settings.components`; the class library adds them to the Stream class's attributes (group "Composition"), so the Inspector, the list and the table need no special case. A blank fraction is none of it once a stream has any composition | No schema change; the list's `Col.key` became a `String` for these columns |
| 0.3.0 | I/O kind from the I/O channel first, then the signal, then letters and symbol; local gauges, pneumatic, capillary and software links take none; spare is 20% per kind, rounded up | What an I/O summary is for: sizing cards per system |
| 0.3.0 | Check 18 reads only ranges it understands (a unit of the right quantity, or none for the project's units); anything else is left alone rather than guessed | A check must not raise false alarms on free text |
| 0.2.0 | One bigger release (0.2.0) instead of several small ones, as the user asked: "a bigger build and release at least until process flow diagrams". Built and committed in five tested stages | The user's call |
| 0.2.0 | Client symbols come from DXF, not DWG (DWG is closed; the user agreed "DXF is fine"). A library symbol keeps a `like` (the built-in it stands for); items keep `base`; every semantic test uses `Item::base_key()`, so checks, classes, tags and lists are unchanged under a drawing standard | The object stays the master; only the drawing changes |
| 0.2.0 | A project carries copies of the library symbols it uses (`custom_syms`, meta "symbols"), registered on open | A project must open on a computer without the library |
| 0.2.0 | Large per-sheet data (an old drawing's shapes, issued snapshots) live in `Arc`s with `#[serde(skip)]`, in their own file tables, never sent to the window; the window fetches the background SVG once per fingerprint (`underlay_svg`). `SetSheet` from the window keeps the engine's copies by fingerprint or revision; a forged one is dropped | The window gets the whole project after every edit, and undo keeps up to 100 copies |
| 0.2.0 | A PFD is a sheet flag; line objects on it get the `Stream` class (reconciled in `normalize`, so every creation path is covered) and the next free number. P&ID-only rules skip PFD sheets; DEXPI refuses a PFD sheet | Reuses the line machinery (runs, lists, inspector) without a second network |
| 0.2.0 | Heat balance: out − in must be zero (both sides of an exchanger drawn) or equal the item's `duty` or `power`, within 2%; mass within 0.5%. Enthalpy flow is the user's, from one datum per diagram | A check on typed numbers, not a simulation |
| 0.2.0 | The `.dpipe` and `.dsim` files were opened, analysed and run with the real Datum Pipe and Datum Sim engines from the local clones (a scratch program, deleted; nothing linked); 16 lines and 4 loops, no repairs. Level loops needed an outflow to settle | "Never link another app"; the files must still be right |
| 0.2.0 | Revision clouds compare against a chosen issued revision; the triangle shows the revision after it (the next one issued, else `next_rev`) | Works both while working towards B and on the issued B |
| 0.1.12 | The Tell Me overlay was `class="ov top"`; the title bar's `.top { background }` rule, later in the stylesheet, made it opaque and hid the window. Renamed the overlay's class to `high` (the same clash exists in Datum Plan's stylesheet but is harmless there) | The user saw the window vanish behind Ctrl+K |
| 0.1.12 | DEXPI round trip: our export writes DatumSchemaSymbol, LineKind, FlowDirection and the turn (Reference) and mirror (Axis Z = −1); numbers at full precision; impulse lines carry their line object's data. The import uses them when present, else the class, else a stand-in by kind | A file we write must come back exactly; tested on every example (tags, line numbers, connections, positions, data, check results) |
| 0.1.12 | Check 8: a check valve counts only when the pump's flow reaches its inlet (port 0); reached at its outlet it is reported on the valve. `Op::TurnRound` mirrors an in-line item and swaps the lines on its two process ports | Lines drawn by hand can meet a check valve backwards; splitting always orients it |
| 0.1.11 | The canvas's drag overlay is a keyed Show: a non-keyed Show's child ran once, so the line under the mouse (and the selection box) never followed the mouse. A click on a port (no drag) leaves the wire "sticky" until the next click | The user saw lines appear only when finished |
| 0.1.11 | Quick insert is a popover at the point with a transparent backdrop, not a modal over the window | The drawing must stay visible while choosing where a symbol goes |
| 0.1.11 | Cause and effect is derived from the drawing only: interlocks are the interlock logic symbol (and switches wired straight to a final element); classification of each neighbour by category, not by line direction | Nothing to keep in step; robust to lines drawn either way |
| 0.1.11 | DEXPI: one sheet per file (as DXF); components placed in the first segment that ends at them, loose ones in a "PNS-UNCONNECTED" system; signal connections to equipment refer to the equipment, process ones to its nozzle. Tested for well-formed XML, unique IDs and resolving references (examples and random projects); not yet validated against the schema or other tools — said so in the help | Honest first step; references must never dangle |
| 0.1.10 | Import reads with calamine (MIT; NOTICE lists its crates): writing an xlsx reader of our own would be weeks for no gain, and hostile files are its job. Rows that live on the line, vessel or drawing (key empty) are never written; a disagreement is a "conflict" row naming the source. Blank cells never clear. Numbers take the cell's own unit, else the header's "(unit)", else the project's display unit; gauge marks (psig, barg) are read as the pressure the datasheet shows | The P&ID stays the one source for process data; imports must be safe to run twice |
| 0.1.10 | The datasheet layout is project data (`Settings.ds_layout`), filtering the datasheet view only: hidden rows keep data, are imported and are in issue snapshots; Δ counts only shown rows. The shell copies the default layout into every new project, examples too | A company form is a view; hiding must never lose data |
| 0.1.10 | Loop = instruments with the same first letter and number (ISA-5.1), at least two, relief devices excluded. Wiring is five text attributes in the class library group "Wiring" (not on the datasheet). Primary elements are drawn without wiring. The diagram is engine prims on an A3 sheet with the standard frame, so print and (later) DXF draw the same thing | Loop diagrams come from the same objects as the P&ID; nothing to keep in step |
| 0.1.9 | An issue (`model::Issue`, project-level `issues`, SQLite table `issues`) keeps the whole datasheet as text per "Section › Label" in SI display units; Δ compares the current SI datasheet to the last issue. Issues of a deleted object go with it (dropped silently in `normalize`). Undo withdraws an issue; there is no other way to delete one | A snapshot of what was issued is what a revision means; SI display text survives unit switches and class changes; issued records should not be edited |
| 0.1.9 | File format 3: 0.1.8 refuses 0.1.9 files ("update Datum Schema") rather than opening them and dropping the revisions on save | Losing issued revisions silently is worse than asking to update |
| 0.1.9 | Revisions: letters (A, B… Z, AA) for Preliminary and Issued for Enquiry, numbers (0, 1…) for Issued for Purchase and As-built; suggested, the user may type another (unique per datasheet, at most 12 characters). Bulk issue gives each datasheet its own next revision | Common practice in process plant documents; companies differ, so it is a suggestion |
| 0.1.8 | Datasheets are a view built by the engine from the object, its class (attribute `group` places a row in a section) and its process source; derived rows carry their source and are not editable on the datasheet | One copy of every value: the datasheet can never disagree with the P&ID |
| 0.1.8 | Process source order: a process line on the instrument; a process line through an in-line element it is tapped to; equipment it is tapped to by an impulse line; else its loop (same first letter and number), measurement first. Signal lines are never followed | LT-101 on a vessel must show the vessel, not the vent; LIC-101 its transmitter, not PVSV-101 |
| 0.1.7 | `Delete { ids, keep }`: deleting views only purges the objects of those views (and only when no other view shows them and `keep` is off); `DeleteObjects` removes objects with all their views. The window asks only when a deletion would leave an object drawn nowhere | Objects kept on purpose (not drawn) must survive later edits; a question on every delete would be noise |
| 0.1.6 | Attribute keys stay the short internal keys (design_p, size…) with the DEXPI name alongside (`dexpi`); class names are DEXPI-style and will be checked against the DEXPI specification when export is built | No migration of existing data; DEXPI export maps names in one place |
| 0.1.6 | The class library replaces `fields.rs`; the window gets each used class's attributes (with the project's additions) in `derived().attrs` | One owner of "what data a thing has"; project attributes show at once |
| 0.1.5 | The release workflow's Windows tests found that saving failed on Windows (flushing the new database through a read-only handle); fixed before 0.1.5 was published | Windows refuses FlushFileBuffers on a read-only handle; Linux allows it |
| 0.1.5 | Objects own tags and data; items and line pieces point at them (`obj`). A process line joined to an in-line symbol or a paired off-page connector whose other side has a line continues that line's object; otherwise it gets a new one with the sheet's next sequence number; `NewLineObject` splits a piece off | The user's "objects are the master"; a line number is a real object, so the run inference (and its bugs) is gone |
| 0.1.5 | Objects no view shows are deleted with their last view (until 0.1.7 offers "remove from sheet only") | No orphans until the Plant panel can show them |
| 0.1.5 | The history is kept in the document (not in the undo snapshots) and saved in the file; undo and redo are recorded as changes too | An audit trail must not disappear with an undo |
| 0.1.5 | SQLite via rusqlite (bundled); objects and attribute values are real tables (queryable); geometry rows keep JSON columns; recovery copies stay JSON | The user's single-database file; JSON for the parts nobody queries keeps the writer small |
| 0.1.5 | Ids stay u32 inside the file; every object also has a uid (UUID form) for exchange and later merging | Small ids keep the engine and window simple; the uid carries identity across projects |
| 0.1.3 (agreed) | Versions stay consecutive (0.1.4 released, then 0.1.5…); the object core order and defaults are in `docs/PLAN-0.2.md` §7; no reverse engineering of commercial software, only public documentation and open standards | The user's answers after the proposal |
| 0.1.3 (course change) | **Datum Schema becomes an object-based plant engineering tool, not a drawing tool**: objects are the master and symbols are views of them; a plant hierarchy (project › plant › unit › area); classes and attributes as data (a JSON class library, user attributes in Options); the project as one SQLite database with a history of every change; DEXPI names. The proposal and open questions are in `docs/PLAN-0.2.md`; 0.1.4 is held until they are discussed | The user's direction after 0.1.3: an open-source tool in the class of the enterprise object-based plant engineering suites, for students, researchers and engineers |
| plan | Everything in the brief §1 (name, magenta accent, identifier, `.dschema`, stack, units, ribbon tabs, layout, own symbols after ISO 10628-2 and ISA-5.1, logo, public words) | Made with the user in the hub, session 22 |
| plan | The shell (main.rs, styles, ribbon, File page, print, help) is copied from Datum Pipe, the newest copy of Plan's shell; the updater, `guard`, crash.log and atomic writes from Plan 0.4.3 itself | Pipe's shell is already adapted to an engineering app; Plan carries the latest safety rules (FAMILY §11) and Pipe's release profile still aborts on panic |
| plan | Symbols are data in the engine (a small drawing language), turned into SVG for the window and into blocks for DXF | One drawing per symbol; the window, print and DXF always agree |
| plan | Positions are sheet millimetres (y down); text primitives are placed by the middle of their height | Prints at true size; one rule for SVG (`dominant-baseline: central`) and DXF (middle alignment) |
| plan | Undo keeps the whole project before each edit (bounded to 200) rather than inverse operations | The brief asks for undo as operations; edits are operations, and a snapshot per operation is the simplest exact undo for a project of this size (Plan does the same) |
| plan | A run (one pipeline) is the process lines joined through in-line items and off-page pairs whose line numbers agree; a size change at a reducer starts a new run | The line list lists pipelines, not drawn pieces; the number carries the size, so it changes at a reducer |
| plan | Data typed on a line goes to every drawn piece of its run; hydraulics are per run, from its first piece | The pieces of one pipeline always agree, so the line list, badges and checks agree |
| plan | Lines are drawn and routed by the engine (Sim's router, ports facing four ways); bend points the user drags are kept | The brief: Sim's wires with bends you can drag |
| plan | Instrument presets (PT, LIC, FIC…) in the palette; a new instrument given only its letters takes the next free loop number on its sheet | The brief lists the tag letters; loops share numbers (LT-101, LIC-101, LV-101) |
| plan | Tags number 101 … on sheet 1, 201 … on sheet 2 (line sequence 1001 …, 2001 …) | "Automatic numbering per prefix and sheet" in the brief |
| 0.1.0 | Manual valves take the prefix HV (hand valve), not V | V is the vessels' prefix; the first test placed a valve tagged V-103 among vessels V-101 and V-102 |
| 0.1.1 | The build ran ahead of the brief's step plan (checks, lists, hydraulics, DXF and examples were ready with the canvas), so 0.1.1 carries brief steps 3–6 and the next releases are speed, polish and the exhibition build; each CHANGELOG section says what its release really contains | Releases stay small and true; holding finished work back to match a plan would only delay it |
| 0.1.3 | An independent review of the engine before the exhibition build, as Datum Pipe did: run joining by number at items with 3+ lines, one new sequence per pasted pipeline, checks 8/10 walk through off-page pairs, merge skipped in loops and keeps the other half's data, running out of ids is an error, CSV quotes CR, no apostrophe in Excel text, DXF letters upright outside blocks and no 5-digit escapes | Every confirmed finding fixed with a regression test; bend points on split left as a known limit |
| 0.1.3 | The DXF was read back with an independent reader (ezdxf 1.4.5): all four examples open with 0 errors and 0 repairs, blocks and TAG attributes intact, and render the right way up | "Opens in any CAD program" should be checked by a reader we did not write |
| 0.1.2 | The router marks cells and edges inside symbols by index ranges (binary search on the grid lines) and keeps distances in arrays, and its second search window is 120 mm | Routing was 5 s on a crowded 500-symbol sheet; now 0.07 s with the same routes |
| 0.1.1 | One line number label and one hydraulic badge per run per sheet, on its longest straight segment | Labels on every short piece between valves cluttered the examples |
| 0.1.1 | A process-type line with an end on an instrument bubble or a thermowell is an instrument connection: no number, not in the line list, not checked as a pipeline | Impulse lines are part of the instrument, not the piping |
| 0.1.1 | Final elements for check 11 are control valves and regulators only (by symbol), not relief devices | PSV and breather tags end in V too, and hid a missing control valve in Find the faults |
| 0.1.0 | The UI tests drive the real engine: `examples/serve.rs` answers the shell's commands over a local HTTP port and serves the built window; `api.rs` holds what both return | The brief's UI tests (place, connect, split, see a check appear and clear, export a list) need the real engine, not a canned fixture |

## 3. Roadmap

✅ done · 🔜 next · ⬜ later

| Version | Item | Status |
| --- | --- | --- |
| plan | Read the hub, Plan, Sim, Calc and Pipe; PLAN-0.1, MEMORY, CLAUDE, icon | ✅ |
| 0.1.0 | Shell, updater, File page, Options; model, symbols (about 80), units, pipe sizes, edits with undo, routing, the frame; canvas, palette, inspector; CI and release | ✅ |
| 0.1.1 | Graph, tags, numbering, hardening tests; off-page connectors, legend sheet, notes and Hold clouds; the 13 checks and the Checks panel; hydraulics with badges; the four lists with CSV and Excel; print, SVG, DXF; the examples and Find the faults | ✅ |
| 0.1.2 | Speed (router marks blocked cells per symbol: 5 s → 0.07 s on a crowded 500-symbol sheet), coloured badges, selected line number highlighted on every sheet, screenshots | ✅ |
| 0.1.3 | An independent engine review (a separate agent, report only): 8 findings fixed with regression tests in `hardening.rs` › `review`; more UI checks; list pages with a title block; DXF checked with ezdxf | ✅ |
| 0.1.4 | Final check: every example opens with 0 errors, prints and exports (engine test and UI checks); README with the demo, site.json (the exhibition build). Released after the discussion | ✅ |
| 0.1.5 | Objects and views, line-number objects, hierarchy node, SQLite file with history (PLAN-0.2 §7) | ✅ |
| 0.1.6 | Class library JSON (DEXPI names), Options › Classes with project attributes, required attributes in check 6 | ✅ |
| 0.1.7 | Plant panel (hierarchy, every object, drag to draw or move), delete the object or only the drawing, objects on several sheets, Area column in the lists | ✅ |
| 0.1.8 | Instrument datasheets (process conditions from the line, vessel or loop; Specification group; print on A4; Excel one worksheet each) | ✅ |
| 0.1.9 | Datasheet revisions and status (issue with rev, status, date, author, note; Δ since the last issue; revision block on screen, print and Excel; datasheet index list with bulk issue); file format 3 | ✅ |
| 0.1.10 | Datasheet import from Excel (by tag, preview, never overwriting P&ID values, one undo step); company datasheet layout (form number, logo, footer, hidden rows, section order, default for new projects); loop diagrams (A3, wiring per device, print one or all). The three planned steps released as one version, as the user asked | ✅ |
| 0.1.11 | Fixes from the user: the line being drawn is shown under the mouse (a click on a port starts a line that follows the mouse until the next click); the symbol search is a small popover beside the point. Cause-and-effect matrix; DEXPI export; loop diagrams in DXF | ✅ |
| 0.1.12 | Tell Me overlay fixed (class clash); DEXPI import; check 8 checks a check valve's direction, Turn round; bend points kept on split and join; the logo in the Excel datasheets | ✅ |
| 0.2.0 | DXF reader; client symbol libraries from a legend (review, connection points, share); drawing standards; an old drawing behind a sheet; send a line to Datum Pipe and a loop to Datum Sim; PFD sheets with streams, the stream table, the Streams list and checks 14–16; sheet revisions with comparison and clouds; the Crude preheat example; file format 4 | ✅ |
| 0.3.0 | Stream compositions and check 17 (component balance, fractions add up); I/O summary (list, print, Excel); checks 18 (range covers the operating value), 19 (fail position), 20 (loop wiring) | ✅ |
| 0.3.1 | Valve and fitting take-off (list, print, Excel, CSV; Tell Me entries for the newer lists) | ✅ |
| 0.3.2 | Company title blocks (cells, fields, own fields, logo; editor with the engine's preview; save, load, use for new projects) | ✅ |
| 0.3.3 | Review comments (pins, replies, resolve/reopen, edit, delete; import a reviewer's copy; list, print, Excel) | ✅ |
| 0.3.4 | User guide (Help › User Guide: chapters from the help topics, rules, shortcuts, glossary; search; print on A4) | ✅ |
| 0.4.0 | Pipe specs: classes with allowed components by size and rating; checks 21–23; take-off descriptions and codes; CSV import; editor | ✅ |
| 0.4.1 | Values passed along a line (size, pipe class) to in-line items; the To do tab | ✅ |
| 0.4.2 | Implied items (flanges, gaskets, bolts) in the take-off; end types on spec components | ✅ |
| 0.4.3 | Assemblies (save, place, share, defaults); data labels by class | ✅ |
| 0.4.4 | Company rules (check 24), tag formats, speed on 120 sheets (open sheet 1.9 s → 39 ms) | ✅ |
| — | Brainstorm again after 0.4.4 (or after 0.5.0, as the user asked to continue to it): compare with the commercial suites, re-plan | 🔜 |
| 0.5.0 | Instrument wiring database (JunctionBox, Cable, IOCard objects; cable schedule; I/O allocation; check 25; loop diagrams) | ✅ |
| 0.5.1 | Calculation sheets in Datum Calc (8 packs copied into `calcpacks/`), filled from the object, its lines, size, spec and limits; refused when essential inputs are missing | ✅ |
| 0.5.2 | Sizing: control valves (ISA-75.01), orifices (ISO 5167), relief valves (API 520) | ✅ |
| 0.5.3 | Equipment datasheets; simulator stream tables imported (Excel, CSV) | ⬜ |
| 0.5.4 | Report templates (columns, filters, grouping saved and exported) | ⬜ |
| 0.6.0 | Document status (IFR, IFA, IFC) and transmittals | ⬜ |
| 0.6.1 | Shared projects (several people, claims, merge) | ⬜ |
| 0.6.2 | Scripting, report templates, DWG | ⬜ |
| 1.0 | DEXPI tested with other programs' files; hydraulics validation note; tested at 100+ sheets; signed installer | ⬜ |
| 0.5 | Company templates; project reports (valve and fitting take-off); review comments | ⬜ |
| 1.0 | User guide, a validation note for the hydraulics, signed installer | ⬜ |

## 4. Known limits

- DXF only (DWG must be saved as DXF first); the reader skips dimensions, images, 3D and patterned hatches (counted in the warnings). Text rotation in old drawings is not kept.
- Legend suggestions are by words (description and block name); unusual legends need the review. Connection points are suggested for two-port symbols; equipment needs its points placed.
- To Datum Pipe sends a straight run (a P&ID has no 3D routing) and estimated valve masses; To Datum Sim uses typical dynamics.
- Compositions are mass fractions only (no molar basis); balances need every stream's mass (and enthalpy) flow; tees between streams are not balanced (draw a mixer or splitter symbol).
- Revision comparison is per sheet, by object ids; a symbol deleted and placed again counts as removed and added.

- DEXPI import reads one drawing per file and does not pair off-page connectors across files; line pieces are re-routed (centre lines are not kept as bend points).
- Lines keep their bend points when symbols move; drag a segment again or Straighten to re-route.
- Hydraulics are single-phase Darcy–Weisbach per line number (from its first piece); two-phase lines only get the two-phase velocity limits, not a two-phase pressure drop.
- DXF text beyond ASCII is written as \U+XXXX escapes (CAD programs decode them; some simple viewers show them literally); the hydraulic badges are written OK, ! and X.
- The examples' layouts are hand-placed; lines route themselves, so a moved symbol can make a route less tidy until a segment is dragged.
- Printing uses one page size per print job (the largest page); mixed sheet sizes print best one size at a time or fitted to A3.

## 5. Session log

### 2026-10-29: Session 1 (0.1.0)
- Read the brief, the hub (CLAUDE, MEMORY, FAMILY, BRAND, BRAIN) and the reference apps (Plan's shell and updater, Sim's canvas and router, Calc's units and pipe pack, Pipe's sizes and shell).
- Wrote CLAUDE.md, this MEMORY, PLAN-0.1; the logo (vessel, line with flow arrow and instrument bubble, pump, amber diamond) and the Windows icons.
- Engine: model and file with repair, symbols (82 definitions), fields, units, pipe sizes, routing, frame, graph, tags, edit operations with split and merge, view, hydraulics (tested against a hand calculation and Datum Calc's pipe pack); 23 tests.
- Window: ribbon, File page, Options (with the check rules and hydraulic limits), canvas, palette, inspector, bottom panel, print and SVG writer, help cards; the test server and 16 UI checks against the real engine.

### 2026-10-29: Session 1, summary (0.1.0 → 0.1.4)
- **Released:** 0.1.0 (shell, sheet, symbols, lines, updater), 0.1.1 (checks, hydraulics, lists, examples, print, DXF), 0.1.2 (fast routing, coloured badges, line number highlight, screenshots), 0.1.3 (fixes from an independent engine review), 0.1.4 (the exhibition build: final check, README demo). Each as `DatumSchema-x.y.z-setup.exe` + `SHA256SUMS.txt` from `release.yml`.
- **Works:** about 80 own symbols; lines port to port with routing, hops, flow arrows and signal marks; in-line symbols split lines and join them again when deleted; off-page connectors with jump; sheets A3/A1/ANSI B/ANSI D with zones, title block and revisions; legend sheet; notes, leaders, Hold clouds; tags and loops numbered per sheet, renumber, copy with the next free tag; the 13 checks with the Checks panel; line hydraulics with badges and editable limits; the four live lists with sort, filter, group, edit, CSV and Excel; print (sheets, lists with title block, legend), SVG, DXF (read by ezdxf with 0 errors); SI/US units; undo/redo; recovery; the updater. 55 engine tests (hardening, every check, hydraulics against a hand calculation and Datum Calc's pack, DXF read back, examples) and 47 UI checks against the real engine.
- **Cut or not done:** nothing from the brief's cut list was cut (the Hold cloud, line hops, ANSI sizes and the rarer symbols are all in). Not done: bend points are dropped when a line is split; check 8 does not look at a check valve's direction; the real shell was only started on Linux (Xvfb), the installer is built and published by the release workflow on Windows.
- **The 2-minute exhibition demo:** open File › Examples › Find the faults; show the Checks panel (3 errors, 5 warnings) and click an issue to jump to it; fix two (drop a check valve on pump P-101A's discharge; rename the duplicate pump to P-101B); open the Line list and click a discharge line to show its velocity badge and hydraulics in the Inspector; File › Print to show a sheet with its title block; Sheets › DXF.
- **For the hub chat:** add Datum Schema to apps.json (id schema, accent #9C1F84) and copy its icon (`docs/brand/datum-schema-icon.svg`) to `brand/datum-schema-icon.svg`.

### 2026-10-29: Session 1, course change
- The user changed direction after 0.1.3: an object-based plant engineering application, not a drawing tool (objects master, views on sheets, plant hierarchy, class library as data, SQLite with history, DEXPI names). 0.1.4 is committed but not released.
- Read the public documentation of the enterprise suites (object database across the lifecycle, base objects and classes, rules with real-time consistency and propagation, task lists, claims and working layers, revisions, 3D comparison) and of DEXPI (Proteus XML, specification CC BY 4.0, pyDEXPI). Wrote `docs/PLAN-0.2.md` with the proposal, the release order and seven questions for the user.

### 2026-10-29: Session 1, 0.1.5 (the object core begins)
- 0.1.4 released as agreed. Built 0.1.5: objects and views, line-number objects, the plant hierarchy model, the history, the SQLite file with 0.1 conversion. Engine 58 tests (new: SQLite round trip with history, 0.1 conversion, damaged databases); 48 UI checks including the SQLite file read back.
- Did not use the reverse-engineering toolkit the user pointed to: it is for inspecting other software, and the rule is to build from public documentation and open standards only.

### 2026-10-29: Session 1, 0.1.6 → 0.1.8, and where we are
- **Released:** 0.1.5 (objects, SQLite file with history; re-released after the Windows save fix), 0.1.6 (class library, Options › Classes, required attributes), 0.1.7 (Plant panel, delete the object or only the drawing, objects on several sheets), 0.1.8 (instrument datasheets).
- **0.1.8:** `datasheets.rs` builds each datasheet; Specification attributes for Instrument, PrimaryElement, ControlValve and ReliefDevice in `classes.json` (`group: "Specification"`); `datasheet` and `datasheets_all` commands and the `datasheets` export kind; datasheet dialog (editable own values, grey derived rows), Print (one or all, A4 portrait with title block), Excel export; Inspector and Plant panel entries; help card. Tests: engine (line, vessel and loop sources; company section; Excel), hardening on random projects, UI (sections, vessel source, typing, printing one page).
- **Found while building:** the first loop fallback matched PVSV-101 for LIC-101 (same number, other loop) and followed a controller's signal line to its valve; both fixed and covered by the engine test.
- **Next (to discuss):** datasheet revisions and status, a company datasheet layout, loop diagrams; DEXPI export; datasheet import from Excel.

### 2026-10-30: Session 1, 0.1.9 (datasheet revisions)
- Built and released 0.1.9: `Op::IssueDatasheet` (one or many, rev suggested per status, `by` from the document's user), `model::Issue` with the snapshot, the `issues` table (format 3), Δ marks and `was` per row, revision block, `next_rev`, the Datasheet index list (fifth list kind) with bulk issue, Excel and print with revisions, history entries `issue` / `undo withdraw`.
- Tests: engine (revision sequence incl. Z → AA; issue, Δ for an own value and a line value, US units do not mark changes, duplicate and bad statuses refused, index row, undo, save and reopen, delete takes issues; format 2 files open cleanly; newer formats refused), hardening (random hostile issues), UI (not issued, suggested A, issue, Δ, index row, bulk issue of the shown rows).

### 2026-10-30: Session 1, 0.1.10 (import, company layout, loop diagrams)
- The user asked for the three planned steps (Excel import, company layout, loop diagrams) as one release, 0.1.10.
- Engine: `import.rs` (calamine reader; table and one-sheet layouts; unit conversion; conflicts for line, vessel and drawing values), `datasheets::full`/`all_full`/`layout_rows` and the layout in `build_in`, `model::DsLayout` with logo checks, `loops.rs`, the Wiring attributes on Instrument and ControlValve. Shell and test-server commands `import_datasheets`, `loops`, `loop_diagram`, `layout_rows`; the default layout goes into every new project.
- Window: import preview dialog (tick, import as one Batch), Options › Datasheets (form, logo, footer, rows, order, use for new projects), logo, form and footer on datasheets and their print, loop diagrams dialog (list, preview, wiring table, print), Print with loop diagrams, ribbon Lists › Loop Diagrams and Import Excel, Inspector Loop diagram…, help cards.
- Tests: engine 74 (import table and read-back of our own export, units as typed, hostile bytes; layout; loops on every example; hardening runs import, layout and loops on random projects); UI 77 (import preview and apply, layout, loop diagram, wiring, print).
- Looked at the rendered loop diagrams: moved device text under the symbols (it crossed the wiring), drew primary elements without wiring.

### 2026-10-30: Session 1, 0.1.11 (the user's two drawing issues, and the proposed features)
- The user reported: the symbol search popup covers everything; a line being drawn does not show until it is finished. Root cause of the second: the drag overlay in `canvas.tsx` was a non-keyed `<Show>` whose child read the drag once, so the preview stayed a zero-length stub at the start port (the selection box was affected too). Fixed with a keyed Show; added click-to-start lines that follow the mouse (Escape or right-click cancels) and a ringed snap target. The search is now a popover at the point.
- Added the cause-and-effect matrix (`cause.rs`, an interlock UY-202 in the compressor example), the DEXPI export (`dexpi.rs`) and loop diagrams in DXF (`dxf::write_prims`, sharing `head` with the sheet writer).
- Tests: engine 79 (matrix from the example and a direct switch; DEXPI well-formed with unique IDs and resolving references on every example, classes and attributes on the pump; loop DXF read back; hardening runs all three on random projects and found a byte-slicing panic on an emoji tag in the matrix code and a dangling DEXPI reference for branches onto signal lines, both fixed); UI checks for drag and click line drawing, the popover, the matrix, its print, DEXPI and loop DXF.
- The disk filled with build output during the session; cleared target/debug/incremental.

### 2026-10-30: Session 1, 0.1.12
- The user showed Ctrl+K hiding the whole window (and compared with Datum Plan). Reproduced in the test browser: the overlay's `top` class picked up the title bar's opaque background. Fixed, with a UI check that the overlay is translucent and the ribbon visible.
- Built the planned items: DEXPI import (with round-trip additions to the export), check 8 direction with Turn round (menu), bend points kept on split and join, the logo in Excel datasheets (base64 decoding moved into the engine).
- The round-trip test found two real export bugs, both fixed: attribute values rounded to 4 decimals (a gas viscosity became 0) and impulse lines losing their line-number data.
- Tests: engine 84 (DEXPI round trip on every example, a foreign file in metres with an unknown class, damaged XML; check 8 both ways; bend points; Excel logo; hardening imports every random project's export); UI checks for Tell Me, DEXPI import and Turn round.

### 2026-10-30: Session 1, 0.2.0 (one bigger release up to process flow diagrams)
- The user asked about importing a client's DWG legend as a separate library; after the brainstorm they chose DXF and asked for "a bigger build and release at least until process flow diagrams". Built in five stages, each tested and committed: DXF reader and client libraries with drawing standards; an old drawing behind a sheet; Datum Pipe and Datum Sim links; PFDs with streams, stream table and balances; revisions with comparison and clouds.
- Found on the way: our own DXF export wrote legend rows as loose geometry (no blocks), so a legend round trip found nothing; legend rows are now INSERTs with their name. Validating embedded copies re-keyed them (added `check_copies`). With a standard on, checks stopped recognising PSVs and control valves (`base_key` everywhere).
- The two other apps' formats were studied by two reader agents in parallel; every generated file was then opened in the real engines.
- Tests: engine 101 (DXF reader and legend scan, re-skin and exact restore, background through undo, save and the window's copy, Pipe and Sim files, PFD numbering, balances, table and rule 16, revisions through save and reopen, random DXF hardening); UI checks for libraries and the standard, the background, both sends, the PFD example, a broken mass balance, a new PFD sheet, DEXPI's refusal, issue, compare and clouds.

### 2026-10-30: Session 1, 0.3.0 (compositions, I/O summary, instrument checks)
- After 0.2.0 the user asked for a comparison with the commercial suites (given in chat only, never in public text), then said to stick to the roadmap. 0.3 was scoped as stream compositions with a component balance, the I/O summary, and checks on instrument ranges, fail positions and loop wiring.
- The hardening tests caught a byte-slicing panic on a hostile tag in the I/O classification, and the examples test caught Excel refusing "/" in the worksheet name "I/O summary"; both fixed.
- Tests: engine 103 (ranges in several units and refusals, checks 17–20 both ways, the I/O summary, component rows and columns); UI checks for the components in Options, a broken component balance, the I/O summary and check 18 from the instrument index.

### 2026-10-30: Session 1, 0.3.1 (take-off)
- The user asked to step the next roadmap items as 0.3.1, 0.3.2 and so on. 0.3.1 is the valve and fitting take-off (`lists.rs` kind `takeoff`), with Tell Me entries for the take-off, I/O summary and stream list.
- Tests: engine 104 (take-off groups, sizes across two lines, one count per object, nothing from a PFD); UI checks for the take-off and its CSV.

### 2026-10-30: Session 1, 0.3.2 (company title blocks)
- The title block is now drawn from cells; the standard block is the default list and draws exactly as before. Company blocks have their own size (60–400 × 15–150 mm, sound after `normalize`), fields of their own filled in the Inspector, fixed text and the logo; files to share (`*.dstitle.json`), and a default for new projects (applied in `fresh`).
- Tests: engine 105 (a company block with logo, own field, fixed text and an absurd cell clamped inside; reserved space; preview; JSON round trip; fallback to standard); UI checks for turning it on, adding a cell and filling the field from the Inspector.

### 2026-10-31: Session 1, 0.3.3 (review comments)
- The user asked for 0.3.3 and then 0.3.4. Review comments: `review.rs` (model, ops, merge, normalize, pins), a Comments panel and pins in the window, Check › Comment and Import Review, the Review comments list for print and Excel.
- The comment dialog opens after the click is over (on mouse-up), so the click cannot land on the dialog. A first UI test failed only because it clicked the tool hint bar at the top of the canvas; it now clicks the sheet.
- Tests: engine 107 (dates; comment, reply, resolve, merge both ways with no doubles, undo, pins, list, a deleted target, save and reopen, hostile input); UI checks for pinning, replying, importing a reviewer's copy and resolving.

### 2026-10-31: Session 1, 0.3.4 (user guide)
- The user guide (`guide.tsx`) as a File page reached from Help › User Guide and Tell Me; shortcuts, glossary and Getting started moved to exported constants in `dialogs.tsx` so the cards and the guide share them. A first look showed the File view's `aside` styles leaking into the guide's contents; it is a `nav` now.
- Tests: UI checks that the guide has every chapter and at least 40 topics, all 20 rules, a search that narrows it, and a print with every topic.

### 2026-11-01: Session 1, re-planning after 0.3.4
- The user asked for a detailed comparison with the commercial P&ID suites; agreed Datum Schema is complete for one engineer but not yet comparable, so not 1.0. New roadmap 0.4.x (piping rules), 0.5.x (instruments and calculations, with Datum Calc for calculation sheets), 0.6.x (teams), then 1.0 (proof). Brainstorm again after 0.4.4. Waiting for the link to an open-source CAD repository the user wants us to look at.

### 2026-11-01: Session 1, 0.4.0 (pipe specs)
- `specs.rs` with checks 21–23, the starter class, CSV import; take-off columns Description and Commodity code; Options editor; Pipe class suggestions in the Inspector; the Pump transfer example carries A1A. A first starter had its temperature limit at the rating point (38 °C), raising false alarms on the example; now 425 °C. A vent on a 3" line was judged at 3"; terminals now use their own size.
- Tests: engine 110 (ranges, entries, checks 21–23 pass and fail, case-insensitive classes, normalize, CSV with quotes and unknown components, take-off codes); UI checks for the spec in Options, CSV import, check 22 and the take-off codes.

### 2026-11-01: Session 1, 0.4.1 (values passed along, To do)
- The user asked to keep going to 0.5.0, one release per step. 0.4.1: `propagate.rs` and the To do tab. The first version wrote the pipe class into a valve's "class", which is its rating (300#), and sized control and relief valves from their lines: the To do list filled with false overrides on the examples. Fixed with "pipe_class" and the exclusions.
- Tests: engine 111 (propagation, overrides kept and listed, taking the line's value, vents unsized, history without hidden keys, sheet and datasheet changes since issue; the DEXPI round trip ignores hidden keys); UI checks for the valve list's line values, an override on To do and taking the line's size.

### 2026-11-01: Session 1, 0.4.2 (implied items)
- `specs::implied`, `ends_of`, `Implied` rules on each class (the starter: weld-neck flange, gasket, bolt set per flanged end); take-off rows after the components; Ends and Implied tables in the spec editor; CSV Ends column. The test first expected 12 gaskets at 3": the level control valve is a seventh flanged 3" component, so 14 is right.
- Tests: engine 112 (ends read from descriptions, quantities by size, the take-off rows, nothing without specs); UI check for the implied gaskets in the take-off.

### 2026-11-01: Session 1, 0.4.3 (assemblies and data labels)
- `Op::SaveAssembly`/`PlaceAssembly` (with `doc::assembly` and `paste` taking source objects), labels in `view.rs`, `ui/src/assemblies.tsx` (save dialog, palette tab, Options sections). An assembly placed in another project first took the next tag although its own was free; now kept where free. Names match whatever their case.
- Tests: engine 114 (save, replace by name, place with the next tag and data kept, place in another project, labels under pumps only); UI checks for saving, placing and labels.

### 2026-11-01: Session 1, 0.4.4 (company rules, tag formats, speed)
- `rules.rs` (check 24), `tags::TagFormat` in `next_free`, `next_loop`, `bump` (width kept) and `renumber`; Options sections in `ui/src/rules.tsx`. The new `bench_big` example (120 sheets) found the open sheet at 1.9 s, nearly all in check 20 (every loop's members, each found with a linear `obj` search). Fixed as recorded in Decisions; the open sheet is 39 ms.
- Tests: engine 116 (rules pass and fail, levels and words, off rules, tag formats and their normalising); UI checks for a company rule in the Checks panel and the tag preview.

### 2026-11-01: Session 1, 0.5.0 (instrument wiring)
- `wiring.rs`, `Op::AddObject`, classes WiringItem › JunctionBox, Cable, IOCard, lists jbs, cables, ioalloc, check 25, loop diagram details, `ui/src/wiring.tsx` and the Wiring menu. The first test showed a valve on an AI card passing: `io_of` read the channel "AI-01/1" as an AI channel; it now ignores card channels.
- Releases 0.4.1 to 0.5.0 went out one by one, each pushed only after the previous one's build had started, so each tag builds its own commit.
- Tests: engine 118 (channels and terminals read, wiring that adds up, every kind of fault, devices in the control system left out); UI checks for adding a card, a device on its channel, and check 25 on a valve on an AI card.
- Next: the brainstorm the user asked for after these releases (compare with the commercial suites again), then 0.5.1 (calculation sheets in Datum Calc).

### 2026-11-01: Session 1, 0.5.1 (Datum Calc sheets)
- After a second comparison with the commercial suites (in chat), the user asked for 0.5.1 to 0.5.4. `calc.rs` and eight packs copied from Datum Calc (NOTICE). Every sheet the three examples can make (35) was opened and computed with Datum Calc's real engine from the local clone (scratch program, deleted): 0 errors, 0 failed checks; 38 others are refused for missing data.
- Tests: engine 119 (sheets by class, values and their sources, the pack's sample words replaced, unique ids, refusal without a flow, the line's velocity limit); UI checks for the Calculations buttons and the saved sheet.

### 2026-11-01: Session 1, 0.5.2 (sizing)
- `sizing.rs`: control valve Cv/Kv (liquid with choked flow, gas with Y), orifice beta by bisection on ISO 5167-2 C (corner taps), relief area per API 520 Part I with the API 526 letter. Sizing attributes added to ControlValve, ReliefDevice and PrimaryElement in classes.json. Inspector Sizing group (`ui/src/inspector.tsx`), help "sizing", command `sizing` (main.rs, serve.rs).
- Pump example: LV-101 has a 0.8 bar design drop (Kv 33.5 = 30 √(0.998/0.8)), PSV-101 is a steam relief (7 200 kg/h, 458 K, M 18, k 1.33: 1 168 mm², orifice K). The compressor's FE-201 sizes with the assumed 25 kPa (β 0.377).
- Tests: engine 121 (hand calculations for each formula, choked liquid and gas, C(1.4) = 0.02703, orifice round trip, devices on the drawing, hostile values never giving a result); UI check 49 (sized in the Inspector, Cv written).
