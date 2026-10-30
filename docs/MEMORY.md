# Datum Schema: project memory

> **Paste this file at the start of each new session**, or just open the repo: `CLAUDE.md` tells Claude to read it.
> It records what exists, why, and what comes next. Update it at the end of every session.

| | |
| --- | --- |
| **Product** | Datum Schema: intelligent P&IDs and process flow diagrams: draw, check and list (symbols and lines that carry their data and connections; live line list, equipment list, instrument index and valve list; drawing checks; line hydraulics; print, SVG and DXF) |
| **Current version** | 0.1.9 (2026-10-30) |
| **Repo** | `Edgar-Mendonca/Datum-Schema` · branch `claude/vigilant-hamilton-ku7r0w` (the first branch pushed, so the default branch) |
| **Stack** | Rust engine (`crates/core`: model, symbols, graph, tags, checks, hydraulics, lists, frame, routing, DXF), Tauri 2, SolidJS + strict TypeScript; no Python |
| **Identifier** | `com.edgarmendonca.datumschema` · files `.dschema` (SQLite, `format` 3 (2 before 0.1.9: no `issues` table); JSON `format` 1 from 0.1.0–0.1.4 converts) · installer `DatumSchema-x.y.z-setup.exe` + `SHA256SUMS.txt` · publisher "Datum" |
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
| `crates/core/src/units.rs` | Calc's unit table (NIST SP 811) with unit expressions and Schema's display table |
| `crates/core/src/pipes.rs` | Pipe's sizes ½"–24" and schedules (ASME B36.10M/B36.19M dimensions); inside diameters |
| `crates/core/src/doc.rs` | The history (`Change`: who, when, what, object, old, new; recorded by diffing objects after every edit, undo and redo). Edit operations (NewLineObject, SetNode, AddNode, SetNode2, DeleteNode, add, set, move, rotate, mirror, delete with merge of in-line items, set data, tags, bend points, split, paste with the next free tags, pair connectors, sheets, title, project, settings, renumber, batch) on a copy, kept only if `normalize` finds nothing to repair; undo/redo; size-limited reads and atomic writes |
| `crates/core/src/graph.rs` | Connectivity: port positions, line numbers from the format, the network (lines per port, branches), runs (one line number through in-line items and off-page pairs), walks along the piping (`reach`), from/to, pump suction/discharge |
| `crates/core/src/tags.rs` | Tags: split, next free per prefix and sheet, copies (P-101A → P-101B), renumber, duplicates, ISA-5.1 letters, controllers, measurements, final elements |
| `crates/core/src/route.rs` | Sim's orthogonal router, extended to ports facing four directions and open ends |
| `crates/core/src/view.rs` | What the window draws for a sheet: items with ports and upright texts (tags, instrument letters and loop numbers, fail positions, connector partners), lines (route, pieces with hops, arrows or signal marks, line number, hydraulic badge), notes and Hold clouds, the legend |
| `crates/core/src/frame.rs` | The sheet frame: border, zones, title block, revision table, as primitives |
| `crates/core/src/hydraulics.rs` | Inside diameter, velocity, Reynolds, Colebrook (or 64/Re), Δp per 100 m and over length + fittings, limits by service, status |
| `crates/core/src/checks.rs` (+ `checks/tests.rs`) | The 13 checks, one function per rule, each issue with level, reason, ids and place; a passing and a failing test per rule |
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
| 0.1.10 | Next, proposed: datasheet import from Excel (match by tag, preview, never overwrite line or vessel values) | ⬜ |
| 0.1.11 | Next, proposed: company datasheet layout (rows, order, logo, form number) | ⬜ |
| 0.1.12 | Next, proposed: loop diagrams from the loop | ⬜ |
| later | DEXPI import and export; send a line to Datum Pipe; send a loop to Datum Sim; DXF import as a background | ⬜ |
| 0.3 | Process flow diagrams with stream tables; heat and mass balance checks; revision clouds and comparison | ⬜ |
| 0.4 | Loop diagrams; cause-and-effect matrix; valve datasheets | ⬜ |
| 0.5 | Company templates; project reports (valve and fitting take-off); review comments | ⬜ |
| 1.0 | User guide, a validation note for the hydraulics, signed installer | ⬜ |

## 4. Known limits

- Splitting a line with a valve (and deleting the valve again) re-routes both halves: bend points the user placed on that line are dropped (review finding 9, not fixed).
- Check 8 accepts a check valve whatever way round it is fitted.
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

