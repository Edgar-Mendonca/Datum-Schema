# Changelog

## 0.1.7 — 2026-10-29

The Plant panel: every object in its place in the plant, drawn or not.

**Plant panel.** Beside the symbol palette, the **Plant** tab shows the plant hierarchy (plant › unit › area) with every object in the project: equipment, valves, instruments and line numbers. Click an object to go to its symbol. Add plants, units and areas, rename them (double-click) and remove empty ones. Drag an object onto another area to move it; the Inspector shows and changes an object's area too.

**Draw an object anywhere.** Drag an object from the Plant panel onto a sheet to draw it there: the same object, with one tag and one set of data, can be drawn on several sheets.

**Delete: the object or only its drawing?** Deleting the last symbol of an object asks whether to delete the object with its data or only remove it from the sheet. Objects kept this way stay in the Plant panel, marked "not drawn", ready to be drawn again.

**Lists.** Every list has an Area column, so lists can be grouped by area.

## 0.1.6 — 2026-10-29

Classes: every kind of object is defined by data, and you can add your own attributes.

**The class library.** Every object belongs to a class (centrifugal pump, gate valve, control valve, instrument function, line number…), named after the open P&ID data standard (DEXPI) where a matching class is known. Classes inherit from the classes above them: a centrifugal pump has a pump's attributes (flow, head, power) and an equipment item's (service, design and operating conditions, material). The Inspector shows the class of the selected object and its attributes.

**Your own attributes.** File › Options › Classes lists every class and its attributes. Add your company's attributes to any class (text, a choice list, or a value with units); they reach every class under it and are saved with the project. Mark one **required** and check 6 lists the objects where it is still empty.

## 0.1.5 — 2026-10-29

The first step to an object-based plant engineering tool: plant objects, a database file and a history.

**Plant objects.** Every equipment item, valve and instrument is now an object with its own tag and data; the symbol on the sheet is a view of it. A line number is an object too, owning every drawn piece of that line: a line continued through a valve, or carried to another sheet by a pair of off-page connectors, keeps its line number, and data typed on any piece belongs to the whole line. Where a line's number changes (after a reducer), **New line number from here** gives a piece a number of its own.

**Plant hierarchy.** Every object sits in a plant › unit › area tree (Plant 1 › Unit 100 › Area 1 to start with), ready for the Plant panel in a coming release.

**The project file is a database.** Projects save as one SQLite database: objects, attribute values, the hierarchy, sheets, symbols and lines, each in its own table, which any SQLite tool can read. Saving is still atomic. Projects from earlier versions open and convert.

**History.** Every change to an object is recorded with who made it, when, and the old and new value, and saved with the project. The Inspector shows the history of the selected object. Your name for the history is in Options (the computer's user name otherwise).

## 0.1.4 — 2026-10-29

The exhibition build.

**Checked end to end.** Before this release every example was opened, checked for errors, printed and exported to DXF in the window, and the DXF files were read back by an independent reader with no errors.

**README.** A two-minute demo of Find the faults, the line list and its hydraulics, printing and DXF, and the screenshots.

## 0.1.3 — 2026-10-29

Fixes from an independent review of the engine, and polish.

**Fixes.**
- A valve with three or more lines on it no longer splits one line number into two (it reported a duplicate line number).
- Copying and pasting a pipeline keeps one new line number across its valves.
- The pump check valve and control valve isolation checks follow a line across sheets through its off-page connectors.
- Deleting a valve in a small loop works again, and joining the two halves keeps data only the second half had.
- A project that has used up its ids says so instead of losing what was added.
- CSV quotes text with a carriage return; Excel cells no longer show a stray apostrophe.
- DXF: symbol letters (M, S, RO…) stay upright like in the window; characters a DXF cannot hold are written as ?; the hydraulic badges are written OK, ! and X.

**Lists.** Printed lists have a title block on every page.

**Window.** An instrument's loop number shows from its tag; more checks of the window run before every release (keyboard editing, units, Tell Me).

## 0.1.2 — 2026-10-29

Fast with big drawings; hydraulic badges in colour.

**Speed.** Lines route much faster: a sheet with 500 symbols and 500 lines is drawn again in well under a tenth of a second after each change (it took seconds on a crowded sheet).

**Badges in colour.** The hydraulic badge on each line is green ✓ within its limits, amber ! close to them, red ✗ well outside.

**Line numbers.** Selecting a line highlights its line number through every drawn piece, and it stays highlighted on the other sheets.

**Line list.** The flow and the hydraulic results come right after From and To.

## 0.1.1 — 2026-10-29

The drawing now checks itself, lists itself and sizes its lines; the examples, printing and DXF.

**Examples.** File › Examples: a pump transfer (tank, duty and standby pumps with strainers and check valves, a level-controlled vessel with its safety valve), a compressor package with its anti-surge loop, a steam heater with its temperature loop and condensate return, and **Find the faults**, the pump transfer with planted mistakes. Each example opens with no errors.

**Checks.** Thirteen rules run on every change and fill the Checks panel: duplicate tags and line numbers, open ends, off-page connectors without a partner, size changes without a reducer, class changes without a spec break, missing tags or line data, flow arrows against each other, pumps without a check valve, vessels without relief, control valves without isolation, controllers without a measurement or a final element, instrument letters that do not fit, and lines outside their hydraulic limits. Click an issue to go to it; the status bar counts errors and warnings. Rules can be switched off in Options.

**Line hydraulics.** Every line with a flow, a density and a viscosity gets its inside diameter (from its size and schedule), velocity, Reynolds number, friction factor (Colebrook) and pressure drop per 100 m and over its length, with a ✓, ! or ✗ badge against the limits for its service (pump suction and discharge are recognised). The limits are editable in Options. Results are a sizing aid, checked by the engineer.

**Lists.** The line list, equipment list, instrument index and valve list are always generated from the drawing. Sort, filter and group them, click a row to find it, edit a cell and the drawing follows; export to Excel or CSV.

**Sheets and connectors.** Off-page connectors pair across sheets and show where the line continues; double-click one to jump to its partner. A legend sheet shows every symbol and line type the project uses. Notes, leader notes and Hold clouds.

**Print and export.** Print to PDF: the sheets at their size or fitted to A3, the lists on A3 with page numbers, and the legend. Save a sheet as SVG. DXF (R12) with a layer per line type and symbol category, symbols as blocks with their tag as an attribute, the border and title block included.

**Drawing.** Lines carry one line number and one hydraulic badge per sheet; Fit shows the drawing, Sheet the whole sheet. Manual valves are tagged HV.

## 0.1.0 — 2026-10-29

The first release: the window, the sheet, symbols and lines, and the updater.

**The window.** The family window in magenta: the ribbon (Home, Draw, Symbols, Lines, Instruments, Check, Lists, Sheets, View, Help and the Format tab for a selection), the File page, Options, the symbol palette on the left, the sheet in the middle, the Inspector on the right, and a panel below for the checks and the lists. Light, dark and automatic themes.

**Sheets.** A blank A3 sheet with its border, grid zones (A–H across, 1–8 down), title block and revision table. Sheets can be A3, A1, ANSI B or ANSI D; add sheets and a legend sheet from the tabs under the drawing. With nothing selected, the Inspector edits the sheet, its revisions and the title block.

**Symbols and lines.** About 80 symbols of our own: vessels, pumps and compressors, exchangers, manual, control and relief valves, fittings, instrument bubbles, flowmeters, line marks and off-page connectors. Drag one from the palette or double-click the sheet and type its name; it gets the next free tag. Drag from a port to another to draw a line; lines route themselves at right angles around symbols, carry a flow arrow on every segment and hop over each other. A valve dropped on a line goes into it, and both halves keep the line's data.

**Files.** Projects save as `.dschema` files and open again, with recovery of unsaved work. Every save is written safely.

**Updates.** Datum Schema checks for new versions when it starts and installs them from File › Updates, after checking the download against its checksum.
