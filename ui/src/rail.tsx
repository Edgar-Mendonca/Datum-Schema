// The left side (0.11.3, the user's mock-up from Datum Mesh): one rail of icons down the left edge,
// and beside it the one panel it opens (the symbols tree, the plant, the Inspector, the documents).
// Click an icon again to fold the panel away; drag the panel's edge to widen it. Nothing is on the
// right: the sheet has all the room.
import { For, Show, createEffect, createMemo, createResource, createSignal, on } from "solid-js";
import { List } from "./api";
import { side, setSide, Side, sel, tool, proj, sheet, setSheetId, activeDoc, setActiveDoc, docTabs, call, setPanel, setListOpen, setBottomOpen, setBackstage, listKey, listOpen } from "./store";
import { Palette, paneW, setPaneW, grip } from "./palette";
import { PlantPanel } from "./plant";
import { Inspector } from "./inspector";
import { Help } from "./help";
import { openDatasheet, dsId } from "./datasheet";
import { openListTab, tabTitle, LIST_TABS } from "./panels";

const Ic = (p: { n: string }) => <svg class="ic"><use href={`#i-${p.n}`} /></svg>;

const PANELS: [Exclude<Side, "">, string, string, string][] = [
  ["symbols", "symbols", "Symbols", "Every symbol, as a tree: drag one onto the sheet, or click it and then the sheet"],
  ["plant", "tag", "Plant", "The plant hierarchy (plant › unit › area) with every object, drawn or not"],
  ["inspector", "info", "Inspector", "The selected item's data; with nothing selected, the sheet's"],
  ["documents", "list", "Documents", "Sheets, lists and datasheets: where you are, and one click to any of them"],
];

/** Show the Inspector when something is picked on the sheet (and go back when it is let go), unless
 *  the user is placing symbols or has folded the panel away. */
let auto: Side | null = null;
let placedAt = 0;
export const followSelection = () => {
  // Placing symbols selects each new one: that is not a pick, so the Symbols panel stays.
  createEffect(on(tool, t => { if (t.k !== "select") placedAt = Date.now(); }));
  createEffect(on(sel, (s, prev) => {
  if (tool().k !== "select" || Date.now() - placedAt < 600) { placedAt = tool().k !== "select" ? Date.now() : placedAt; return; }
  // Only an open panel switches: a folded one stays folded (the user wanted the room), and the
  // drawing never shifts under the pointer.
  if (s.length && s.join() !== (prev ?? []).join() && (side() === "symbols" || side() === "documents")) { auto = side(); setSide("inspector"); }
  else if (!s.length && auto !== null && side() === "inspector") { setSide(auto); auto = null; }
}, { defer: true }));
};

export const Rail = () => {
  const pick = (k: Side) => { auto = null; setSide(side() === k ? "" : k); };
  return (
    <nav class="rail" aria-label="Panels">
      <For each={PANELS}>{([k, ic, l, t]) => <button classList={{ on: side() === k }} data-side={k} title={t} onClick={() => pick(k)}><Ic n={ic} /><span>{l}</span></button>}</For>
      <span class="rgrow" />
      <span class="rcap">Tools</span>
      <button class="rextra" title="The checks, the to-do list, comments, messages and the log (bottom pane)" onClick={() => { setPanel("checks"); setBottomOpen(true); }}><Ic n="check" /><span>Checks</span></button>
      <button class="rextra" title="Lists under the drawing, to review it against them" classList={{ on: listOpen() && !activeDoc() }} onClick={() => { setActiveDoc(null); setPanel(listKey()); setListOpen(!listOpen()); }}><Ic n="table" /><span>Lists</span></button>
      <button class="rextra" title="Symbol libraries, and abbreviations and codes" onClick={() => setBackstage("libraries")}><Ic n="legend" /><span>Libraries</span></button>
      <button class="rextra" title="Find anything (Ctrl+F)" onClick={() => import("./find").then(m => m.openFind())}><Ic n="search" /><span>Find</span></button>
    </nav>
  );
};

/** The documents tree: sheets, lists and datasheets, with the one open marked. */
const Documents = () => {
  const [q, setQ] = createSignal("");
  const [ds] = createResource(() => proj(), () => call<List>("list", { kind: "datasheets" }));
  const match = (s: string) => !q().trim() || s.toLowerCase().includes(q().trim().toLowerCase());
  const [open, setOpen] = createSignal<Record<string, boolean>>({ sheets: true, lists: true, ds: true });
  const B = (p: { k: string; t: string; n: number; children: any }) => <div class="tgrp" classList={{ open: open()[p.k] || !!q() }}>
    <button class="tsum" onClick={() => setOpen({ ...open(), [p.k]: !open()[p.k] })}><span class="tt">{p.t}</span><span class="pill">{p.n}</span></button>
    <Show when={open()[p.k] || !!q()}><div class="tlist">{p.children}</div></Show></div>;
  const lists = createMemo(() => LIST_TABS().filter(([, l]) => match(l)));
  const dsRows = createMemo(() => (ds()?.rows ?? []).filter(r => match(r.cells.join(" "))));
  const listOn = (k: string) => (activeDoc() ? docTabs().find(t => t.id === activeDoc())?.list === k : listOpen() && listKey() === k);
  const dsOn = (id: number) => activeDoc() === "datasheet" && dsId() === id;
  return (
    <div class="docs">
      <label class="tfilter"><svg class="ic"><use href="#i-search" /></svg><input type="search" placeholder="Find a sheet, list or tag" value={q()} onInput={e => setQ(e.currentTarget.value)} /></label>
      <div class="tbody">
        <B k="sheets" t="Sheets" n={proj()?.sheets.length ?? 0}><For each={(proj()?.sheets ?? []).filter(s => match(`${s.number} ${s.title}`))}>{s =>
          <button class="ti" classList={{ on: !activeDoc() && sheet()?.id === s.id }} onClick={() => { setActiveDoc(null); setSheetId(s.id); }}><svg class="ic"><use href="#i-sheet" /></svg><span>{s.number}</span><small>{s.title}</small></button>}</For></B>
        <B k="lists" t="Lists" n={lists().length}><For each={lists()}>{([k, l]) =>
          <button class="ti" classList={{ on: listOn(k) }} title="Click: under the drawing (or in its own tab, if it has one). Double-click: its own tab" onClick={() => { const t = docTabs().find(x => x.list === k); if (t) setActiveDoc(t.id); else { setActiveDoc(null); setPanel(k); setListOpen(true); } }} onDblClick={() => openListTab(k)}><svg class="ic"><use href="#i-table" /></svg><span>{l}</span></button>}</For></B>
        <B k="ds" t="Datasheets" n={dsRows().length}><For each={dsRows().slice(0, 2000)} fallback={<p class="muted small plhint">Instruments, control valves, relief devices and equipment have datasheets; they appear here as you draw them.</p>}>{r =>
          <button class="ti" classList={{ on: dsOn(r.ids[0]) }} onClick={() => openDatasheet(r.ids[0])}><svg class="ic"><use href="#i-report" /></svg><span>{r.cells[0]}</span><small>{r.cells[1]}</small></button>}</For></B>
      </div>
    </div>
  );
};

/** The panel beside the rail. */
export const SidePanel = () => (
  <Show when={side()}>
    <aside class="sidep" style={{ width: `${paneW()}px` }}>
      <header class="sideh"><b>{PANELS.find(p => p[0] === side())?.[2]}</b><Help k={side() === "inspector" ? "inspector" : side() === "plant" ? "plant" : side() === "documents" ? "workspace" : "palette"} /><span class="grow" /><button class="pfold" title="Fold the panel away (click its icon to bring it back)" onClick={() => setSide("")}>«</button></header>
      <div class="sideb">
        <Show when={side() === "symbols"}><Palette /></Show>
        <Show when={side() === "plant"}><PlantPanel /></Show>
        <Show when={side() === "inspector"}><Inspector /></Show>
        <Show when={side() === "documents"}><Documents /></Show>
      </div>
      <div class="vgrip pgrip" title="Drag to widen or narrow this panel" onPointerDown={e => grip(e, paneW, setPaneW, "ds.pane.w", 1, 220, 560)} />
    </aside>
  </Show>
);

export { tabTitle };
