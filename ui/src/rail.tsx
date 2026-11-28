// The left side (0.11.3, the user's mock-up from Datum Mesh): one rail of icons down the left edge,
// and beside it the one panel it opens (the symbols tree, the plant, the Inspector, the documents).
// Click an icon again to fold the panel away; drag the panel's edge to widen it. Nothing is on the
// right: the sheet has all the room.
import { For, Show, createEffect, createMemo, createResource, createSignal, on } from "solid-js";
import { List } from "./api";
import { takeQuiet, followPick, side, setSide, Side, sel, tool, proj, sheet, setSheetId, activeDoc, setActiveDoc, call, setPanel, setListOpen, setBottomOpen, bottomOpen, bottomKey, listKey, listOpen, listsInTab, LISTS_TAB, openListsTab, openDoc } from "./store";
import { LibrariesPage } from "./libraries";
import { Palette, paneW, setPaneW, grip } from "./palette";
import { PlantPanel } from "./plant";
import { Inspector } from "./inspector";
import { Help } from "./help";
import { openDatasheet, dsId } from "./datasheet";
import { tabTitle, LIST_TABS } from "./panels";

const Ic = (p: { n: string }) => <svg class="ic"><use href={`#i-${p.n}`} /></svg>;

const PANELS: [Exclude<Side, "">, string, string, string][] = [
  ["symbols", "symbols", "Symbols", "Every symbol, as a tree: drag one onto the sheet, or click it and then the sheet"],
  ["plant", "tag", "Plant", "The plant hierarchy (plant › unit › area) with every object, drawn or not"],
  ["inspector", "info", "Inspector", "The selected item's data; with nothing selected, the sheet's"],
  ["documents", "list", "Documents", "Sheets, lists and datasheets: where you are, and one click to any of them"],
];

/** Show the Inspector when the user picks something (and go back when it is let go), unless the user
 *  switched that off (View) or folded the panel away. A selection the app makes (a symbol placed,
 *  a paste) is marked quiet where it is made (0.11.5), so placing keeps the Symbols panel. */
let auto: Side | null = null;
export const followSelection = () => {
  createEffect(on(sel, (s, prev) => {
  if (takeQuiet() || tool().k !== "select" || !followPick()) return;
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
      {/* The tools (0.11.5) each open their place, and a second click closes it again. */}
      <button class="rextra" data-tool="checks" title="The checks, in the drawer under the drawing (click again to fold it)" classList={{ on: bottomOpen() && bottomKey() === "checks" }} onClick={() => { if (bottomOpen() && bottomKey() === "checks") setBottomOpen(false); else { setPanel("checks"); setBottomOpen(true); } }}><Ic n="check" /><span>Checks</span></button>
      <button class="rextra" data-tool="lists" title="The lists: under the drawing, or in their own tab if you moved them there (click again to put them away)" classList={{ on: listsInTab() ? activeDoc() === LISTS_TAB : listOpen() }} onClick={() => { if (listsInTab()) setActiveDoc(activeDoc() === LISTS_TAB ? null : LISTS_TAB); else if (listOpen()) setListOpen(false); else { setActiveDoc(null); setPanel(listKey()); setListOpen(true); } }}><Ic n="table" /><span>Lists</span></button>
      <button class="rextra" data-tool="libraries" title="Symbol libraries, and abbreviations and codes, in a tab (click again to go back to the drawing)" classList={{ on: activeDoc() === "libraries" }} onClick={() => (activeDoc() === "libraries" ? setActiveDoc(null) : openLibraries())}><Ic n="legend" /><span>Libraries</span></button>
      <button class="rextra" title="Find anything (Ctrl+F)" onClick={() => import("./find").then(m => m.openFind())}><Ic n="search" /><span>Find</span></button>
    </nav>
  );
};

/** The libraries page, as a tab over the sheet (0.11.5). */
export const openLibraries = () => openDoc({ id: "libraries", title: "Libraries", render: () => <div class="docpage"><LibrariesPage /></div> });

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
  const listOn = (k: string) => listKey() === k && (listsInTab() ? activeDoc() === LISTS_TAB : listOpen() && !activeDoc());
  const dsOn = (id: number) => activeDoc() === "datasheet" && dsId() === id;
  return (
    <div class="docs">
      <label class="tfilter"><svg class="ic"><use href="#i-search" /></svg><input type="search" placeholder="Find a sheet, list or tag" value={q()} onInput={e => setQ(e.currentTarget.value)} /></label>
      <div class="tbody">
        <B k="sheets" t="Sheets" n={proj()?.sheets.length ?? 0}><For each={(proj()?.sheets ?? []).filter(s => match(`${s.number} ${s.title}`))}>{s =>
          <button class="ti" classList={{ on: !activeDoc() && sheet()?.id === s.id }} onClick={() => { setActiveDoc(null); setSheetId(s.id); }}><svg class="ic"><use href="#i-sheet" /></svg><span>{s.number}</span><small>{s.title}</small></button>}</For></B>
        <B k="lists" t="Lists" n={lists().length}><For each={lists()}>{([k, l]) =>
          <button class="ti" classList={{ on: listOn(k) }} title="Click: open it (under the drawing, or in the Lists tab if the lists are there). Double-click: the Lists tab" onClick={() => { setPanel(k); setListOpen(true); }} onDblClick={() => openListsTab(k)}><svg class="ic"><use href="#i-table" /></svg><span>{l}</span></button>}</For></B>
        <B k="ds" t="Datasheets" n={dsRows().length}><For each={dsRows().slice(0, 2000)} fallback={<p class="muted small plhint">Instruments, control valves, relief devices and equipment have datasheets; they appear here as you draw them.</p>}>{r =>
          <button class="ti" classList={{ on: dsOn(r.ids[0]) }} onClick={() => openDatasheet(r.ids[0])}><svg class="ic"><use href="#i-report" /></svg><span>{r.cells[0]}</span><small>{r.cells[1]}</small></button>}</For></B>
      </div>
    </div>
  );
};

/** A panel kept as it was left (its search, scroll and open branches) while another one is shown:
 *  built the first time it is opened, then only hidden (0.11.4). */
const seen = new Set<string>();
const Keep = (p: { k: Side; children: any }) => {
  const on = () => side() === p.k;
  return <Show when={on() || seen.has(p.k)}>{(() => { seen.add(p.k); return <div class="keep" style={{ display: on() ? "flex" : "none" }}>{p.children}</div>; })()}</Show>;
};

/** The panel beside the rail. */
export const SidePanel = () => (
  <Show when={side() || seen.size}>
    <aside class="sidep" style={{ width: `${paneW()}px`, display: side() ? undefined : "none" }}>
      <header class="sideh"><b>{PANELS.find(p => p[0] === side())?.[2]}</b><Help k={side() === "inspector" ? "inspector" : side() === "plant" ? "plant" : side() === "documents" ? "workspace" : "palette"} /><span class="grow" /><button class="pfold" title="Fold the panel away (click its icon to bring it back)" onClick={() => setSide("")}>«</button></header>
      <div class="sideb">
        <Keep k="symbols"><Palette /></Keep>
        <Keep k="plant"><PlantPanel /></Keep>
        <Show when={side() === "inspector"}><Inspector /></Show>
        <Keep k="documents"><Documents /></Keep>
      </div>
      <div class="vgrip pgrip" title="Drag to widen or narrow this panel" onPointerDown={e => grip(e, paneW, setPaneW, "ds.pane.w", 1, 220, 560)} />
    </aside>
  </Show>
);

export { tabTitle };
