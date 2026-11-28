// Datum Schema window: layout (palette, sheet, inspector, bottom panel), the sheet tab bar, the
// status bar, keyboard shortcuts, start-up and recovery.
import { render } from "solid-js/web";
import { For, Show, onMount, createEffect, on, untrack } from "solid-js";
import "@fontsource/ibm-plex-sans/latin-400.css";
import "@fontsource/ibm-plex-sans/latin-500.css";
import "@fontsource/ibm-plex-sans/latin-600.css";
import "@fontsource/ibm-plex-sans/latin-700.css";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "@fontsource/ibm-plex-mono/latin-600.css";
import "./styles.css";
import { invoke, listen, Out, Options, Shown } from "./api";
import { symOf, itemOf, lineOf, LISTS_TAB, listsTabKept, openListsTab, listKey, loads, setBottomOpen, toggleRibbon, ribbonPeek, setRibbonPeek, out, setOut, backstage, setBackstage, setOpts, opts, dialogs, toast, loadOut, setThemeRaw, applyTheme, sel, proj, sheet, setSheetId, refreshLibs, derived, tool, setTool, openMenu, hidden, docTabs, activeDoc, setActiveDoc, closeDoc, setPanel } from "./store";
import { C, setFit, setCentre, setCopy, setViewCentre } from "./cmds";
import { TitleBar, Ribbon, palette } from "./ribbon";
import { Backstage, update, checkUpdate } from "./backstage";
import { Canvas, fit, fitSheet, goTo, copySelection, viewCenter } from "./canvas";
import { Rail, SidePanel, followSelection } from "./rail";
import { Drawer, PanelBody, ListsDoc } from "./panels";
import { Menu, Toast, DlgFn, InTab } from "./ui";
import { QuickBar } from "./selbar";
import { setUnitTable } from "./units";
import * as D from "./dialogs";

setFit(fit);
setCentre(goTo);
setCopy(copySelection);
setViewCentre(viewCenter);

/** The document tabs, over the sheet (0.11.3): one tab per sheet (right-click for sheet commands),
 *  then the lists and the datasheet moved to tabs of their own. */
const SheetTabs = () => (
  <div class="stabs doctabs">
    <For each={proj()?.sheets ?? []}>{s => <button classList={{ on: !activeDoc() && sheet()?.id === s.id, legend: s.legend }} onClick={() => { setActiveDoc(null); setSheetId(s.id); }} title={s.title}
      onContextMenu={e => { setSheetId(s.id); openMenu(e, [{ label: "New sheet", run: () => C.addSheet() }, { label: "Legend sheet", run: () => C.addSheet(true) }, "-", { label: "Move left", run: () => C.moveSheet(-1) }, { label: "Move right", run: () => C.moveSheet(1) }, "-", { label: "Delete this sheet", icon: "del", run: C.deleteSheet }]); }}>
      {s.number}</button>}</For>
    <button class="sadd" title="New sheet" onClick={() => C.addSheet()}>+</button>
    <For each={docTabs()}>{t => <span class="dtab" classList={{ on: activeDoc() === t.id }}><button onClick={() => setActiveDoc(t.id)} title={t.list ? "A list, full size (it stays in step with the drawing)" : t.title}>{t.title}</button><button class="dx" title="Close this tab" onClick={() => closeDoc(t.id)}>×</button></span>}</For>
  </div>
);

/** The status line's hint (0.11.5): the tool, the next step, and its keys. */
const hint = () => {
  const t = tool();
  if (t.k === "place") { const o = t.obj !== undefined; return `Placing ${symOf(t.sym)?.name ?? "a symbol"}: click the sheet${o ? "" : " (Shift+click keeps placing)"} · Esc stops`; }
  if (t.k === "line") return "Drawing a line: drag from a port to a port, a line or an empty spot · Esc stops";
  if (t.k === "note") return "Click where the note goes · Esc stops";
  if (t.k === "comment") return "Click where the comment goes · Esc stops";
  if (t.k === "assembly") return `Placing ${t.name}: click the sheet · Esc stops`;
  if (t.k === "nozzle") return "Click the equipment's outline where the nozzle goes · Esc stops";
  const s = sel(); if (!s.length) return "Ready";
  if (s.length === 1 && itemOf(s[0])) return "R rotate · M mirror · Ctrl+D duplicate · Del delete";
  if (s.length === 1 && lineOf(s[0])) return "Drag a bend to move it · Del delete";
  return "Drag to move · R rotate · Del delete";
};

const Status = () => {
  const counts = () => { const i = derived()?.issues ?? []; return [i.filter(x => x.level === "error").length, i.filter(x => x.level === "warning").length]; };
  return (
    <footer class="status">
      <span class="shint" title="What the pointer does now, and the keys for it">{hint()}</span>
      <button class="sbtn" classList={{ bad: counts()[0] > 0, warn: counts()[0] === 0 && counts()[1] > 0 }} onClick={() => C.panel("checks")} title="Open the Checks panel">{counts()[0] === 0 && counts()[1] === 0 ? "✓ " : ""}{counts()[0]} errors · {counts()[1]} warnings</button>
      <span class="muted">{proj() ? `${proj()!.sheets.length} sheets · ${proj()!.items.length} symbols · ${proj()!.lines.length} lines` : ""}</span>
      <Show when={sel().length}><span class="muted">{sel().length} selected</span></Show>
      <Show when={hidden().length}><span class="muted">Hidden: {hidden().join(", ")}</span></Show>
      <Show when={(proj()?.comments ?? []).some(c => !c.resolved)}><button class="sbtn" onClick={() => C.panel("comments")} title="Open the Comments panel">{(proj()?.comments ?? []).filter(c => !c.resolved).length} open comments</button></Show>
      <span class="grow" />
      <span class="muted">{proj()?.units === "US" ? "US units" : "SI units"}</span>
    </footer>
  );
};

const App = () => {
  onMount(async () => {
    try {
      const o = await invoke<Options>("options"); setOpts(o); setThemeRaw(o.theme || "auto"); applyTheme();
      setUnitTable(await invoke<[Shown[], Shown[]]>("unit_table"));
      await refreshLibs();
      setOut(await invoke<Out>("state"));
      const start = await invoke<string | null>("startup_file");
      if (start) await loadOut("open", { path: start });
      const rec = await invoke<{ name: string; saved: string } | null>("recovery");
      if (rec) D.recoverDialog(rec.name, rec.saved, () => setBackstage(null));
      checkUpdate(true);
      setInterval(() => { if (opts().autosave > 0) invoke("autosave").catch(() => {}); }, Math.max(1, opts().autosave) * 60000);
      listen("close-requested", () => D.confirmClose(() => invoke("quit")));
    } catch (e) { toast(String(e)); }
  });
  followSelection();
  // A project that opens with errors opens the Checks pane; notes and warnings alone leave it folded.
  let fresh = false;
  createEffect(on(loads, () => { fresh = true; if (listsTabKept() && !docTabs().some(t => t.id === LISTS_TAB)) { openListsTab(listKey(), false); setActiveDoc(null); } }, { defer: true }));
  createEffect(on(derived, d => { if (!fresh || !d) return; fresh = false; if (d.issues.some(x => x.level === "error")) { setPanel("checks"); setBottomOpen(true); } }));
  // Back on a sheet from a tab: the drawing measures its room again.
  createEffect(on(activeDoc, () => queueMicrotask(() => window.dispatchEvent(new Event("resize"))), { defer: true }));
  const keys = (e: KeyboardEvent) => {
    const ctrl = e.ctrlKey || e.metaKey, k = e.key.toLowerCase();
    if (e.key === "Escape" && ribbonPeek()) { setRibbonPeek(false); return; }
    if (dialogs().length) return;
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
    if ((ctrl && k === "k") || (e.altKey && k === "q")) { e.preventDefault(); palette(); }
    else if (ctrl && k === "s") { e.preventDefault(); C.save(e.shiftKey); }
    else if (ctrl && k === "o") { e.preventDefault(); C.open(); }
    else if (ctrl && k === "n") { e.preventDefault(); setBackstage("home"); }
    else if (ctrl && k === "p") { e.preventDefault(); C.print(); }
    else if (ctrl && k === "f") { e.preventDefault(); import("./find").then(m => m.openFind()); }
    else if (ctrl && k === "z" && !typing) { e.preventDefault(); C.undo(); }
    else if (ctrl && k === "y" && !typing) { e.preventDefault(); C.redo(); }
    else if (e.key === "F1" && e.ctrlKey) { e.preventDefault(); toggleRibbon(); }
    else if (e.key === "F1") { e.preventDefault(); D.shortcuts(); }
    else if (!typing && !ctrl && !backstage() && k === "f") (e.shiftKey ? fitSheet() : C.fit());
    else if (e.key === "Escape" && backstage() && out()) setBackstage(null);
    else if (e.key === "Escape" && tool().k !== "select") setTool({ k: "select" });
  };
  return (
    <div class="app" ref={() => document.addEventListener("keydown", keys)}>
      <TitleBar update={update()?.available ? update()!.latest : null} />
      <Ribbon />
      <main class="work">
        <Rail />
        <SidePanel />
        <div class="content">
          <Show when={out()}>
            <SheetTabs />
            <div class="sheetarea"><div class="canvhold" style={{ display: activeDoc() ? "none" : "contents" }}><Canvas /></div>
              <For each={docTabs()}>{t => <div class="docview" style={{ display: activeDoc() === t.id ? "flex" : "none" }}><InTab.Provider value={t.id}>{t.id === LISTS_TAB ? <ListsDoc /> : t.list ? <div class="lists docl"><PanelBody k={t.list} /></div> : untrack(() => t.render?.())}</InTab.Provider></div>}</For>
            </div>
            <Drawer />
          </Show>
        </div>
      </main>
      <Status />
      <Show when={backstage()}><Backstage /></Show>
      <For each={dialogs()}>{d => <DlgFn.Provider value={d}>{untrack(d)}</DlgFn.Provider>}</For>
      <QuickBar />
      <Menu /><Toast />
    </div>
  );
};

render(() => <App />, document.getElementById("app")!);
