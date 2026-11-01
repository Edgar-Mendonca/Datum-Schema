# Changelog

## 0.4.1 — 2026-11-01

Values passed along a line, and the To do list.

**Values passed along.** A valve's or fitting's size and pipe class now follow the line it sits in: change the line and its components change with it. Control and relief valves, reducers, vents and drains are sized on their own. A value typed on the component is kept; the component follows its line again once it matches. The pipe class goes into a new field, *Pipe class (from its line)*: a valve's own *Class or rating* stays its rating (300#). The valve list shows both, as Pipe class and Rating.

**To do.** A new tab under the drawing lists what to look at after a change: a component whose own size or pipe class differs from its line's (take the line's value with one click), a datasheet changed since it was issued (open it), a sheet changed since its last issued revision (compare it).

## 0.4.0 — 2026-11-01

Pipe specs: the first of the piping rules.

**Pipe specs.** File › Options › **Pipe specs** holds the project's pipe classes (A1A, B2A…): each with its description, rating, material, sizes, schedule, design pressure and temperature limits and corrosion allowance, and the valves and fittings it allows, by size, each with a description and commodity code. Start from a typical carbon steel class (ASME B16.5 class 150, ½"–24", the usual valves and fittings), add an empty one, **read your specs from a CSV table** (Class, Component, From, To, Description, Code), or load a spec file; save a copy to use them in another project.

**Three new checks**, only when the project has specs: **21** a line's pipe class is not one of the specs; **22** a valve or fitting its line's class does not allow at that size (vents, drains and sample points by their own size); **23** a line outside its class's sizes, design pressure or design temperature.

**Take-off from the spec.** The valve and fitting take-off names each component from its class: its description and commodity code, ready for a requisition.

**Pipe class field.** A line's Pipe class in the Inspector offers the project's classes.

The Pump transfer example now carries its A1A spec.

## 0.3.4 — 2026-10-31

The user guide.

**User guide.** Help › **User Guide** (or File › User guide, or Tell Me) opens the guide: getting started, then every feature in chapters (drawing the plant; data and objects; checks and hydraulics with every check rule; lists and reports; instruments; process flow diagrams; symbols, standards and title blocks; revisions and review; working with other programs; examples), the keyboard shortcuts and the glossary. Search it, jump to a chapter from the contents, or **Print / Save as PDF** on A4. It is written from the same help as the ? buttons, so the two always agree, and any tool without a chapter of its own is still listed.

Getting started now names all the lists, the I/O summary, the take-off and the stream list.

## 0.3.3 — 2026-10-31

Review comments.

**Comments on the drawing.** Select what a comment is about (or nothing), then Check › **Comment** and click the sheet where it applies. A numbered pin marks it; the **Comments** tab under the drawing lists them, open ones first, with replies: Reply, Resolve or Reopen, Edit, delete, or click one to go to it and select what it is about. Comments are signed with your name from File › Options and dated. The status bar counts the open ones.

**Several reviewers.** Send each reviewer a copy of the project file. When it comes back, Check › **Import Review…** takes in their new comments, replies and resolutions without doubles (every comment and reply has its own id; the newest resolution wins), and Undo takes the import back.

**Print and export.** File › Print can include the review comments as a list; the Comments list exports to Excel or CSV. Pins are never printed on the sheets, so a review does not end up on an issued drawing.

## 0.3.2 — 2026-10-30

Your company's title block.

**Company title block.** File › Options › **Title block** › *Use a company title block on every sheet* starts from a copy of the standard block; change, add or remove cells, with the block drawn as you edit. The block can be any size up to 400 × 150 mm. Each cell has a caption and shows a field (company, project, client, unit, sheet title, drawing number, revision, scale, sheet n of m, date, drawn, checked, approved), fixed text, a field of your own, or the company logo (the one chosen under Datasheets). The revision table above the block follows its width, and the drawing area keeps clear of it.

**Fields of your own** (a contract number, a document code) are filled in for each project in the Inspector, with nothing selected, under Title block.

**Share it.** *Save a copy* writes the block as a file; *Load a title block* reads one into another project; *Use for new projects* starts every new project with it. It prints, exports to DXF (without the logo) and is kept in the project file.

## 0.3.1 — 2026-10-30

The valve and fitting take-off.

**Take-off.** Lists › **Take-off** (and its tab under the drawing) counts every valve, control valve, fitting and in-line element on the P&IDs by type, size, pipe class and rating, with their tags and sheets: the quantities for a material requisition. The size is the item's own, else that of the lines it sits in (a component between two sizes shows both, such as 3" × 2"); the pipe class comes from its line, the rating from the item. Each object counts once, even when drawn on two sheets; process flow diagrams are left out. Print it, or export it to Excel or CSV; click a row to select its items.

**Tell Me** now finds the take-off, the I/O summary and the stream list.

## 0.3.0 — 2026-10-30

Stream compositions with a component balance, the I/O summary, and three instrument checks.

**Stream compositions.** File › Options › **Stream components** lists the components of the project's streams, one per line. Every stream then has a mass fraction for each (Inspector › Composition; blank is none of it), the stream table on the PFD shows a row per component, and the Streams list a column.

**Component balance.** Check **17**: each component balances round every item on a PFD (within 0.5% of the mass in), and each stream's fractions add up to 1 (within 1%). Reactors are left out of the component balance, since they change their components. The Crude preheat example now carries compositions that close by hand.

**I/O summary.** Lists › **I/O Summary** (and its tab under the drawing) counts the inputs and outputs each control system takes: analogue inputs (transmitters), analogue outputs (control valves), discrete inputs (switches), discrete outputs (solenoids) and fieldbus devices, with the total and the total with 20% spare per kind. An instrument's control system comes from its Wiring; its kind from its I/O channel ("AI 01-03"), else its signal, else its letters and symbol. Print it, or export it to Excel or CSV; click a row to select its instruments.

**Instrument checks.** Check **18**: a transmitter's or gauge's range (typed as "0–10 bar", "-50 to 150 °C", "0-60 m3/h", or a bare range in the project's units) covers the operating pressure, temperature or flow of the line it measures. Check **19**: every control valve has a fail position (FO, FC or FL). Check **20**: once a loop's wiring is started, every device in it has its junction box, terminals, cable, control system and I/O channel (the control-system functions need only their system).

## 0.2.0 — 2026-10-30

A client's own symbols, old drawings to trace over, links to Datum Pipe and Datum Sim, process flow diagrams with heat and mass balances, and revision clouds.

**A client's symbol library.** File › Libraries › **Import a legend (DXF)** reads a client's legend sheet (save a DWG as DXF from any CAD program first). Every block placed on the legend becomes a symbol, described by the text beside it. Datum Schema suggests what each one stands for (a gate valve, a centrifugal pump…) from its description and block name, and puts its connection points where lines meet the drawing. Check the suggestions in the review table, edit the connection points by clicking on the drawing, and save the library: it stays on this computer for every project, and **Save a copy** / **Add a library file** share it with colleagues. The palette's **Library** tab places its symbols one by one.

**Drawing standard.** File › Options › **Drawing standard** draws a whole project with a library's symbols. The objects do not change: the same class, tag, data, checks and lists; lines stay on their nozzles (followed by name). Switching back to Datum Schema's symbols restores the drawing exactly. A project keeps its own copies of the library symbols it uses, so it opens on any computer, and DXF export keeps the client's block names.

**An old drawing behind a sheet.** Sheets › **Old Drawing** shows a DXF faintly behind the sheet, to redraw an old P&ID as a live one. A drawing larger than the sheet is scaled to fit and centred; set its size and position in the Inspector. It is never picked by mistake, and is printed and exported (on its own BACKGROUND layer in DXF) only when you ask.

**To Datum Pipe.** Select a process line, then Format › Send › **To Datum Pipe**: the line becomes a Datum Pipe model to start its stress analysis, with its size, schedule, design pressure and temperature, density and length, and its valves at their places along the run between two anchors. A P&ID has no routing in three dimensions, so the line goes as a straight run; add the bends and supports in Datum Pipe. What was assumed (a missing schedule, length or condition) is listed.

**To Datum Sim.** Lists › Loop Diagrams › **To Datum Sim** saves a control loop as a Datum Sim model to tune: set point, PID controller, valve, process and transmitter, with a trend of SP, PV and OP. The dynamics are typical values for the measured variable (flow, level, pressure, temperature), kept as variables at the top of the model to set from the plant; level loops are integrating, with an outflow.

**Process flow diagrams.** Sheets › **PFD Sheet** (or tick *Process flow diagram* for a sheet in the Inspector). On a PFD, process lines are streams, numbered 1, 2, 3… in diamonds, each with mass and volume flow, temperature, pressure, vapour fraction, density and enthalpy flow. A stream table is drawn along the bottom of the sheet in the project's units, and the **Streams** tab lists them (edit, Excel, CSV, print). Equipment on a PFD can be the same objects as on the P&IDs.

**Heat and mass balances.** Three new checks: **14** the mass flows into each item on a PFD equal those out (within 0.5%); **15** the enthalpy flows out less those in equal the item's duty or shaft power, or nothing (within 2%); **16** a P&ID line that names a PFD stream (Inspector › PFD stream) carries the stream's mass flow. The P&ID checks (sizes, check valves, relief, isolation) leave PFD sheets alone. The balances check the numbers typed; they are not a simulation.

**Revisions, comparison and clouds.** Inspector › Revisions › **Issue…** puts a row in the revision table and keeps the sheet as issued in the project file. **Compare with A…** lists what was added, removed or changed since (moved, rerouted, a new tag or data); click a change to go to it. **Revision clouds** draw a cloud round each changed area with the next revision in a triangle, printed and on the REVISION layer in DXF.

**Example.** File › Examples › **Crude preheat (PFD)**: feed pump, exchanger, fired heater and separator with eight streams whose heat and mass balances close by hand.

## 0.1.12 — 2026-10-30

Tell Me shows the drawing behind it, DEXPI import, a check valve's direction, bend points kept, the logo in Excel.

**Tell Me (Ctrl+K).** The command search now opens over the window, dimmed behind it, instead of hiding everything else.

**DEXPI import.** File › New › From a DEXPI file opens a P&ID saved in the DEXPI exchange format (XML) as a new project: equipment, valves and instruments with their tags, positions, turns and data (converted to SI from the file's units), each line number with its pieces, valves in their lines and connections to nozzles, and the signal and impulse lines. Files from Datum Schema come back exactly as they were exported; files from other programs are read as far as their content allows, with classes Datum Schema does not know drawn with a stand-in symbol and listed, and drawings larger than the sheet scaled to fit. The DEXPI export now also keeps each symbol's turn and mirror, the exact symbol and the line types, and writes attribute values at full precision.

**Check valves the right way round.** Check 8 now looks at which way a check valve on a pump's discharge is fitted: one that would stop the flow is reported. Right-click any in-line symbol › **Turn round** swaps its inlet and outlet, keeping its lines where they are.

**Bend points kept.** Dropping a valve onto a line keeps the bend points you placed: those before the valve stay on the first piece, those after it go to the second. Deleting the valve joins the pieces with all their bend points.

**Logo in Excel.** Datasheets exported to Excel carry your company logo (PNG or JPEG) on every sheet.

## 0.1.11 — 2026-10-30

Drawing fixes, the cause-and-effect matrix, the DEXPI exchange format, and loop diagrams in DXF.

**Drawing lines.** The line you are drawing is now shown under the mouse all the way: press on a port and drag, or click a port and move. A click on a port starts a line that follows the mouse, snaps to the port under it (ringed), and ends at the next click: on a port, on a line (a branch) or anywhere (an open end). Escape or a right-click drops it. (Before, the line only appeared once it was finished. The box you drag to select several things is shown again too.)

**Adding a symbol by name.** A double-click on the sheet (or Add Symbol) opens a small search box beside the point instead of a window over the drawing, so you can see where the symbol will go. Type a name or instrument letters, choose with the arrow keys, Enter places it; Escape or a click elsewhere closes it.

**Cause and effect.** Lists › Cause & Effect reads the interlocks on the P&ID into a matrix: causes down (the switches and transmitters that trip, with their trip settings), effects across (the valves and drives they act on, and what each does: a valve goes to its fail position, a drive stops), and in each cell the interlock that links them. A switch wired straight to a valve counts too. Click a tag to find it; print the matrix on A3 or export it to Excel. The Compressor package example now has a trip to show it: high-high drum level or high-high gas temperature stops the motor and opens the recycle valve.

**DEXPI export.** Sheets › DEXPI saves the sheet in the DEXPI P&ID exchange format (Proteus XML): equipment with its nozzles, each line number with its segments, centre lines, valves and connections, the instrument functions with their letters and loop numbers, the signal and impulse lines, and the attributes (DEXPI names in SI units). It is a first mapping, checked for well-formed XML and consistent references, not yet with other programs.

**Loop diagrams in DXF.** The loop diagram window has a DXF button: the frame, the drawing and the texts on their own layers.

## 0.1.10 — 2026-10-30

Datasheets from Excel, your company's datasheet form, and loop diagrams.

**Import datasheets from Excel.** Lists › Import Excel reads a workbook (.xlsx) and matches it to this project's datasheets by tag. It reads a table (a Tag column and one column per value, such as "Manufacturer" or "Set pressure (psig)") or one worksheet per datasheet, as Datum Schema exports them. Columns match by label, ignoring case and a unit in brackets; numbers are converted from the unit given (psi, °F, m³/h…) or from the project's units; choices match however their dashes were typed. A preview comes first: what will change, what is already the same, and what is not imported. Values that come from the line, the vessel or the drawing are never imported; where the file disagrees, the preview says so and where to change them. Blank cells change nothing. Untick what you do not want; the import is one step that Undo takes back. Tags with no datasheet and columns that match nothing are listed.

**Your company's datasheet form.** File › Options › Datasheets sets the form number, your logo (PNG, JPEG or SVG, up to 200 KB), a footer line, which rows the datasheet shows and the order of its sections. Hidden rows keep their data, are still imported and are still part of each issued revision. **Use this layout for new projects** makes it the default, for the examples too. The form number and footer go on screen, in print and in Excel; the logo on screen and in print.

**Loop diagrams.** Lists › Loop Diagrams draws every instrument loop (LT-101, LIC-101 and LV-101 make loop L-101) on an A3 sheet with the title block. Each field device is drawn with its own symbol, with its function, where it is mounted, its range or fail position, and its signal. It is wired through its junction box terminals and cable to its I/O channel in the control system, and on to the controller. Type the junction box, terminals, cable, control system and I/O channel for each device in the loop's Wiring table or in the Inspector; what is still missing is listed and noted on the drawing. Primary elements such as orifice plates are shown beside their transmitter; relief valves are not part of loops. Print one loop from its Print button, or all of them from File › Print. The Inspector has **Loop diagram…** for any instrument.

## 0.1.9 — 2026-10-30

Datasheet revisions: issue, track what changed, issue again.

**Issue a datasheet.** **Issue…** on a datasheet gives it a revision and a status: Preliminary, Issued for Enquiry, Issued for Purchase or As-built. Revisions run A, B, C… before purchase and 0, 1, 2… from purchase on, suggested for you; type your own if your company numbers differently. Each issue records the date, who issued it (File › Options › Your name) and a note, and it appears in the object's history.

**See what changed since.** The issue keeps what the datasheet said. From then on every value that changes, whether typed on the datasheet or changed on its line or vessel, is marked **Δ**; hover it to see what it was at the last revision. The marks clear at the next issue.

**Revision block.** Every datasheet shows, prints and exports its revisions, newest first. Printed datasheets carry the revision in the header and the title block; the Excel workbook has the revision on each sheet, a Δ column with the old values, and the revision state in its index.

**Datasheet index.** A new list (Lists › Datasheets) shows every datasheet with its revision, status, date, who issued it and how many values changed since. Click a row to open the datasheet. **Issue the … shown** issues every datasheet in the filtered list at once, each with its own next revision. Like every list, it sorts, filters, groups, prints and exports to Excel and CSV.

**File format.** Projects saved by 0.1.9 keep the issued revisions in the file, so 0.1.8 and earlier cannot open them; they ask you to update. Files from earlier versions open as before.

## 0.1.8 — 2026-10-29

Instrument datasheets, made from the drawing.

**A datasheet for every instrument.** Every instrument, primary element, control valve and relief device has a datasheet: **General** (tag, type, measured variable, loop, P&ID, area, line), **Process conditions**, the instrument's or valve's own data (range, signal, fail position, set pressure…), and a new **Specification** section (manufacturer, model, process connection, wetted and body materials, electrical connection, hazardous area, IP rating, power supply, accuracy; for control valves the required and rated Cv, characteristic, leakage class, actuator, positioner and trim; for relief devices the orifice area and relieving case). Attributes your company adds in Options › Classes get a section of their own.

**Process conditions from the P&ID.** The datasheet takes the fluid, flow, density, viscosity, operating and design pressure and temperature, size, schedule and pipe class from the line the instrument measures or sits in, from a primary element it is tapped to, or from the vessel it is mounted on. A controller takes its transmitter's. These values show where they come from and change where they live, so the datasheet and the P&ID never disagree.

**Open, type, print, export.** Open a datasheet from the Inspector (**Datasheet…**) or the Plant panel. Type the instrument's own values on it; every change is in the object's history. Print one datasheet from its Print button, or all of them from File › Print on A4 pages with the title block. **Excel › Datasheets** exports every datasheet, one worksheet each, with an index.

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
