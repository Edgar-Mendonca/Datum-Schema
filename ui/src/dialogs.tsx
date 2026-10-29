// Dialogs: saving changes, closing, recovery, adding a symbol by name, pairing off-page connectors,
// and the help pages (shortcuts, getting started, glossary).
import { For, Show, createSignal, createMemo, onMount } from "solid-js";
import { Out, Pt } from "./api";
import { openDialog, closeDialog, call, out, setOut, lib, proj, itemOf, act } from "./store";
import { C } from "./cmds";
import { Dialog } from "./ui";
import { PRESETS, SymPic } from "./palette";

export const keepChanges = (then: () => unknown) => {
  if (!out()?.dirty) { then(); return; }
  openDialog(() => (
    <Dialog title="Save changes?" okText="Save" cancel="Cancel" ok={async () => { await C.save(); if (out()?.dirty) return false; setTimeout(then); }}
      extra={<button class="btn" onClick={() => { closeDialog(); setTimeout(then); }}>Don't save</button>}>
      <p>“{out()?.p.name}” has changes that are not saved.</p>
    </Dialog>
  ));
};

export const confirmClose = (quit: () => void) => openDialog(() => (
  <Dialog title="Save changes?" okText="Save" cancel="Cancel" ok={async () => { await C.save(); if (!out()?.dirty) quit(); }}
    extra={<button class="btn" onClick={() => { closeDialog(); quit(); }}>Don't save</button>}>
    <p>“{out()?.p.name}” has changes that are not saved.</p>
  </Dialog>
));

export const confirm = (text: string, okText: string, run: () => void) => openDialog(() => (
  <Dialog title="Are you sure?" okText={okText} ok={() => { run(); }}><p>{text}</p></Dialog>
));

/** Double-click on the sheet: type to find a symbol (or an instrument by its letters) and add it there. */
export const quickInsert = (at: Pt) => openDialog(() => {
  const [q, setQ] = createSignal(""), [k, setK] = createSignal(0);
  const list = createMemo(() => {
    const s = q().trim().toLowerCase();
    const syms = (lib()?.symbols ?? []).filter(x => x.key !== "unknown" && (!s || x.name.toLowerCase().includes(s) || x.cat_name.toLowerCase().includes(s))).map(x => ({ sym: x.key, tag: undefined as string | undefined, name: x.name, cat: x.cat_name }));
    const pre = PRESETS.filter(p => s && (p[0].toLowerCase() === s || p[2].toLowerCase().includes(s))).map(p => ({ sym: p[1], tag: p[0], name: `${p[2]} (${p[0]})`, cat: "Instruments" }));
    return [...pre, ...syms].slice(0, 14);
  });
  const go = (i: number) => { const d = list()[i]; if (!d) return; closeDialog(); const g = proj()?.settings.grid || 2.5; C.place(d.sym, [Math.round(at[0] / g) * g, Math.round(at[1] / g) * g], null, d.tag); };
  let input!: HTMLInputElement;
  onMount(() => input.focus());
  return (
    <div class="ov top" onMouseDown={e => e.target === e.currentTarget && closeDialog()}>
      <div class="pal">
        <input ref={input} placeholder="Add a symbol: type its name (ball valve, pump, LIC…)" value={q()} onInput={e => { setQ(e.currentTarget.value); setK(0); }}
          onKeyDown={e => { if (e.key === "ArrowDown") { e.preventDefault(); setK(Math.min(list().length - 1, k() + 1)); } else if (e.key === "ArrowUp") { e.preventDefault(); setK(Math.max(0, k() - 1)); } else if (e.key === "Enter") { e.preventDefault(); go(k()); } else if (e.key === "Escape") closeDialog(); }} />
        <div class="pl"><For each={list()}>{(d, i) => <button classList={{ on: i() === k() }} onMouseEnter={() => setK(i())} onClick={() => go(i())}><Show when={lib()?.symbols.find(s => s.key === d.sym)}>{s => <SymPic s={s()} size={22} letters={d.tag} />}</Show><span>{d.name}</span><kbd>{d.cat}</kbd></button>}</For></div>
      </div>
    </div>
  );
});

/** Pair an off-page connector with one on another sheet. */
export const pairDialog = (id: number) => openDialog(() => {
  const me = itemOf(id);
  const others = () => (proj()?.items ?? []).filter(i => i.sym === "offpage" && i.id !== id && i.sheet !== me?.sheet);
  const [pick, setPick] = createSignal<number>(me?.link ?? 0);
  const sheetNo = (s: number) => proj()?.sheets.find(x => x.id === s)?.number ?? "?";
  const lineAt = (iid: number) => { const l = proj()?.lines.find(l => (l.a.at === "port" && l.a.item === iid) || (l.b.at === "port" && l.b.item === iid)); return l ? (proj()?.objects.find(o => o.id === l.obj)?.data.seq ? String(proj()!.objects.find(o => o.id === l.obj)!.data.seq) : "") : ""; };
  return <Dialog title="Pair off-page connectors" okText="Pair" ok={() => act({ Link: { a: id, b: pick() } }).then(o => (o ? undefined : false))}
    extra={<Show when={me?.link}><button class="btn" onClick={() => { act({ Link: { a: id, b: 0 } }); closeDialog(); }}>Unpair</button></Show>}>
    <p class="muted">A pair of connectors carries a line from one sheet to another. Clicking one jumps to its partner; the checks make sure both sides carry the same line number.</p>
    <Show when={others().length} fallback={<p>There is no off-page connector on another sheet yet. Add one there first (Lines › Off-Page Connector).</p>}>
      <table class="tbl"><tbody><For each={others()}>{o => <tr class="f" classList={{ on: pick() === o.id }} onClick={() => setPick(o.id)}><td><input type="radio" checked={pick() === o.id} /></td><td>Sheet {sheetNo(o.sheet)}</td><td class="muted">at {Math.round(o.x)}, {Math.round(o.y)} mm{lineAt(o.id) ? ` · line ${lineAt(o.id)}` : ""}{o.link && o.link !== id ? " · paired elsewhere" : ""}</td></tr>}</For></tbody></table>
    </Show>
  </Dialog>;
});

export const shortcuts = () => openDialog(() => (
  <Dialog title="Keyboard shortcuts">
    <table class="tbl keys"><tbody><For each={[
      ["Save / Save As", "Ctrl+S / Ctrl+Shift+S"], ["Open", "Ctrl+O"], ["New", "Ctrl+N"], ["Print", "Ctrl+P"], ["Tell me what you want to do", "Ctrl+K"],
      ["Undo / Redo", "Ctrl+Z / Ctrl+Y"], ["Delete the selection", "Del"], ["Copy / Paste / Duplicate", "Ctrl+C / Ctrl+V / Ctrl+D"], ["Select everything on the sheet", "Ctrl+A"],
      ["Rotate / Mirror the selected symbols", "R / M"], ["Nudge the selection (Shift: further)", "Arrow keys"], ["Draw lines", "L (Esc stops)"],
      ["Add a symbol by name", "Double-click the sheet"], ["Draw a line", "Drag from a port (the small circles)"], ["Move a line's segment", "Drag it"], ["Remove a bend point", "Double-click it"],
      ["Jump to an off-page partner", "Double-click the connector"], ["Fit the sheet", "F"], ["Pan / zoom", "Wheel or middle drag / Ctrl+wheel"], ["This list", "F1"],
    ] as [string, string][]}>{([a, b]) => <tr><td>{a}</td><td><kbd>{b}</kbd></td></tr>}</For></tbody></table>
  </Dialog>
));

export const gettingStarted = () => openDialog(() => (
  <Dialog title="Getting started" wide>
    <div class="helpcard">
      <p>Datum Schema draws P&IDs and process flow diagrams where every symbol and line carries its data and knows what it is connected to. From the drawing it writes the line list, the equipment list, the instrument index and the valve list, checks the drawing as you work, and sizes every line.</p>
      <h4>Draw the plant</h4>
      <ol>
        <li>Drag symbols from the <b>palette</b> on the left onto the sheet, or double-click the sheet and type a name (“ball valve”, “LIC”). Equipment gets the next free tag (P-101, V-101); instruments get their letters and the next loop number.</li>
        <li>Draw a <b>line</b> by dragging from a port (the small circles on a symbol) to another port. Drop it on a line to branch off it, or anywhere to leave an open end. Lines route themselves around symbols; drag a segment to move it.</li>
        <li>Drop an in-line symbol (a valve, a reducer, a strainer, a flowmeter) <b>onto a line</b>: it goes into the line, and both halves keep the line's data.</li>
        <li>Carry a line to another sheet with a pair of <b>off-page connectors</b>; double-click one to jump to its partner.</li>
      </ol>
      <h4>Give it its data</h4>
      <ol>
        <li>Select anything and fill its data in the <b>Inspector</b> on the right: the tag, the service, design and operating conditions, sizes. Values show in SI or US units (Home › Units); the file keeps SI.</li>
        <li>A line's number is built from its size, fluid code, sequence and pipe class (3"-P-1001-A1A). Give it a flow, a density and a viscosity and Datum Schema works out its velocity and pressure drop.</li>
      </ol>
      <h4>Check it and list it</h4>
      <ol>
        <li>The <b>Checks</b> panel below lists what needs attention: duplicate tags, open ends, size changes without a reducer, a pump discharge without a check valve, and more. Click one to go to it.</li>
        <li>The <b>line list</b>, <b>equipment list</b>, <b>instrument index</b> and <b>valve list</b> are always in step with the drawing; export them to CSV or Excel, or print them.</li>
        <li>Print the sheets to PDF, save a sheet as SVG, or export DXF for any CAD program.</li>
      </ol>
      <p class="muted">Every tool has a small ? beside it. Hydraulic results are a sizing aid, checked by the engineer.</p>
    </div>
  </Dialog>
));

export const glossary = () => openDialog(() => {
  const [q, setQ] = createSignal("");
  const G: [string, string][] = [
    ["P&ID", "Piping and instrument diagram: the plant's equipment, piping, valves and instruments, with how they connect and are controlled."],
    ["PFD", "Process flow diagram: the main equipment and streams of a process, without every valve and instrument."],
    ["Tag", "The name of an item: P-101A for a pump, V-101 for a vessel, LIC-101 for a level controller."],
    ["Line number", "The name of a pipeline, built from its size, fluid code, sequence number and pipe class, such as 3\"-P-1001-A1A."],
    ["Pipe class (spec)", "The piping specification a line is built to: materials, pressure rating, wall thickness."],
    ["Spec break", "The mark where the pipe class changes along a line."],
    ["Battery limit", "The edge of the plant area the drawing covers; a line may end there."],
    ["Off-page connector", "The arrow that carries a line to another sheet. It names the sheet where the line continues."],
    ["In-line item", "A symbol that sits in a line: valves, reducers, strainers, flowmeters. Dropped on a line, it splits it."],
    ["Loop", "The instruments that work together on one variable, sharing a loop number: LT-101, LIC-101, LV-101."],
    ["Instrument letters (ISA-5.1)", "The first letter is the measured variable (F flow, L level, P pressure, T temperature…); the next ones are what the instrument does (I indicate, T transmit, C control, V valve, A alarm, S switch, H high, L low)."],
    ["Bubble", "The circle of an instrument. A plain circle is in the field; a line through it means a control-room panel; a circle in a square is a shared display (DCS); a diamond in a square is programmable logic (PLC)."],
    ["Fail position", "What a control valve does when it loses its air or power: fail open (FO), fail closed (FC) or fail last (FL)."],
    ["Normal position", "How a manual valve is normally left: normally open (NO), normally closed (NC), locked open (LO), locked closed (LC)."],
    ["Velocity", "The mean speed of the fluid in the pipe: the volume flow divided by the inside area."],
    ["Reynolds number", "ρ v D / μ: below about 2000 the flow is laminar, above about 4000 turbulent."],
    ["Friction factor", "The Darcy friction factor f, from the Colebrook equation for turbulent flow (64/Re for laminar)."],
    ["Pressure drop", "Δp = f (L/D) ρ v²/2 (Darcy–Weisbach), shown per 100 m (or 100 ft) and over the line's length plus its fittings allowance."],
    ["Fittings allowance", "Extra length for bends, tees and valves, as a percentage of the straight length."],
    ["Hold", "A cloud round an area of the drawing that is not final yet."],
    ["DXF", "A drawing exchange format that CAD programs open. Datum Schema writes the R12 kind, the most widely read."],
  ];
  return <Dialog title="Glossary"><input placeholder="Search" value={q()} onInput={e => setQ(e.currentTarget.value)} style={{ width: "100%", "margin-bottom": "8px" }} />
    <dl class="gloss"><For each={G.filter(g => (g[0] + g[1]).toLowerCase().includes(q().toLowerCase()))}>{([t, d]) => <><dt>{t}</dt><dd>{d}</dd></>}</For></dl></Dialog>;
});

/** Recover unsaved work after a crash. */
export const recoverDialog = (name: string, saved: string, done: () => void) => openDialog(() => (
  <Dialog title="Recover unsaved work?" okText="Recover" cancel="Not now" ok={async () => { const r = await call<Out>("recover", { keep: true }); if (r) { setOut(r); done(); } }}
    extra={<button class="btn" onClick={() => { call("recover", { keep: false }); closeDialog(); }}>Discard</button>}>
    <p>Datum Schema closed before “{name}” was saved. A copy from {saved} was kept.</p>
  </Dialog>
));
