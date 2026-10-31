<p align="center"><img src="docs/brand/datum-schema-icon.svg" width="96" alt="Datum Schema"></p>

<h1 align="center">Datum Schema</h1>

<p align="center">Intelligent P&IDs and process flow diagrams: draw, check and list.</p>

<p align="center">
  <a href="https://github.com/Edgar-Mendonca/Datum-Schema/releases"><img src="https://img.shields.io/github/v/release/Edgar-Mendonca/Datum-Schema?include_prereleases&color=9C1F84" alt="Release"></a>
  <a href="https://github.com/Edgar-Mendonca/Datum-Schema/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Edgar-Mendonca/Datum-Schema/ci.yml?label=CI" alt="CI"></a>
  <a href="https://github.com/Edgar-Mendonca/Datum-Schema/releases"><img src="https://img.shields.io/github/downloads/Edgar-Mendonca/Datum-Schema/total?color=9C1F84" alt="Downloads"></a>
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT">
  <img src="https://img.shields.io/badge/platform-Windows%2010%2F11-0C1A2B" alt="Windows">
  <img src="https://img.shields.io/badge/Rust-engine-B7410E" alt="Rust">
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB" alt="Tauri 2">
  <img src="https://img.shields.io/badge/SolidJS-window-2C4F7C" alt="SolidJS">
</p>

<p align="center">
  <a href="https://github.com/Edgar-Mendonca/Datum-Schema/releases/latest"><img src="https://img.shields.io/badge/Download_for_Windows-9C1F84?style=for-the-badge" alt="Download for Windows"></a>
  <a href="CHANGELOG.md"><img src="https://img.shields.io/badge/What's_new-0C1A2B?style=for-the-badge" alt="What's new"></a>
  <a href="docs/MEMORY.md"><img src="https://img.shields.io/badge/Memory_&_roadmap-5B6878?style=for-the-badge" alt="Memory and roadmap"></a>
</p>

![Datum Schema: a gas compressor package with its anti-surge loop, knock-out drum level control and after-cooler](docs/images/compressor.png)

Draw the plant with real symbols, and every symbol and line carries its data and knows what it is connected to. So Datum Schema writes the line list, equipment list, instrument index and valve list for you, always in step with the drawing; checks the drawing as you work; and sizes every line, with its velocity and pressure drop against limits. It is for process, chemical, mechanical, piping and instrument engineers, operators and students. It is free and open source (MIT), Windows first, installs in seconds and updates itself. More on the family site: [edgar-mendonca.github.io/Datum/apps/schema](https://edgar-mendonca.github.io/Datum/apps/schema).

> **Hydraulic results are a sizing aid.** Velocities and pressure drops help you size lines; they are checked by the engineer before they are used. The same goes for the heat and mass balance checks (they check the numbers typed, they are not a simulation) and for the models sent to Datum Pipe and Datum Sim (starting points with stated assumptions).

## What it does

- **Real symbols**: about 80 of our own drawings following ISO 10628-2 (equipment, valves, lines) and ISA-5.1 (instrument bubbles and letters), with their connection points.
- **Lines that know their ends**: drawn port to port, routed at right angles around symbols, with flow arrows and hops; a valve dropped on a line goes into it; off-page connectors carry a line to another sheet.
- **Data on everything**: tags, services, design and operating conditions, sizes, line numbers built from size, fluid, sequence and class, in SI or US units.
- **Sheets**: A3, A1, ANSI B and ANSI D with border, grid zones, title block and revision table; a legend sheet; your company's own title block (cells, fields, logo), shared as a file.
- **Live checks**: twenty rules on every change (duplicate tags, open ends, size changes without a reducer, class changes without a spec break, pumps without check valves, vessels without relief, control loops, hydraulic limits, heat, mass and component balances, instrument ranges, fail positions, loop wiring and more), each a click away from the problem.
- **Line hydraulics**: inside diameter from size and schedule, velocity, Reynolds number, Colebrook friction factor, pressure drop per 100 m and over the line, with a ✓ ! ✗ badge against the limits for its service.
- **Plant objects**: every equipment item, valve, instrument and line number is an object in a plant › unit › area hierarchy (the Plant panel); symbols are views of it, so one object can be drawn on several sheets. Every change is recorded with who made it and when.
- **Classes you can extend**: a class library (named after the open DEXPI data standard) defines what data each kind of object has; add your company's attributes in Options and mark them required.
- **Instrument datasheets**: one per instrument, control valve and relief device, with the process conditions taken from its line or vessel; type on it, issue it with a revision and status, see every value changed since the last issue (Δ), print it, export every datasheet to Excel; a datasheet index tracks them all; import datasheet values from Excel (matched by tag, with a preview first, never overwriting values that come from the P&ID); set your company's datasheet form (form number, logo, rows and section order).
- **Cause and effect**: the matrix of the interlocks, read from the P&ID (causes, effects with their actions, the interlock in each cell); print or Excel.
- **Loop diagrams**: every instrument loop drawn from the P&ID on an A3 sheet, with each device wired through its junction box and cable to its I/O channel and controller; type the wiring, print one or all, or save as DXF.
- **Process flow diagrams**: PFD sheets whose lines are streams (mass and volume flow, temperature, pressure, vapour fraction, density, enthalpy flow), numbered in diamonds, with their compositions, a stream table on the sheet, a stream list, and heat and mass balance checks round every item; P&ID lines can name the stream they carry and are checked against it.
- **A client's symbols**: read a client's legend sheet (DXF) into a symbol library, with suggested meanings and connection points to review; draw a whole project to that standard and back, with the objects, data and checks unchanged; share libraries as files.
- **Old drawings**: a DXF shown faintly behind a sheet, to redraw an old P&ID as a live one.
- **Review comments**: pinned to the drawing, with replies and resolve or reopen; reviewers work on their own copies and their comments are taken back in without doubles.
- **Revisions**: issue a sheet at a revision, compare it later with what changed, and cloud the changes with the revision triangle.
- **The Datum family**: send a line to Datum Pipe as a stress model, and a control loop to Datum Sim as a model to tune.
- **I/O summary**: inputs and outputs per control system (AI, AO, DI, DO, fieldbus) with 20% spare, from the instruments' wiring and signals.
- **Valve and fitting take-off**: every piping component counted by type, size, pipe class and rating, with its tags, for a material requisition.
- **Lists**: line list, stream list, I/O summary, take-off, equipment list, instrument index, valve list and datasheet index, always in step with the drawing; edit them, export to Excel or CSV, print them.
- **Print and export**: PDF (sheets at their size or fitted to A3, lists, legend), SVG, DXF (R12) that opens in any CAD program, and the DEXPI P&ID exchange format (XML), which it also opens.
- **User guide**: in the app (Help › User Guide), searchable and printable, with every feature step by step.
- **Examples**: a pump transfer, a compressor package, a steam heater loop, a crude preheat process flow diagram, and "Find the faults".
- **Safe files**: `.dschema` projects (a single SQLite database with the history), atomic saves, recovery of unsaved work, undo and redo for everything.

![The Checks panel on "Find the faults": duplicate tag, open end, a size change without a reducer, a pump without a check valve, a vessel without relief](docs/images/checks.png)

![The line list with hydraulics, and a line's velocity and pressure drop in the Inspector](docs/images/line-list.png)

## The two-minute demo

1. File › Examples › **Find the faults**: the Checks panel lists three errors and five warnings, and the status bar counts them.
2. Click "Pump P-101A has no check valve on its discharge": the pump is selected on the drawing. Drag a check valve from the palette onto its discharge line; the issue clears.
3. Click the duplicate tag issue and rename the standby pump P-101B in the Inspector; the issue clears.
4. Open the **Line list**: every line number with its from and to, flow, velocity and pressure drop. Click a line on the drawing to see its hydraulics and its ✓, ! or ✗ badge.
5. File › Print: the sheet with its title block, then the lists. Sheets › DXF writes the drawing for any CAD program.

## Build

```bash
npm ci
npm run build:ui
cargo build -p datum-schema --release
```

Checks before every push: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npm run check:ui`, `npm run test:ui`, `cargo deny check`.

## Licence

MIT © 2026 Edgar Mendonca. See [NOTICE](NOTICE) for the fonts and crates it uses.
