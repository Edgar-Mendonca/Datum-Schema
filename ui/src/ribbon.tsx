// Title bar, quick access toolbar, Tell Me and the ribbon (Home · Draw · Symbols · Lines ·
// Instruments · Check · Lists · Sheets · View · Help, and the contextual Format tab). Copied from
// Datum Plan's ribbon by way of Datum Pipe.
import { For, Show, JSX, createSignal, createMemo, onMount } from "solid-js";
import { call, out, proj, tab, setTab, setBackstage, openMenu, openDialog, closeDialog, theme, setTheme, sel, tool, setTool, lib, derived, hidden, opts, setOpts, itemOf, lineOf, act, sheet } from "./store";
import { C, ALL_CMDS, exampleMenu } from "./cmds";
import * as D from "./dialogs";
import { Icon } from "./ui";
import { Help, helpCard, HELP } from "./help";
import { PRESETS } from "./palette";
import { Cat, Kind } from "./api";
import { importDatasheets } from "./datasheet";
import { openLoops } from "./loops";
import { openCause } from "./cause";

const GH = "https://github.com/Edgar-Mendonca/Datum-Schema";

export const B = (p: { i: string; l: string; run?: (e: MouseEvent) => void; big?: boolean; on?: boolean; off?: boolean; dd?: (e: MouseEvent) => void; title?: string }) => p.big && p.run && p.dd ? (
  <div class="rbw big splitbig" classList={{ on: p.on }}>
    <button class="rb top" title={p.title ?? p.l} disabled={p.off} onClick={e => p.run!(e)}><Icon n={p.i} /></button>
    <button class="rb bot" title={`${p.l} options`} onClick={e => p.dd!(e)}><span>{p.l}</span><b class="car">▾</b></button>
  </div>
) : (
  <div class="rbw" classList={{ big: p.big }}>
    <button class="rb" classList={{ big: p.big, on: p.on }} disabled={p.off} title={p.title ?? p.l} onClick={e => (p.run ?? p.dd)?.(e)}><Icon n={p.i} />{p.big ? <span>{p.l}{p.dd && !p.run ? " ▾" : ""}</span> : <span>{p.l}</span>}{!p.big && p.dd && !p.run && <b class="car">▾</b>}</button>
    {p.run && p.dd && <button class="rb split" onClick={e => p.dd!(e)}>▾</button>}
  </div>
);
export const G = (p: { l: string; h?: string; children: JSX.Element }) => <div class="grp"><div class="items">{p.children}</div><div class="cap">{p.l}{p.h && <Help k={p.h} />}</div></div>;
export const S = (p: { children: JSX.Element }) => <div class="stack">{p.children}</div>;

/** Tell Me / command search (Ctrl+K). */
export const palette = () => openDialog(() => {
  const [q, setQ] = createSignal(""), [k, setK] = createSignal(0);
  const list = createMemo(() => { const s = q().toLowerCase(); return ALL_CMDS.filter(c => c[0].toLowerCase().includes(s)).slice(0, 14); });
  const go = (i: number) => { const c = list()[i]; if (c) { closeDialog(); setTimeout(() => c[1]()); } };
  let input!: HTMLInputElement;
  onMount(() => input.focus());
  return (
    <div class="ov high" onMouseDown={e => e.target === e.currentTarget && closeDialog()}>
      <div class="pal">
        <input ref={input} placeholder="Tell me what you want to do" value={q()} onInput={e => { setQ(e.currentTarget.value); setK(0); }}
          onKeyDown={e => { if (e.key === "ArrowDown") { e.preventDefault(); setK(Math.min(list().length - 1, k() + 1)); } else if (e.key === "ArrowUp") { e.preventDefault(); setK(Math.max(0, k() - 1)); } else if (e.key === "Enter") { e.preventDefault(); go(k()); } else if (e.key === "Escape") closeDialog(); }} />
        <div class="pl"><For each={list()}>{(c, i) => <button classList={{ on: i() === k() }} onMouseEnter={() => setK(i())} onClick={() => go(i())}><span>{c[0]}</span>{c[2] && <kbd>{c[2]}</kbd>}</button>}</For></div>
      </div>
    </div>
  );
});

export const TABS: [string, string][] = [["home", "Home"], ["draw", "Draw"], ["symbols", "Symbols"], ["lines", "Lines"], ["instruments", "Instruments"], ["check", "Check"], ["lists", "Lists"], ["sheets", "Sheets"], ["view", "View"], ["help", "Help"]];

export const TitleBar = (p: { update: string | null }) => (
  <header class="top">
    <div class="bar">
      <div class="qat">
        <img src="logo.svg" alt="" class="logo" />
        <button title="Save (Ctrl+S)" onClick={() => C.save()}><Icon n="save" /></button>
        <button title="Undo (Ctrl+Z)" disabled={!out()?.undo} onClick={C.undo}><Icon n="undo" /></button>
        <button title="Redo (Ctrl+Y)" disabled={!out()?.redo} onClick={C.redo}><Icon n="redo" /></button>
        <button title="Print (Ctrl+P)" onClick={C.print}><Icon n="print" /></button>
      </div>
      <div class="title">{out() ? `${out()!.p.name}${out()!.dirty ? " •" : ""} — Datum Schema` : "Datum Schema"}</div>
      <button class="search" onClick={palette} title="Tell me what you want to do (Ctrl+K)"><Icon n="search" /><span>Tell me what you want to do</span><kbd>Ctrl+K</kbd></button>
      <Show when={p.update}><button class="upd" onClick={() => setBackstage("about")}>Update {p.update} available</button></Show>
      <button class="theme" title="Theme: Auto follows Windows" onClick={() => setTheme(theme() === "auto" ? "light" : theme() === "light" ? "dark" : "auto")}>Theme · {theme()}</button>
    </div>
    <nav class="tabs">
      <button class="file" onClick={() => setBackstage("home")}>File</button>
      <For each={TABS}>{([k, l]) => <button classList={{ on: tab() === k }} onClick={() => setTab(k)}>{l}</button>}</For>
      <div class="ctxgrp" style={{ visibility: sel().length ? "visible" : "hidden" }}><small>Selection</small><button classList={{ on: tab() === "format" }} onClick={() => setTab("format")}>Format</button></div>
    </nav>
  </header>
);

/** Shrinks the ribbon in steps instead of scrolling sideways (FAMILY §6). */
const fitRibbon = (el: HTMLElement) => {
  const fit = () => {
    const gs = [...el.querySelectorAll<HTMLElement>(":scope > .grp")];
    el.dataset.fit = "0"; gs.forEach(g => delete g.dataset.fit);
    const over = () => el.scrollWidth > el.clientWidth + 1;
    if (!over()) return;
    el.dataset.fit = "1";
    for (const lvl of ["2", "3"]) for (let i = gs.length - 1; i >= 0 && over(); i--) gs[i].dataset.fit = lvl;
  };
  new ResizeObserver(fit).observe(el);
  new MutationObserver(() => requestAnimationFrame(fit)).observe(el, { childList: true, subtree: false });
};

/** A menu of the symbols in one or more categories (click to place). */
const symMenu = (cats: Cat[]) => (e: MouseEvent) => openMenu(e, (lib()?.symbols ?? []).filter(s => cats.includes(s.cat) && s.key !== "unknown").map(s => ({ label: s.name, run: () => C.placeTool(s.key) })));
const placing = (key: string) => { const t = tool(); return t.k === "place" && t.sym === key; };
const Sym = (p: { k: string; i: string; l?: string; big?: boolean; tag?: string }) => <B i={p.i} l={p.l ?? lib()?.symbols.find(s => s.key === p.k)?.name ?? p.k} big={p.big} on={placing(p.k)} run={() => C.placeTool(p.k, p.tag)} title={`Place: click the sheet (or drag from the palette)`} />;
const LineB = (p: { k: Kind; i: string; big?: boolean; l?: string }) => <B i={p.i} l={p.l ?? lib()?.kinds.find(x => x[0] === p.k)?.[1] ?? p.k} big={p.big} on={tool().k === "line" && (tool() as any).kind === p.k} run={() => C.lineTool(p.k)} title="Drag from a port, a line or anywhere on the sheet" />;
const counts = () => { const i = derived()?.issues ?? []; return { e: i.filter(x => x.level === "error").length, w: i.filter(x => x.level === "warning").length, n: i.filter(x => x.level === "note").length }; };
const presetMenu = (e: MouseEvent) => openMenu(e, PRESETS.map(([l, k, name]) => ({ label: `${l}: ${name}`, run: () => C.placeTool(k, l) })));
const layer = (l: string, name: string) => <label class="chk"><input type="checkbox" checked={!hidden().includes(l)} onChange={() => C.toggleLayer(l)} />{name}</label>;
const numberMenu = (e: MouseEvent) => {
  const ns = [...new Set(Object.values(derived()?.numbers ?? {}).filter(n => n))].sort();
  openMenu(e, ns.length ? [{ label: "No highlight", run: () => C.highlight("") }, "-", ...ns.map(n => ({ label: n, run: () => C.highlight(n) }))] : [{ label: "No line has a number yet", off: true }]);
};

export const Ribbon = () => {
  const one = () => (sel().length === 1 ? sel()[0] : null);
  const selItem = () => (one() !== null ? itemOf(one()!) : undefined);
  const selLine = () => (one() !== null ? lineOf(one()!) : undefined);
  return (
    <section class="ribbon" ref={fitRibbon}>
      <Show when={tab() === "home"}>
        <G l="Clipboard" h="edit"><B big i="paste" l="Paste" run={C.paste} /><S><B i="copy" l="Copy" run={C.copy} off={!sel().length} /><B i="dup" l="Duplicate" run={C.duplicate} off={!sel().length} /><B i="del" l="Delete" run={C.del} off={!sel().length} /></S></G>
        <G l="Draw" h="draw"><B big i="select" l="Select" on={tool().k === "select"} run={() => setTool({ k: "select" })} /><LineB big k="major" i="line" l="Line" /><B big i="symbols" l="Add Symbol" run={() => C.quickInsert()} title="Type a symbol's name (double-click the sheet does the same)" /></G>
        <G l="Units" h="units"><S><B i="units" l="SI: bar, °C, m³/h" on={proj()?.units === "SI"} run={() => C.units("SI")} /><B i="units" l="US: psi, °F, gpm" on={proj()?.units === "US"} run={() => C.units("US")} /></S></G>
        <G l="Examples" h="examples"><B big i="open" l="Examples" dd={exampleMenu} /></G>
        <G l="Checks" h="checks"><B big i="check" l={`${counts().e} errors`} run={() => C.panel("checks")} title="Open the Checks panel" /><S><span class="rcount warn">{counts().w} warnings</span><span class="rcount">{counts().n} notes</span></S></G>
      </Show>
      <Show when={tab() === "draw"}>
        <G l="Tools" h="draw"><B big i="select" l="Select" on={tool().k === "select"} run={() => setTool({ k: "select" })} /><LineB big k="major" i="line" l="Process Line" /><B big i="symbols" l="Add Symbol" run={() => C.quickInsert()} /></G>
        <G l="Text" h="notes"><B big i="note" l="Note" on={tool().k === "note"} run={() => C.noteTool("note")} /><S><B i="leader" l="Leader Note" run={() => C.noteTool("leader")} /><B i="hold" l="Hold Cloud" run={() => C.noteTool("hold")} /></S></G>
        <G l="Arrange" h="edit"><S><B i="rotate" l="Rotate (R)" run={C.rotate} off={!sel().length} /><B i="flip" l="Mirror (M)" run={C.flip} off={!sel().length} /><B i="dup" l="Duplicate" run={C.duplicate} off={!sel().length} /></S></G>
        <G l="Tags" h="tags"><B big i="renum" l="Renumber" run={C.renumber} title="Number every tag again: per prefix, per sheet, left to right" /></G>
      </Show>
      <Show when={tab() === "symbols"}>
        <G l="Equipment" h="cat:vessel"><B big i="vessel" l="Vessels" dd={symMenu(["vessel"])} /><B big i="pump" l="Rotating" dd={symMenu(["rotating"])} /><B big i="hx" l="Heat Transfer" dd={symMenu(["heat"])} /></G>
        <G l="Common" h="palette"><S><Sym k="vvessel" i="vessel" /><Sym k="tank_cone" i="vessel" /><Sym k="pump_cent" i="pump" /></S><S><Sym k="comp_cent" i="pump" /><Sym k="hx_fixed" i="hx" /><Sym k="air_cooler" i="hx" /></S></G>
        <G l="Valves" h="cat:valve"><B big i="valve" l="Manual" dd={symMenu(["valve"])} /><B big i="cv" l="Control and Relief" dd={symMenu(["control"])} /><S><Sym k="gate" i="valve" /><Sym k="check" i="valve" /><Sym k="cv_pneu" i="cv" /></S></G>
        <G l="Fittings" h="cat:fitting"><B big i="fitting" l="Fittings" dd={symMenu(["fitting"])} /><S><Sym k="reducer_con" i="fitting" /><Sym k="strainer_y" i="fitting" /><Sym k="drain" i="fitting" /></S></G>
        <G l="Find" h="palette"><B big i="search" l="By Name" run={() => C.quickInsert()} /></G>
      </Show>
      <Show when={tab() === "lines"}>
        <G l="Process" h="lines"><LineB big k="major" i="line" /><S><LineB k="minor" i="line" /><LineB k="utility" i="line" /></S></G>
        <G l="Signals" h="signals"><S><LineB k="pneumatic" i="sig" /><LineB k="electric" i="sig" /></S><S><LineB k="data" i="sig" /><LineB k="capillary" i="sig" /></S></G>
        <G l="Flow" h="lines"><B big i="flow" l="Reverse Flow" off={!selLine()} run={() => selLine() && C.reverse(selLine()!.id)} /></G>
        <G l="Sheets" h="offpage"><Sym big k="offpage" i="offpage" l="Off-Page Connector" /><S><B i="link" l="Pair…" off={selItem()?.sym !== "offpage"} run={() => selItem() && C.pairDialog(selItem()!.id)} /><B i="go" l="Go to Partner" off={selItem()?.sym !== "offpage"} run={() => selItem() && C.goPartner(selItem()!.id)} /></S></G>
        <G l="Line Marks" h="cat:mark"><S><Sym k="specbreak" i="mark" /><Sym k="battery_limit" i="mark" /><Sym k="tiein" i="mark" /></S><S><Sym k="insulation" i="mark" /><Sym k="trace_steam" i="mark" /><Sym k="trace_elec" i="mark" /></S><B big i="mark" l="More" dd={symMenu(["mark"])} /></G>
      </Show>
      <Show when={tab() === "instruments"}>
        <G l="Bubbles" h="cat:instrument"><Sym big k="inst_field" i="inst" l="Field" /><S><Sym k="inst_panel" i="inst" l="Control Room" /><Sym k="inst_dcs" i="inst" l="Shared Display" /><Sym k="inst_plc" i="inst" l="PLC" /></S></G>
        <G l="Functions" h="isa"><B big i="inst" l="Common" dd={presetMenu} /><S><Sym k="inst_field" i="inst" l="PT" tag="PT" /><Sym k="inst_field" i="inst" l="FT" tag="FT" /><Sym k="inst_field" i="inst" l="LT" tag="LT" /></S><S><Sym k="inst_dcs" i="inst" l="LIC" tag="LIC" /><Sym k="inst_dcs" i="inst" l="FIC" tag="FIC" /><Sym k="inst_dcs" i="inst" l="PIC" tag="PIC" /></S></G>
        <G l="Primary Elements" h="cat:primary"><B big i="meter" l="Elements" dd={symMenu(["primary"])} /><S><Sym k="orifice" i="meter" /><Sym k="magmeter" i="meter" /><Sym k="thermowell" i="meter" /></S></G>
        <G l="Final Elements" h="cat:control"><Sym big k="cv_pneu" i="cv" l="Control Valve" /><S><Sym k="cv_motor" i="cv" l="Motor Valve" /><Sym k="cv_solenoid" i="cv" l="Solenoid Valve" /><Sym k="psv" i="cv" l="Safety Valve" /></S></G>
        <G l="Signals" h="signals"><S><LineB k="electric" i="sig" /><LineB k="pneumatic" i="sig" /></S><S><LineB k="data" i="sig" /><LineB k="capillary" i="sig" /></S></G>
      </Show>
      <Show when={tab() === "check"}>
        <G l="Checks" h="checks"><B big i="check" l="Checks Panel" run={() => C.panel("checks")} /><S><span class="rcount err">{counts().e} errors</span><span class="rcount warn">{counts().w} warnings</span><span class="rcount">{counts().n} notes</span></S></G>
        <G l="Rules" h="rules"><B big i="opts" l="Check Rules" run={() => setBackstage("options")} title="Switch rules on or off (Options › Checks)" /><B big i="gauge" l="Hydraulic Limits" run={() => setBackstage("options")} /></G>
        <G l="Tags" h="tags"><B big i="renum" l="Renumber" run={C.renumber} /></G>
      </Show>
      <Show when={tab() === "lists"}>
        <G l="Lists" h="lists"><B big i="list" l="Line List" run={() => C.panel("lines")} /><B big i="list" l="Equipment" run={() => C.panel("equipment")} /><B big i="list" l="Instruments" run={() => C.panel("instruments")} /><B big i="list" l="Valves" run={() => C.panel("valves")} /><B big i="list" l="Datasheets" run={() => C.panel("datasheets")} /><B big i="sig" l="Loop Diagrams" run={() => openLoops()} /><B big i="table" l="Cause & Effect" title="The cause-and-effect matrix of the interlocks" run={openCause} /><B big i="import" l="Import Excel" title="Import datasheet values from an Excel workbook, with a preview first" run={importDatasheets} /></G>
        <G l="Export" h="export"><B big i="xlsx" l="Excel" dd={e => openMenu(e, [...[["lines", "Line list"], ["equipment", "Equipment list"], ["instruments", "Instrument index"], ["valves", "Valve list"], ["datasheets", "Datasheet index"]].map(([k, l]) => ({ label: l, run: () => C.exportList(k, "xlsx") })), "-", { label: "Datasheets (one worksheet each)", run: C.exportDatasheets }, { label: "Cause-and-effect matrix", run: C.exportCause }])} /><B big i="csv" l="CSV" dd={e => openMenu(e, [["lines", "Line list"], ["equipment", "Equipment list"], ["instruments", "Instrument index"], ["valves", "Valve list"], ["datasheets", "Datasheet index"]].map(([k, l]) => ({ label: l, run: () => C.exportList(k, "csv") })))} /><B big i="print" l="Print" run={C.print} /></G>
      </Show>
      <Show when={tab() === "sheets"}>
        <G l="Sheets" h="sheets"><B big i="sheet" l="New Sheet" run={() => C.addSheet()} /><B big i="legend" l="Legend Sheet" run={() => C.addSheet(true)} title="A sheet that shows every symbol the project uses" /><S><B i="left" l="Move Left" run={() => C.moveSheet(-1)} /><B i="right" l="Move Right" run={() => C.moveSheet(1)} /><B i="del" l="Delete Sheet" run={C.deleteSheet} /></S></G>
        <G l="Size" h="sheets"><S><For each={["A3", "A1"]}>{z => <B i="sheet" l={z} on={sheet()?.size === z} run={() => sheet() && act({ SetSheet: { sheet: { ...sheet()!, size: z } } })} />}</For></S><S><For each={["ANSI B", "ANSI D"]}>{z => <B i="sheet" l={z} on={sheet()?.size === z} run={() => sheet() && act({ SetSheet: { sheet: { ...sheet()!, size: z } } })} />}</For></S></G>
        <G l="Output" h="export"><B big i="print" l="Print / PDF" run={C.print} /><B big i="dxf" l="DXF" run={C.exportDxf} /><B big i="svg" l="SVG" run={C.exportSvg} /><B big i="export" l="DEXPI" title="This sheet in the DEXPI P&ID exchange format (XML)" run={C.exportDexpi} /></G>
      </Show>
      <Show when={tab() === "view"}>
        <G l="Zoom" h="view"><B big i="fit" l="Fit Drawing" run={C.fit} /><B big i="sheet" l="Whole Sheet" run={() => import("./canvas").then(c => c.fitSheet())} /></G>
        <G l="Layers" h="layers"><S>{layer("process", "Process")}{layer("utility", "Utility")}</S><S>{layer("instrument", "Instruments")}{layer("notes", "Notes and marks")}</S></G>
        <G l="Select" h="select"><B big i="select" l="By Type" dd={e => openMenu(e, [...(["vessel", "rotating", "heat", "valve", "control", "fitting", "instrument", "primary", "mark", "connector"] as Cat[]).map(c => ({ label: lib()?.symbols.find(s => s.cat === c)?.cat_name ?? c, run: () => C.selectType(c) })), "-" as const, ...(lib()?.kinds ?? []).map(k => ({ label: `${k[1]} lines`, run: () => C.selectType(`line:${k[0]}`) }))])} /><B big i="line" l="By Line Number" dd={numberMenu} /></G>
        <G l="Show" h="view"><S><label class="chk"><input type="checkbox" checked={opts().grid} onChange={e => setOpts({ ...opts(), grid: e.currentTarget.checked })} />Grid</label></S></G>
      </Show>
      <Show when={tab() === "format"}>
        <Show when={sel().length} fallback={<G l="Format"><div class="rnote">Select a symbol or a line on the sheet.</div></G>}>
          <G l="Arrange" h="edit"><B big i="rotate" l="Rotate" run={C.rotate} off={!selItem() && sel().length < 2} /><B big i="flip" l="Mirror" run={C.flip} off={!selItem() && sel().length < 2} /><S><For each={[0.75, 1, 1.5]}>{x => <B i="scale" l={`${x}×`} off={!selItem()} on={selItem()?.scale === x} run={() => selItem() && act({ SetItem: { item: { ...selItem()!, scale: x } } })} />}</For></S></G>
          <Show when={selLine()}>{l => <>
            <G l="Line Type" h="lines"><S><For each={(lib()?.kinds ?? []).slice(0, 3)}>{k => <B i="line" l={k[1]} on={l().kind === k[0]} run={() => act({ SetLine: { line: { ...l(), kind: k[0] } } })} />}</For></S><S><For each={(lib()?.kinds ?? []).slice(3, 6)}>{k => <B i="sig" l={k[1]} on={l().kind === k[0]} run={() => act({ SetLine: { line: { ...l(), kind: k[0] } } })} />}</For></S></G>
            <G l="Flow" h="lines"><B big i="flow" l="Reverse Flow" run={() => C.reverse(l().id)} /><S><B i="line" l="Straighten" off={!l().pts.length} run={() => act({ SetPts: { id: l().id, pts: [] } })} /><B i="select" l="Whole Line Number" run={() => C.selectRun(l().id)} /></S></G>
          </>}</Show>
          <G l="Edit" h="edit"><S><B i="copy" l="Copy" run={C.copy} /><B i="dup" l="Duplicate" run={C.duplicate} /><B i="del" l="Delete" run={C.del} /></S></G>
        </Show>
      </Show>
      <Show when={tab() === "help"}>
        <G l="Learn"><B big i="report" l="Getting Started" run={() => D.gettingStarted()} /><B big i="info" l="Glossary" run={() => D.glossary()} /><B big i="help" l="What Is This Tool?" dd={e => openMenu(e, Object.entries(HELP).map(([k, h]) => ({ label: h.t, run: () => helpCard(k) })))} /><B big i="keys" l="Shortcuts" run={() => D.shortcuts()} /><B big i="search" l="Tell Me" run={palette} /></G>
        <G l="Datum Schema on GitHub"><B big i="github" l="GitHub" run={() => call("open_url", { url: GH })} /><B big i="tag" l="Releases" run={() => call("open_url", { url: `${GH}/releases` })} /><B big i="bug" l="Report an Issue" run={() => call("open_url", { url: `${GH}/issues/new` })} /></G>
        <G l="Updates"><B big i="upd" l="Check for Updates" run={() => setBackstage("about")} /><B big i="list" l="Roadmap" run={() => setBackstage("roadmap")} /></G>
      </Show>
    </section>
  );
};
