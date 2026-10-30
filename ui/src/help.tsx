// "?" help: a small button beside a tool; a click opens a short card saying what the tool is for
// and how to use it. One place for every explanation, so the whole app teaches itself.
import { For, Show } from "solid-js";
import { openDialog } from "./store";
import { Dialog } from "./ui";

type Topic = { t: string; what: string; how?: string[]; tip?: string };

/** The check rules (brief §2.4): number, what it finds, level. Keep in step with checks.rs. */
export const RULES: [number, string, string][] = [
  [1, "Duplicate tag or duplicate line number in the project", "error"],
  [2, "Line end not connected (open end), unless it ends at a battery limit, a drain or vent, or an off-page connector", "error"],
  [3, "Off-page connector without a matching partner, or partners that disagree on the line number", "error"],
  [4, "Size changes along a line without a reducer", "error"],
  [5, "Pipe class changes without a spec-break mark", "warning"],
  [6, "Line or item with a missing tag, number, size or class", "warning"],
  [7, "Flow arrows on one line point against each other", "warning"],
  [8, "Pump discharge without a check valve", "warning"],
  [9, "Vessel or exchanger shell without a relief path (a safety valve, a rupture disc or a note)", "warning"],
  [10, "Control valve without isolation valves (a bypass is optional)", "note"],
  [11, "Controller (xIC) without a measurement and a final element in its loop", "warning"],
  [12, "Instrument tag letters that do not match its symbol type or loop", "note"],
  [13, "Line velocity or pressure drop outside its limits", "warning"],
];

export const HELP: Record<string, Topic> = {
  palette: { t: "Symbol palette", what: "Every symbol, by category: vessels, rotating equipment, heat transfer, valves, fittings, instruments, primary elements, line marks and connectors. They are Datum Schema's own drawings, following ISO 10628-2 (equipment, valves, lines) and ISA-5.1 (instruments).",
    how: ["Drag a symbol onto the sheet, or click it and then click the sheet (Shift keeps placing).", "Type in the search box to find one (“ball valve”, “pump”).", "Common instruments (PT, LIC, FIC…) come ready-lettered; they take the next free loop number."],
    tip: "An in-line symbol (a valve, a reducer, a strainer, a flowmeter) dropped onto a line goes into it." },
  draw: { t: "Drawing", what: "Select, move and connect.", how: ["Drag from a port (the small circles on a symbol) to another port to draw a line; drop it on a line to branch off, or anywhere to leave an open end.", "Lines route themselves around symbols. Drag a segment to move it; double-click a bend point to remove it.", "Box-select by dragging on empty sheet; Shift or Ctrl adds to the selection.", "Pan with the wheel or the middle button; zoom with Ctrl+wheel."] },
  edit: { t: "Edit", what: "Copy, paste and duplicate (copies take the next free tag: P-101A becomes P-101B), rotate in quarter turns (R), mirror (M) and delete. Every change can be undone." },
  units: { t: "Units", what: "How values are shown and typed: SI (bar, °C, m³/h, kg/m³, cP, m) or US (psi, °F, gpm, lb/ft³, cP, ft).", tip: "The file always keeps SI base units, so switching never rounds anything." },
  examples: { t: "Examples", what: "Worked projects: a pump transfer, a compressor package with an anti-surge loop, a steam heater loop, and “Find the faults”, a copy of the pump transfer with deliberate mistakes for the Checks panel to find." },
  lines: { t: "Lines", what: "Lines are drawn port to port and route themselves at right angles. Process lines (major, minor, utility) carry fluid, a line number and a flow arrow on every segment; signal lines (pneumatic, electrical, software or data, capillary) join instruments.",
    how: ["The line number is built from the size, fluid code, sequence and pipe class (Options › Line numbers sets the format).", "Reverse Flow flips the arrows.", "A line crossing another gets a small gap where it hops over."],
    tip: "One line number can run through several drawn pieces (across valves and sheets); data typed on one goes to all of them." },
  signals: { t: "Signal lines", what: "ISA-5.1 conventions: pneumatic signals have double slashes, electrical signals are dashed, software or data links have small circles, capillaries have crosses." },
  offpage: { t: "Off-page connectors", what: "A pair of connectors carries a line from one sheet to another. Each shows the sheet where the line continues, and the line number.", how: ["Place a connector on each sheet and join the line to it.", "Pair them (right-click › Pair with…, or the Inspector).", "Double-click a connector to jump to its partner."], tip: "Check 3 makes sure every connector has a partner and both carry the same line number." },
  notes: { t: "Notes", what: "Free notes, notes with a leader arrow, and Hold clouds round areas of the drawing that are not final." },
  tags: { t: "Tags", what: "New items get the next free tag for their prefix: numbers run 101, 102 … on sheet 1, 201 … on sheet 2. Instruments take their letters and the next free loop number.", how: ["Renumber numbers every tag again, per prefix and per sheet, left to right; items sharing a number with letter suffixes (P-101A, P-101B) stay together."] },
  isa: { t: "Instrument letters", what: "ISA-5.1 letter codes: the first letter is the measured variable (F flow, L level, P pressure, T temperature, A analysis, U several), the next ones what the instrument does (I indicate, T transmit, C control, V valve, A alarm, S switch, E element, H high, L low).", tip: "A loop is the instruments sharing a loop number: LT-101 measures, LIC-101 controls, LV-101 acts." },
  checks: { t: "Checks", what: "Datum Schema checks the drawing as you work. Every issue has a level (error, warning, note), a short reason, and a click that takes you to it. The status bar shows the count.",
    how: RULES.map(r => `${r[0]}. ${r[1]} (${r[2]}).`),
    tip: "Each rule can be switched off in Options › Checks, to follow company practice." },
  rules: { t: "Check rules", what: "Switch each rule on or off for the open project (File › Options › Checks); the choice is saved in the project." },
  hydraulics: { t: "Line hydraulics", what: "For every line with a flow, a density and a viscosity: the inside diameter (from the size and schedule; ASME B36.10M/B36.19M dimensions), the velocity v = Q/A, the Reynolds number Re = ρvD/μ, the Darcy friction factor (the Colebrook equation for turbulent flow, 64/Re for laminar) and the pressure drop Δp = f (L/D) ρv²/2 per 100 m and over the line's length plus its fittings allowance.",
    how: ["The badge on the line is ✓ within the limits, ! close to or just outside them, ✗ well outside.", "The limits depend on the service: liquid, pump suction or discharge (worked out from the pump the line joins), gravity drain, gas, steam or two-phase. Options › Hydraulic limits sets them."],
    tip: "Sources: Darcy–Weisbach; C. F. Colebrook, J. Inst. Civil Eng. 11 (1939) 133–156; the same method as Datum Calc's pipe pressure-drop pack, with the same answers. The results are a sizing aid: they are checked by the engineer." },
  lists: { t: "Lists", what: "The line list, equipment list, instrument index, valve list and datasheet index, always generated from the drawing.", how: ["Click a column to sort; filter and group above the list.", "Click a row to find it on the drawing.", "Edit a cell and the drawing follows.", "Export to Excel or CSV, or print with the title block."] },
  export: { t: "Export", what: "Print to PDF (the sheets at their size or fitted to A3, plus the lists and the legend), save a sheet as SVG, or export DXF (R12: a layer per line type and symbol category, symbols as blocks with their tag as an attribute, text as text, the border and title block included)." },
  sheets: { t: "Sheets", what: "A project holds several sheets, each with a size (A3, A1, ANSI B, ANSI D), a border with grid zones (A–H across, 1–8 down), a title block and a revision table. A legend sheet shows every symbol the project uses.", how: ["The sheet tabs are under the drawing; right-click one for sheet commands.", "With nothing selected, the Inspector edits the sheet and the title block."] },
  view: { t: "View", what: "Fit the sheet (F), zoom with Ctrl+wheel or the buttons at the bottom right, show or hide the grid." },
  layers: { t: "Layers", what: "Show or hide process lines and equipment, utility lines, instruments and signals, and notes and line marks." },
  select: { t: "Select", what: "Select everything of one type, or a whole line number; the line number is highlighted on every sheet." },
  plant: { t: "Plant", what: "The plant hierarchy, plant › unit › area, with every object in the project: equipment, valves, instruments, line numbers. An object is the master of its tag and data; its symbols on the sheets are views of it.", how: ["Click an object to go to its symbol; objects not drawn yet say so.", "Drag an object onto a sheet to draw it there; an object can be drawn on several sheets.", "Drag an object onto an area to move it; the Inspector also has its area.", "Right-click a level to add units and areas, rename (or double-click) and delete; + Plant adds a plant."], tip: "Deleting the last symbol of an object asks whether to delete the object or only remove it from the sheet." },
  datasheets: { t: "Datasheets", what: "Every instrument, control valve and relief device has a datasheet: its own data (range, signal, fail position, materials, manufacturer, model…) with the process conditions of the line it measures or sits in (fluid, flow, density, viscosity, operating and design pressure and temperature, line size and class). The attributes come from the class library; attributes you add in Options › Classes get their own section.",
    how: ["Open it from the Inspector (Datasheet…) or the Plant panel (right-click an instrument).", "Type the instrument's own values on the datasheet; grey rows come from the line or the drawing and change there, so the datasheet and the P&ID never disagree.", "Print one datasheet from its Print button, or all of them from File › Print (Datasheets).", "Excel › Datasheets exports every datasheet, one worksheet each, with an index.",
      "Issue… gives the datasheet a revision and a status (Preliminary, Issued for Enquiry, Issued for Purchase, As-built). Revisions run A, B, C… before purchase and 0, 1, 2… from purchase on; you can type your own.",
      "After an issue, every value changed since is marked Δ (hover it for what it was) until the next issue; the revision block lists every issue with its date, who issued it and a note.",
      "Options › Datasheets sets your company's form: its number, your logo, a footer, which rows it shows and the order of the sections. Hidden rows keep their data.",
      "Import from Excel (Lists › Import Excel) reads values from a workbook by tag, with a preview first.",
      "The Datasheet index (Lists › Datasheets) shows every datasheet with its revision, status and how many values changed since; click a row to open it, or issue all the rows shown at once."],
    tip: "A controller takes the line of its loop: LIC-101 shows the conditions of the line LT-101 measures." },
  import: { t: "Import datasheets from Excel", what: "Reads datasheet values from an Excel workbook (.xlsx), such as a vendor's datasheets or an older project's instrument index, and matches them to this project's datasheets by tag. Nothing changes until you have seen the preview.",
    how: ["Two layouts are read: a table with a Tag column and one column per value (“Manufacturer”, “Set pressure (bar)”), or one worksheet per datasheet as Datum Schema exports them (label, value, unit).", "Columns and rows match by their label, ignoring case and a unit in brackets; numbers are converted from the unit given (psi, °F, m³/h…), or from the project's units.", "The preview lists what would change, what is already the same, and what is not imported: values that come from the line, the vessel or the drawing (change them there), and values that cannot be used, such as a choice not on the list.", "Untick anything you do not want, then Import. The whole import is one step: Undo takes it back."],
    tip: "Columns that match nothing are listed: add them as attributes in Options › Classes and import again." },
  loops: { t: "Loop diagrams", what: "One diagram per instrument loop (the instruments sharing a first letter and a loop number, such as LT-101, LIC-101 and LV-101), drawn from the P&ID on an A3 sheet with the title block: each field device with its own symbol, wired through its junction box terminals and cable to its I/O channel in the control system, and on to the controller.",
    how: ["Open them from Lists › Loop Diagrams, or from an instrument's Inspector (Loop diagram…).", "Type each device's junction box, terminals (such as 1, 2), cable, control system and I/O channel in the Wiring table, or in the Inspector; the diagram follows.", "What is still missing is listed under the diagram and in a To complete note on it.", "Print one loop from its Print button, or all of them from File › Print (Loop diagrams)."],
    tip: "Primary elements such as orifice plates are shown beside their transmitter without wiring; relief valves are not part of loops." },
  history: { t: "History", what: "Every change to an object is recorded with who made it, when, and the old and new value. The history is saved in the project file, so it survives closing and reopening; undo and redo are recorded too." },
  inspector: { t: "Inspector", what: "The selected item's data, with units: tag, service, design and operating conditions, sizes, connections, and for lines the hydraulics. With nothing selected, the sheet, its revisions and the title block." },
  "cat:vessel": { t: "Vessels", what: "Vertical vessels, horizontal drums, tanks, columns (trays or packing), a separator with a boot, a filter vessel, a knock-out drum and a silo. Their nozzles are ports; each carries design and operating conditions, material, volume and a relief note." },
  "cat:rotating": { t: "Rotating equipment", what: "Centrifugal, positive-displacement and vertical in-line pumps (with suction and discharge ports), centrifugal, reciprocating and screw compressors, fans, a steam turbine, an electric motor and an agitator." },
  "cat:heat": { t: "Heat transfer", what: "Shell-and-tube exchangers (fixed tubesheet, U-tube, kettle), a plate exchanger, an air cooler, a fired heater, an electric heater, a cooling tower and a jacketed vessel. Shell and tube sides have their own ports." },
  "cat:valve": { t: "Manual valves", what: "Gate, globe, ball, butterfly, plug, needle, diaphragm, check, non-slam check and three-way valves. They go into lines and carry a size, class, operator and normal position (NO, NC, LO, LC)." },
  "cat:control": { t: "Control and relief valves", what: "Control valves with pneumatic, motor or solenoid actuators (with their fail position: FO, FC, FL), a pressure safety valve, a pressure regulator, a rupture disc and a breather valve." },
  "cat:fitting": { t: "Fittings and specials", what: "Concentric and eccentric reducers, a blind flange, a spectacle blind, a ring spacer, Y and basket strainers, a steam trap, a sight glass, a flame arrester, a flexible hose, a restriction orifice, an expansion joint, a drain, a vent and a sample point." },
  "cat:instrument": { t: "Instruments", what: "ISA-5.1 bubbles: field-mounted (a plain circle), control-room panel (a line through it), local panel (a dashed line), shared display or DCS (a circle in a square), programmable logic (a diamond in a square), and interlock logic (a diamond). The tag letters and loop number are written inside." },
  "cat:primary": { t: "Primary elements", what: "An orifice plate, magnetic, Coriolis, vortex and ultrasonic flowmeters (in-line), and a thermowell (tapped onto a line)." },
  "cat:mark": { t: "Line marks", what: "Insulation, steam and electric tracing and jacketing marks, a spec break, a battery limit, a tie-in point and a package boundary. Spec breaks, battery limits and tie-ins go into lines." },
  "cat:connector": { t: "Connectors", what: "Off-page connectors carry a line to another sheet." },
};

/** The small "?" beside a tool. */
export const Help = (p: { k: string }) => <button class="help" title={`What is ${HELP[p.k]?.t ?? "this"}?`} onClick={e => { e.stopPropagation(); e.preventDefault(); helpCard(p.k); }}>?</button>;

export const helpCard = (k: string) => openDialog(() => {
  const h = HELP[k];
  return (
    <Dialog title={h?.t ?? "Help"}>
      <div class="helpcard"><p>{h?.what}</p>
        <Show when={h?.how?.length}><h4>How to use it</h4><ul><For each={h!.how}>{x => <li>{x}</li>}</For></ul></Show>
        <Show when={h?.tip}><p class="muted">{h!.tip}</p></Show></div>
    </Dialog>
  );
});
