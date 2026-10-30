// Datum Schema window: layout (palette, sheet, inspector, bottom panel), the sheet tab bar, the
// status bar, keyboard shortcuts, start-up and recovery.
import { render } from "solid-js/web";
import { For, Show, onMount } from "solid-js";
import "@fontsource/ibm-plex-sans/latin-400.css";
import "@fontsource/ibm-plex-sans/latin-500.css";
import "@fontsource/ibm-plex-sans/latin-600.css";
import "@fontsource/ibm-plex-sans/latin-700.css";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "@fontsource/ibm-plex-mono/latin-600.css";
import "./styles.css";
import { invoke, listen, Out, Options, Shown } from "./api";
import { out, setOut, backstage, setBackstage, setOpts, opts, dialogs, toast, loadOut, setThemeRaw, applyTheme, sel, proj, sheet, setSheetId, refreshLibs, derived, tool, setTool, openMenu, hidden } from "./store";
import { C, setFit, setCentre, setCopy, setViewCentre } from "./cmds";
import { TitleBar, Ribbon, palette } from "./ribbon";
import { Backstage, update, checkUpdate } from "./backstage";
import { Canvas, fit, fitSheet, goTo, copySelection, viewCenter } from "./canvas";
import { Palette } from "./palette";
import { Inspector } from "./inspector";
import { Bottom } from "./panels";
import { Menu, Toast } from "./ui";
import { setUnitTable } from "./units";
import * as D from "./dialogs";

setFit(fit);
setCentre(goTo);
setCopy(copySelection);
setViewCentre(viewCenter);

/** The sheet tab bar: one tab per sheet; right-click for sheet commands. */
const SheetTabs = () => (
  <div class="stabs">
    <For each={proj()?.sheets ?? []}>{s => <button classList={{ on: sheet()?.id === s.id, legend: s.legend }} onClick={() => setSheetId(s.id)} title={s.title}
      onContextMenu={e => { setSheetId(s.id); openMenu(e, [{ label: "New sheet", run: () => C.addSheet() }, { label: "Legend sheet", run: () => C.addSheet(true) }, "-", { label: "Move left", run: () => C.moveSheet(-1) }, { label: "Move right", run: () => C.moveSheet(1) }, "-", { label: "Delete this sheet", icon: "del", run: C.deleteSheet }]); }}>
      {s.number}</button>}</For>
    <button class="sadd" title="New sheet" onClick={() => C.addSheet()}>+</button>
  </div>
);

const Status = () => {
  const counts = () => { const i = derived()?.issues ?? []; return [i.filter(x => x.level === "error").length, i.filter(x => x.level === "warning").length]; };
  return (
    <footer class="status">
      <span>{tool().k === "select" ? "Ready" : "Drawing"}</span>
      <button class="sbtn" classList={{ bad: counts()[0] > 0, warn: counts()[0] === 0 && counts()[1] > 0 }} onClick={() => C.panel("checks")} title="Open the Checks panel">{counts()[0] === 0 && counts()[1] === 0 ? "✓ " : ""}{counts()[0]} errors · {counts()[1]} warnings</button>
      <span class="muted">{proj() ? `${proj()!.sheets.length} sheets · ${proj()!.items.length} symbols · ${proj()!.lines.length} lines` : ""}</span>
      <Show when={sel().length}><span class="muted">{sel().length} selected</span></Show>
      <Show when={hidden().length}><span class="muted">Hidden: {hidden().join(", ")}</span></Show>
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
  const keys = (e: KeyboardEvent) => {
    const ctrl = e.ctrlKey || e.metaKey, k = e.key.toLowerCase();
    if (dialogs().length) return;
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
    if ((ctrl && k === "k") || (e.altKey && k === "q")) { e.preventDefault(); palette(); }
    else if (ctrl && k === "s") { e.preventDefault(); C.save(e.shiftKey); }
    else if (ctrl && k === "o") { e.preventDefault(); C.open(); }
    else if (ctrl && k === "n") { e.preventDefault(); setBackstage("home"); }
    else if (ctrl && k === "p") { e.preventDefault(); C.print(); }
    else if (ctrl && k === "z" && !typing) { e.preventDefault(); C.undo(); }
    else if (ctrl && k === "y" && !typing) { e.preventDefault(); C.redo(); }
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
        <Palette />
        <div class="content">
          <Show when={out()}>
            <div class="sheetarea"><Canvas /><SheetTabs /></div>
            <Bottom />
          </Show>
        </div>
        <Inspector />
      </main>
      <Status />
      <Show when={backstage()}><Backstage /></Show>
      <For each={dialogs()}>{d => d()}</For>
      <Menu /><Toast />
    </div>
  );
};

render(() => <App />, document.getElementById("app")!);
