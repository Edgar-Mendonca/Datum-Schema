# Datum Schema: project memory

> **Paste this file at the start of each new session**, or just open the repo: `CLAUDE.md` tells Claude to read it.
> It records what exists, why, and what comes next. Update it at the end of every session.

| | |
| --- | --- |
| **Product** | Datum Schema: intelligent P&IDs and process flow diagrams: draw, check and list (symbols and lines that carry their data and connections; live line list, equipment list, instrument index and valve list; drawing checks; line hydraulics; print, SVG and DXF) |
| **Current version** | 0.1.0 (2026-10-29) |
| **Repo** | `Edgar-Mendonca/Datum-Schema` · branch `claude/vigilant-hamilton-ku7r0w` (the first branch pushed, so the default branch) |
| **Stack** | Rust engine (`crates/core`: model, symbols, graph, tags, checks, hydraulics, lists, frame, routing, DXF), Tauri 2, SolidJS + strict TypeScript; no Python |
| **Identifier** | `com.edgarmendonca.datumschema` · files `.dschema` (JSON, `format` 1) · installer `DatumSchema-x.y.z-setup.exe` + `SHA256SUMS.txt` · publisher "Datum" |
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
| `crates/core/src/model.rs` | The project: sheets (number, title, size A3/A1/ANSI B/ANSI D, revisions, legend flag), items (symbol, position, quarter turns, mirror, scale, tag, data, off-page partner), lines (type, ends on a port, on another line or free, bend points, flow flip, data), notes (note, leader, Hold cloud), settings (line number format, rules off, hydraulic limits, roughness, grid); `.dschema` read part by part with `normalize` (damaged files open as far as possible, with warnings), and write |
| `crates/core/src/symbols.rs` | The symbol library (about 80 of our own drawings in a small drawing language: polylines, outlines, fills, dashes, circles, arcs, ellipses, rectangles, text), ports with directions, categories, tag prefixes, in-line and terminal flags; placement (turn, mirror, scale); primitives and their SVG |
| `crates/core/src/fields.rs` | The data each kind of thing carries, with the quantity of each number (for units) and choices |
| `crates/core/src/units.rs` | Calc's unit table (NIST SP 811) with unit expressions and Schema's display table |
| `crates/core/src/pipes.rs` | Pipe's sizes ½"–24" and schedules (ASME B36.10M/B36.19M dimensions); inside diameters |
| `crates/core/src/doc.rs` | Edit operations (add, set, move, rotate, mirror, delete with merge of in-line items, set data, tags, bend points, split, paste with the next free tags, pair connectors, sheets, title, project, settings, renumber, batch) on a copy, kept only if `normalize` finds nothing to repair; undo/redo; size-limited reads and atomic writes |
| `crates/core/src/graph.rs` | Connectivity: port positions, line numbers from the format, the network (lines per port, branches), runs (one line number through in-line items and off-page pairs), walks along the piping (`reach`), from/to, pump suction/discharge |
| `crates/core/src/tags.rs` | Tags: split, next free per prefix and sheet, copies (P-101A → P-101B), renumber, duplicates, ISA-5.1 letters, controllers, measurements, final elements |
| `crates/core/src/route.rs` | Sim's orthogonal router, extended to ports facing four directions and open ends |
| `crates/core/src/view.rs` | What the window draws for a sheet: items with ports and upright texts (tags, instrument letters and loop numbers, fail positions, connector partners), lines (route, pieces with hops, arrows or signal marks, line number, hydraulic badge), notes and Hold clouds, the legend |
| `crates/core/src/frame.rs` | The sheet frame: border, zones, title block, revision table, as primitives |
| `crates/core/src/hydraulics.rs` | Inside diameter, velocity, Reynolds, Colebrook (or 64/Re), Δp per 100 m and over length + fittings, limits by service, status |
| `crates/core/src/checks.rs`, `lists.rs`, `dxf.rs`, `examples.rs` | The checks, the four lists, the DXF writer and the examples (filled in 0.1.2 and 0.1.3) |
| `crates/core/src/api.rs` | What the shell's commands return (`derived`: the sheet's view, issues, hydraulics, runs, numbers; `library`), shared with the test server |
| `crates/core/examples/serve.rs` | The test server: the window and the engine behind one local HTTP port, answering the shell's commands, for the UI tests |
| `src-tauri/src/main.rs` | Commands: state, apply, history, derived, list, views, examples, new project, library, unit table, open/save, save file, export (CSV, Excel, DXF), options, recovery, recent, startup file, open_url, version, updates; `guard`, crash.log, atomic saves |
| `src-tauri/src/update.rs` | The updater (Plan's, `REPO` = Datum-Schema) |
| `ui/src/main.tsx` | Window layout (palette, sheet with its tab bar, inspector, bottom panel), status bar, keys, start-up and recovery |
| `ui/src/store.ts` | Window state: the project, the open sheet, the engine's derived data, the library, the selection, the drawing tool, layers, panel, menus, dialogs |
| `ui/src/canvas.tsx` | The drawing canvas (from Sim's): pan, zoom, snap, marquee, move, draw lines from ports, drag segments and ends, drop symbols (in-line ones into lines), the symbol sprite, primitives |
| `ui/src/palette.tsx` | The symbol palette: categories, search, common instruments |
| `ui/src/inspector.tsx` | The Inspector: item, line (with hydraulics), note, or the sheet and title block |
| `ui/src/panels.tsx` | The bottom panel: Checks and the four lists (sort, filter, group, click to find, edit, export) |
| `ui/src/ribbon.tsx`, `ui/src/cmds.ts` | Title bar, quick access, Tell Me, the ribbon; every command |
| `ui/src/backstage.tsx`, `ui/src/print.tsx`, `ui/src/roadmap.tsx` | File page (examples, export, options with the check rules and limits, updates), print (sheets at size or A3, lists, legend; the SVG writer), roadmap |
| `ui/src/help.tsx`, `ui/src/dialogs.tsx` | The `?` cards and the rule list; dialogs (save changes, recovery, add a symbol by name, pair connectors, shortcuts, getting started, glossary) |
| `ui/src/styles.css`, `ui/src/ui.tsx`, `ui/index.html` | Plan's styles (magenta accent) and components; the icon sprite |
| `tests_ui/run.cjs` | UI checks of the built window against the real engine (through the test server) |
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
| 0.1.0 | The UI tests drive the real engine: `examples/serve.rs` answers the shell's commands over a local HTTP port and serves the built window; `api.rs` holds what both return | The brief's UI tests (place, connect, split, see a check appear and clear, export a list) need the real engine, not a canned fixture |

## 3. Roadmap

✅ done · 🔜 next · ⬜ later

| Version | Item | Status |
| --- | --- | --- |
| plan | Read the hub, Plan, Sim, Calc and Pipe; PLAN-0.1, MEMORY, CLAUDE, icon | ✅ |
| 0.1.0 | Shell, updater, File page, Options; model, symbols (about 80), units, pipe sizes, edits with undo, routing, the frame; canvas, palette, inspector; CI and release | ✅ |
| 0.1.1 | Engine: graph, tags, numbering, hardening tests; canvas: off-page connectors, legend sheet, notes and Hold clouds | 🔜 |
| 0.1.2 | The 13 checks and the Checks panel; hydraulics with badges; the four lists with CSV and Excel | ⬜ |
| 0.1.3 | Print to PDF, SVG, DXF; the three examples and "Find the faults"; ? cards for every tool | ⬜ |
| 0.1.4 | Polish, speed with 500 items, screenshots, README, site.json; final check (the exhibition build) | ⬜ |
| 0.2 | DEXPI import and export; send a line to Datum Pipe; send a loop to Datum Sim; DXF import as a background | ⬜ |
| 0.3 | Process flow diagrams with stream tables; heat and mass balance checks; revision clouds and comparison | ⬜ |
| 0.4 | Loop diagrams and instrument datasheets; cause-and-effect matrix; valve datasheets | ⬜ |
| 0.5 | Company templates; project reports (valve and fitting take-off); review comments | ⬜ |
| 1.0 | User guide, a validation note for the hydraulics, signed installer | ⬜ |

## 4. Known limits

- Lines keep their bend points when symbols move; drag a segment again or Straighten to re-route.
- Printing uses one page size per print job (the largest page); mixed sheet sizes print best one size at a time or fitted to A3.

## 5. Session log

### 2026-10-29: Session 1 (0.1.0)
- Read the brief, the hub (CLAUDE, MEMORY, FAMILY, BRAND, BRAIN) and the reference apps (Plan's shell and updater, Sim's canvas and router, Calc's units and pipe pack, Pipe's sizes and shell).
- Wrote CLAUDE.md, this MEMORY, PLAN-0.1; the logo (vessel, line with flow arrow and instrument bubble, pump, amber diamond) and the Windows icons.
- Engine: model and file with repair, symbols (82 definitions), fields, units, pipe sizes, routing, frame, graph, tags, edit operations with split and merge, view, hydraulics (tested against a hand calculation and Datum Calc's pipe pack); 23 tests.
- Window: ribbon, File page, Options (with the check rules and hydraulic limits), canvas, palette, inspector, bottom panel, print and SVG writer, help cards; the test server and 16 UI checks against the real engine.
