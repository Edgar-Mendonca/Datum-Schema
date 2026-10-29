# Datum Schema: the object core (agreed; releases 0.1.5 onwards)

Written after 0.1.3, when the user changed course (session 1). Nothing here is built yet; the questions at the end are for the user. Commercial products are not named in this file (FAMILY §13); "the enterprise suites" means the commercial object-based plant engineering systems the user has worked with.

## 1. Feedback

Datum Schema must not stay a drawing tool that knows its connections. The aim is an open-source, object-based plant engineering application in the class of the enterprise suites: easier, reliable, efficient, robust, for students, researchers and engineers, written from scratch in Rust with no proprietary content. The user's five points:

1. **Objects are the master.** Every equipment item, line, valve and instrument is an object with a unique id; a symbol on a sheet is only a view of it. An object can appear on several sheets, in lists and in datasheets. Deleting a symbol asks: delete the object, or only remove it from this sheet?
2. **Plant hierarchy.** Every object sits in a tree: project › plant › unit › area, with defaults so it can be ignored at first. A "Plant" panel beside the palette; lists group by it.
3. **Class library.** Object types are data, not code. Each class (centrifugal pump, control valve, pressure transmitter…) defines its attributes (name, unit, allowed values, required or not) and its default symbol. Built-in classes ship as a JSON library; users add attributes in Options. Checks and lists read attributes through the classes.
4. **One file, a database inside.** The `.dschema` file becomes one SQLite database: objects, attributes, classes, hierarchy, sheets, views, and a history table recording every change (who, when, old value, new value). The base for revisions and, later, several people working on one plant.
5. **DEXPI names.** Name classes and attributes after the open P&ID data standard (DEXPI) where it costs nothing, so import and export is a mapping, not a rewrite.

## 2. What the enterprise suites do (public documentation)

- **One object database for the whole plant life.** Each component is one object holding all its data; drawings, lists, datasheets and documents are views linked to it. Library ("base") objects are templates; each plant object takes its defaults from one, and classes group them.
- **A classified library.** A tree of base objects and classes (unit, location, device…) carries attributes and symbols; administration adds attributes and rules without code.
- **Rules.** Consistency is checked in real time against company rules, inconsistencies are marked on the drawing and listed; rules also propagate values (a line's size to its valves). A task list tracks what must be updated when shared data changes.
- **Many people, one plant.** Items are claimed by a project; work areas or working layers let people change a copy of the plant data and release it to the baseline, compare layers, and keep a project history.
- **Revisions and versions** of objects and documents, compared visually and as reports.
- **Integration** with instrumentation, electrical and 3D design: the logical P&ID is compared with the physical 3D model and isometrics (connections, valve sequence, attributes).
- **Open exchange.** DEXPI is the open P&ID exchange model (a practical implementation of ISO 15926; Proteus XML files; specification under CC BY 4.0); an open-source Python implementation (pyDEXPI) exists, but no free desktop editor writes it.

## 3. Diagnosis: where 0.1 stands

What 0.1 already has that carries over: the symbol drawings, routing, the frame, the 13 checks, hydraulics, lists, print, DXF, the examples and the test harness (engine tests, hardening, UI checks against the real engine).

What must change underneath:
- An item **is** its symbol (one `Item` per placement); a line **is** its drawn piece. Pipelines (runs) are inferred from matching line numbers, which caused the review's run bugs.
- Attributes are a free `data` map; the fields per category are Rust code (`fields.rs`); the checks read hard-coded keys.
- No hierarchy, no history beyond undo, a JSON file.

## 4. Proposal

### 4.1 The model (DEXPI-aligned names)

```
Project ─ PlantStructure tree: Plant › Unit › Area (defaults "Plant 1 › Unit 100 › Area 1")
Object  { id: UUID, class, tag, parent (hierarchy node), attributes }
  Equipment              (classes: CentrifugalPump, Tank, Vessel, ShellAndTubeHeatExchanger…)
  PipingNetworkSystem    a line number: size, fluid, class, conditions, flow…
    PipingNetworkSegment a stretch of it between two branch points or nozzles
      PipingComponent    in-line valves, reducers, strainers… (GateValve, CheckValve…)
  ProcessInstrumentationFunction   an instrument function: letters, loop
  InstrumentationLoopFunction      a loop
  Nozzle                 equipment connection points (from the class)
View    { sheet, object, symbol, position, turn, mirror, scale }   one per appearance
Connection { from object/nozzle, to object/nozzle }                topology, not geometry
Route   { segment, sheet, bend points }                            how a segment is drawn
```

- A line number becomes a real object (`PipingNetworkSystem`) instead of something inferred: renaming it renames every piece; a valve dropped on a line becomes a component of its segment. The run logic and its bugs go away.
- Topology (what connects to what) is separate from geometry (where it is drawn), so the same object can be on two sheets and the checks never depend on the drawing.
- Ids are UUIDs, so two copies of a project can later be merged.

### 4.2 The class library

- `classes.json`, built in: about 80 classes for today's symbols, with inheritance (Pump › CentrifugalPump), each with DEXPI class name, display name, default symbol, tag prefix, nozzles, and attributes `{ name (DEXPI where one exists), label, quantity/unit, type (number, text, choice, boolean), choices, required, default }`.
- Company classes and attributes added in Options are stored in the project (and can be saved as a company template).
- `fields.rs` disappears; the Inspector, lists and checks read the classes. Check 6 (missing data) reads "required".

### 4.3 The file: SQLite

- `rusqlite` with SQLite bundled (both permissive licences; to confirm with `cargo deny`). One file, opened in WAL mode, saved by the same atomic rule (a copy is written and renamed) or kept open with transactions — to decide (question 4).
- Tables: `meta` (format, units), `classes`, `attributes` (class attributes), `nodes` (hierarchy), `objects`, `values` (object, attribute, value, unit-free SI), `connections`, `sheets`, `views`, `routes`, `notes`, `settings`, `history` (seq, time, user, operation, object, attribute, old, new).
- Undo and redo come from the history table; revisions are named points in the history ("Rev A issued").
- 0.1 `.dschema` JSON files are read and converted on open (the examples too).

### 4.4 The window

- A **Plant** tab beside the palette: the hierarchy tree with every object (drag to move between units), filter, "show on sheet".
- Deleting a symbol asks: remove from this sheet, or delete the object everywhere. Objects not on any sheet are listed in the Plant tab ("not drawn").
- Lists group by plant, unit or area; the line list is one row per `PipingNetworkSystem`.
- Options › Classes: browse classes, add attributes.

### 4.5 What stays the same

Symbols, routing, the frame, print, DXF, hydraulics, the checks (re-written over the object model, same 13 rules and tests), the examples, the shell, the updater, the tests.

## 5. Size and order

This replaces the core under the window, so it is the 0.2 stage, in small releases (FAMILY §9):

| Release | Content |
| --- | --- |
| 0.2.0 | The object model and SQLite file; objects ≠ views; line numbers as objects; 0.1 files converted; every 0.1 test passes on the new core |
| 0.2.1 | The class library (JSON, inheritance, DEXPI names); Inspector, lists and checks read it; user attributes in Options |
| 0.2.2 | Plant hierarchy and the Plant panel; delete from sheet or object; an object on several sheets; lists grouped by plant/unit/area |
| 0.2.3 | History: every change recorded (who, when, old, new), undo from history, named revisions, compare two revisions as a report |
| 0.2.4 | DEXPI (Proteus XML) export, then import |

The old 0.2–0.5 roadmap (links to Pipe and Sim, PFD, instrument documents, templates) moves after this stage.

## 6. Questions for the user

1. **0.1.4.** Release it as the exhibition build on today's core (it is ready and tested), or hold it and show 0.2 when the object core is done?
2. **Scope of the stage.** Is 0.2.0–0.2.4 the right order, or should DEXPI export come earlier (it proves the object model against an outside standard)?
3. **Who works on a project.** Is "one person per file, with history and revisions" enough for the 0.2 stage, with several people on one plant (claims, working copies merged back) planned for later? Or should multi-user shape 0.2 now (a server, not a file)?
4. **The file.** Keep the familiar save button (the database is written as a whole on Save, recovery as now), or let the database save every change as it happens (no Save, like a database application)?
5. **Disciplines.** Beyond P&ID: is the next discipline instrumentation (index, loop diagrams, datasheets), electrical (motor lists, cable schedules), or the link to 3D (Datum Pipe)?
6. **Hierarchy names.** Project › Plant › Unit › Area, or your company's breakdown (site, plant, unit, system, area…)? Fixed levels or user-defined?
7. **Users and history.** "Who" in the history: the Windows user name, or a name typed in Options?

## 7. Agreed with the user (session 1, after 0.1.3)

- **Versions stay consecutive**: 0.1.4 (the held exhibition build) is released, then 0.1.5, 0.1.6, 0.1.7… — no jump to 0.2.0. The table in §5 is renumbered:

| Release | Content |
| --- | --- |
| 0.1.5 | Objects and views: every equipment item, valve and instrument is an object with its tag and data; symbols are views of it; a line number is an object (`PipingNetworkSystem`) owning its drawn pieces; plant hierarchy node on every object (defaults); the project file is one SQLite database with a history table; 0.1 JSON files open and convert |
| 0.1.6 | The class library as JSON (DEXPI names, inheritance, attributes with units, choices, required); Inspector, lists and checks read it; user attributes in Options |
| 0.1.7 | The Plant panel (hierarchy tree), delete from sheet or object, objects not drawn, lists grouped by plant/unit/area |
| 0.1.8 | Instrument datasheets generated from the class attributes: one per instrument, editable, printable, exportable — the user's daily pain point in the oil and gas work |
| next | History and revisions, DEXPI export and import, further datasheets (valves, equipment), loop diagrams |

- **The user's aim**: a flexible, free, open-source plant engineering tool for students and engineers who cannot afford the enterprise suites; datasheets and every document from one place. Built from scratch in Rust in the Datum way; open-source libraries only where needed (SQLite).
- **No reverse engineering** of any commercial product (the user suggested a reverse-engineering toolkit; it is not used on proprietary software). Only public documentation and open standards.
- **Defaults chosen** for the open questions (to revisit with the user): Save button kept (the database is written as a whole on Save, atomically; recovery as now); hierarchy levels Plant › Unit › Area with names the user edits; "who" in the history is the name in Options (default: the computer's user name); one person per file for now, with UUIDs and history so merging can come later.
