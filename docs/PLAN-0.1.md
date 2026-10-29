# Datum Schema 0.1: plan

Written before building, from the brief (`Edgar-Mendonca/Datum` › `briefs/DATUM-SCHEMA.md`, hub session 22). Every product decision was made in the brief; this file says how the build is cut and where the risks are. Choices the brief left open are in `docs/MEMORY.md` › Decisions.

## Scope (brief §2)

Intelligent P&IDs and process flow diagrams: draw the plant with real symbols; every symbol and line carries its data and knows what it is connected to. From that, Datum Schema writes the line list, equipment list, instrument index and valve list, checks the drawing as you work (13 rules), and sizes every line (velocity and pressure drop against limits). Printing to PDF, SVG per sheet, DXF (R12, our own writer). Three examples plus "Find the faults" for the exhibition.

## How it is built

- **Engine first** (`crates/core`): the model (project, sheets, items, lines, notes), operations with undo, the symbol library as data (one drawing per symbol in a small primitive language, turned into SVG for the window and blocks for DXF), connectivity (`graph`), tags, checks, hydraulics, lists, the sheet frame (border, zones, title block, revision table) and DXF. The window draws what the engine returns: routed line paths with hops and flow arrows, symbol placements, badges.
- **The canvas** is Datum Sim's block canvas (pan, zoom, snap, marquee, drag) with Sim's orthogonal router extended to ports facing any of four directions.
- **Units** from Datum Calc's table (by way of Datum Pipe): SI and US display, SI in the file.
- **Pipe sizes** from Datum Pipe's library (½"–24", every schedule) for inside diameters.
- **Shell** from Datum Plan (guarded commands, crash.log, atomic saves, checksummed updater), Excel export with Plan's writer (rust_xlsxwriter).

## Day plan (brief §6)

| Step | Work | Release |
| --- | --- | --- |
| 1 | Read; skeleton from Plan (renamed, magenta, logo, publisher "Datum", updater, CI, release); a blank sheet with border and title block saves and opens | 0.1.0 |
| 2 | Engine: model, graph, tags, units, pipe library, undo; tests | — |
| 3 | Canvas from Sim: sheets, placing symbols, lines with ports, split by in-line items, off-page connectors; the inspector | 0.1.1 |
| 4 | All ~80 symbols and line types; data on everything; automatic numbering; legend sheet | — |
| 5 | Checks (13) and the Checks panel; hydraulics with badges; the four lists with CSV/Excel | 0.1.2 |
| 6 | Print to PDF, SVG, DXF; the three examples and "Find the faults"; ? cards | 0.1.3 |
| 7 | Polish, 500-item performance, screenshots, README, site.json; final check | 0.1.4 |

Cut order if time runs short (brief): the Hold cloud, line hops, ANSI sheet sizes, the less common symbols. Never cut connectivity, checks, lists, hydraulics, printing or DXF.

## Risks

- **Size of the job in one run.** Mitigation: the engine carries everything the window and DXF share (symbols, frame, routes), so each feature is written once; the examples are built in code and double as tests.
- **Routing quality** with ports on four sides and many lines: Sim's sparse-grid router is proven; user bend points override it.
- **Releases from a cloud session**: tag pushes are blocked, so each release runs `release.yml` by workflow_dispatch on Windows; the CI on Linux checks the same code first.
- **No Windows machine here**: the window is checked in Chromium with a mocked bridge (Playwright) and the real shell is built on Linux (WebKitGTK) for compile checks; the installer is built only by the release workflow.
