// Window state: the open project, the open sheet, what the engine works out from it (the sheet's
// drawing, the checks, the hydraulics), the selection, the drawing tool, menus, dialogs and messages.
import type { ColourMode } from "./colours";
import { createSignal, createMemo, batch, JSX } from "solid-js";
import { invoke, Out, Options, Project, Derived, Library, SymInfo, Kind, NoteKind, Item, Line, Obj, LibView, SymLibrary } from "./api";

export const [out, setOutRaw] = createSignal<Out | null>(null);
export const proj = (): Project | undefined => out()?.p;
export const [sheetId, setSheetIdRaw] = createSignal(0);
export const [derived, setDerived] = createSignal<Derived | null>(null);
export const [lib, setLib] = createSignal<Library | null>(null);
/** The symbol libraries on this computer. */
export const [libs, setLibs] = createSignal<LibView[]>([]);
/** Reads the libraries again (and the symbols, which include theirs). */
export const refreshLibs = async () => {
  try {
    setLibs(await invoke<LibView[]>("libraries"));
    setLib(await invoke<Library>("library"));
  } catch (e) { toast(String(e)); }
};
/** Every library the user can use here: those on this computer and those kept in this project,
 *  one entry for a library that is both (by its id), with where it lives and whether it is ticked. */
export type LibEntry = { library: SymLibrary; file: string; onComputer: boolean; inProject: boolean; shown: boolean; svgs: string[] };
export const allLibs = (): LibEntry[] => {
  const st = out()?.p.settings, off = new Set(st?.libs_off ?? []);
  const svgOf = (k: string) => lib()?.symbols.find(s => s.key === k)?.svg ?? "";
  const idOf = (l: SymLibrary) => l.id || l.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  const v: LibEntry[] = libs().map(x => ({ library: x.library, file: x.file, onComputer: true, inProject: false, shown: !off.has(idOf(x.library)), svgs: x.svgs }));
  for (const l of st?.libraries ?? []) {
    const e = v.find(x => idOf(x.library) === idOf(l));
    if (e) { e.inProject = true; e.library = l; }
    else v.push({ library: l, file: "", onComputer: false, inProject: true, shown: !off.has(idOf(l)), svgs: l.symbols.map(s => svgOf(s.key)) });
  }
  return v.sort((a, b) => a.library.name.localeCompare(b.library.name));
};
/** Old drawings behind sheets, as SVG, by fingerprint: fetched once each. */
export const [bgs, setBgs] = createSignal<Record<string, string>>({});
const bgWant = new Set<string>();
export const loadBg = async (sheet: number, rev: string) => {
  if (bgWant.has(rev)) return;
  bgWant.add(rev);
  try { const s = await invoke<string>("underlay_svg", { sheet }); setBgs({ ...bgs(), [rev]: s }); } catch { bgWant.delete(rev); }
};
const symMap = createMemo(() => new Map((lib()?.symbols ?? []).map(s => [s.key, s])));
export const symOf = (k: string): SymInfo | undefined => symMap().get(k);
export const sheet = () => proj()?.sheets.find(s => s.id === sheetId()) ?? proj()?.sheets[0];
export const itemOf = (id: number): Item | undefined => proj()?.items.find(i => i.id === id);
export const lineOf = (id: number): Line | undefined => proj()?.lines.find(l => l.id === id);
export const objOf = (id: number): Obj | undefined => proj()?.objects.find(o => o.id === id);

let seq = 0;
/** After a project is opened (or a sheet changes), the next drawing from the engine is fitted. */
let wantFit = false;
let fitHook = () => {};
export const onFit = (f: () => void) => { fitHook = f; };
/** Asks the engine for the open sheet's drawing, the checks and the hydraulics. */
export const refresh = async () => {
  const n = ++seq, s = sheet()?.id ?? 0;
  try { const d = await invoke<Derived>("derived", { sheet: s }); if (n === seq) { setDerived(d); if (wantFit) { wantFit = false; requestAnimationFrame(() => fitHook()); } } }
  catch (e) { toast(String(e)); }
};
export const setSheetId = (id: number) => {
  wantFit = id !== sheetId();
  // A selected line's number stays highlighted on the other sheets.
  const one = sel().length === 1 ? derived()?.numbers[sel()[0]] : "";
  batch(() => { if (one) setHiNumber(one); setSheetIdRaw(id); setSel([]); });
  refresh();
};

export const setOut = (o: Out | null | undefined) => {
  if (!o) return;
  // A symbol the window does not know yet (a nozzle just added, a project's own): read them again.
  if (lib() && (o.p.items.some(i => !symMap().has(i.sym)) || (o.p.settings.libraries ?? []).some(l => l.symbols.some(s => !symMap().has(s.key))))) void refreshLibs();
  batch(() => {
    setOutRaw(o);
    if (!o.p.sheets.some(s => s.id === sheetId())) setSheetIdRaw(o.p.sheets[0]?.id ?? 0);
    // Drop selected things that no longer exist (deleted or undone away).
    const ids = new Set([...o.p.items, ...o.p.lines, ...o.p.notes].map(x => x.id));
    const s = sel().filter(id => ids.has(id));
    if (s.length !== sel().length) setSel(s);
  });
  refresh();
  if (o.warnings?.length) toast(`Opened with repairs: ${o.warnings.join(" ")}`);
};

/** auto, light or dark; auto follows Windows. */
export const [theme, setThemeRaw] = createSignal("auto");
const darkMq = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;
export const applyTheme = () => { document.documentElement.dataset.theme = theme() === "auto" ? (darkMq?.matches ? "dark" : "light") : theme(); };
darkMq?.addEventListener("change", () => theme() === "auto" && applyTheme());
export const setTheme = (t: string) => { setThemeRaw(t); applyTheme(); const o = { ...opts(), theme: t }; setOpts(o); invoke("set_options", { opts: o }).catch(() => {}); };
export const [opts, setOpts] = createSignal<Options>({ autosave: 5, beta: false, theme: "auto", openLast: false, units: "SI", grid: true, paper: "sheet", company: "", user: "", defaults: null });

export const [backstage, setBackstage] = createSignal<string | null>("home");
export const [tab, setTab] = createSignal("home");

// ---------- selection and tools ----------
/** Selected items, lines and notes (ids are unique across all three). */
export const [sel, setSel] = createSignal<number[]>([]);
export type Tool = { k: "select" } | { k: "line"; kind: Kind } | { k: "place"; sym: string; tag?: string; obj?: number } | { k: "note"; kind: NoteKind } | { k: "comment" } | { k: "assembly"; name: string } | { k: "nozzle"; item: number };
export const [tool, setTool] = createSignal<Tool>({ k: "select" });
/** Problems (errors and warnings) marked on the sheet, with their rule number. */
export const [showIssues, setShowIssues] = createSignal(false);
/** The line number highlighted on every sheet (Select › By line number). */
export const [hiNumber, setHiNumber] = createSignal("");
/** The bottom panel's tab, and whether it is open. */
// Two places show tables (0.11.3): the lists under the drawing (in the drawing's own tab, linked
// both ways with it, for the review) and the slim bottom pane (checks, to-do, comments, messages,
// the log). `setPanel` sends a key to the right one and opens it.
export const BOTTOM_KEYS = ["checks", "todo", "comments", "messages", "log"];
const prefKeep = (k: string, d: string) => { try { return localStorage.getItem(k) ?? d; } catch { return d; } };
const prefSave = (k: string, v: string) => { try { localStorage.setItem(k, v); } catch { /* private window */ } };
export const [listKey, setListKey] = createSignal(prefKeep("ds.lists.key", "lines"));
export const [listOpen, setListOpenRaw] = createSignal(prefKeep("ds.lists.open", "0") === "1");
export const setListOpen = (v: boolean) => { setListOpenRaw(v); prefSave("ds.lists.open", v ? "1" : "0"); queueMicrotask(() => window.dispatchEvent(new Event("resize"))); };
export const [bottomKey, setBottomKey] = createSignal("checks");
export const [bottomOpen, setBottomOpenRaw] = createSignal(true);
export const setBottomOpen = (v: boolean) => { setBottomOpenRaw(v); queueMicrotask(() => window.dispatchEvent(new Event("resize"))); };
let lastArea: "lists" | "bottom" = "bottom";
/** The key last shown (for help), and the one place to send a list or a panel to. */
export const panel = () => (lastArea === "bottom" ? bottomKey() : listKey());
export const setPanel = (k: string) => {
  if (BOTTOM_KEYS.includes(k)) { lastArea = "bottom"; setBottomKey(k); return; }
  lastArea = "lists"; setListKey(k); prefSave("ds.lists.key", k);
  // A list moved to its own tab opens there.
  const t = docTabs().find(x => x.list === k);
  if (t) { setActiveDoc(t.id); return; }
  setActiveDoc(null);
};
export const panelOpen = () => (lastArea === "bottom" ? bottomOpen() : listOpen());
export const setPanelOpen = (v: boolean) => (lastArea === "bottom" ? setBottomOpen(v) : docTabs().some(x => x.list === listKey()) ? undefined : setListOpen(v));
/** How the drawing is coloured on the screen and in print (0.11.3), kept on this computer. */
export const [screenMode, setScreenModeRaw] = createSignal<ColourMode>((["colour", "grey", "bw"].includes(prefKeep("ds.colour.screen", "colour")) ? prefKeep("ds.colour.screen", "colour") : "colour") as ColourMode);
export const setScreenMode = (m: ColourMode) => { setScreenModeRaw(m); prefSave("ds.colour.screen", m); };
export const [printMode, setPrintModeRaw] = createSignal<ColourMode>((["colour", "grey", "bw"].includes(prefKeep("ds.colour.print", "bw")) ? prefKeep("ds.colour.print", "bw") : "bw") as ColourMode);
export const setPrintMode = (m: ColourMode) => { setPrintModeRaw(m); prefSave("ds.colour.print", m); };
/** What the app said, newest last (the Messages panel). */
export const [messages, setMessages] = createSignal<{ time: number; text: string; ok: boolean }[]>([]);
/** The left side (0.11.3): which panel is open beside the rail (none: folded). */
export type Side = "symbols" | "plant" | "inspector" | "documents" | "";
export const [side, setSideRaw] = createSignal<Side>((["symbols", "plant", "inspector", "documents", ""].includes(prefKeep("ds.side", "symbols")) ? prefKeep("ds.side", "symbols") : "symbols") as Side);
export const setSide = (v: Side) => { setSideRaw(v); prefSave("ds.side", v); queueMicrotask(() => window.dispatchEvent(new Event("resize"))); };
/** Hidden layers: process, utility, instrument, notes. */
export const [hidden, setHidden] = createSignal<string[]>([]);

// ---------- editing ----------
export const act = async (op: object | string, quiet = false): Promise<Out | null> => {
  try { const o = await invoke<Out>("apply", { op }); setOut(o); return o; }
  catch (e) { if (!quiet) toast(String(e)); return null; }
};
export const call = async <T = any>(cmd: string, args?: object): Promise<T | undefined> => {
  try { return await invoke<T>(cmd, args); } catch (e) { toast(String(e)); return undefined; }
};
/** Counts projects loaded, so the canvas fits each newly opened drawing. */
export const [loads, setLoads] = createSignal(0);
let loaded = () => {};
export const onLoadHook = (f: () => void) => { loaded = f; };
export const loadOut = async (cmd: string, args?: object) => {
  const o = await call<Out | null>(cmd, args);
  if (o) wantFit = true;
  if (o) batch(() => { setSel([]); setSheetIdRaw(o.p.sheets[0]?.id ?? 0); setOut(o); setBackstage(null); setTool({ k: "select" }); setLoads(n => n + 1); loaded(); });
  return o;
};

// ---------- messages, menus, dialogs ----------
export const [msg, setMsg] = createSignal<{ text: string; ok: boolean } | null>(null);
let timer = 0;
export const toast = (text: string, ok = false) => { setMsg({ text, ok }); setMessages(v => [...v.slice(-199), { time: Date.now(), text, ok }]); clearTimeout(timer); timer = window.setTimeout(() => setMsg(null), ok ? 3000 : 7000); };
export type MenuItem = { label: string; run?: () => void; keys?: string; off?: boolean; on?: boolean; sub?: MenuItem[]; icon?: string } | "-";
export type Item_ = MenuItem;
export const [menu, setMenu] = createSignal<{ x: number; y: number; items: MenuItem[] } | null>(null);
export const openMenu = (e: MouseEvent | { x: number; y: number }, items: MenuItem[]) => {
  if ("preventDefault" in e) e.preventDefault();
  const r = (e as MouseEvent).currentTarget instanceof HTMLElement && (e as MouseEvent).type === "click" ? ((e as MouseEvent).currentTarget as HTMLElement).getBoundingClientRect() : null;
  setMenu({ x: r ? r.left : (e as MouseEvent).clientX ?? (e as any).x, y: r ? r.bottom + 2 : (e as MouseEvent).clientY ?? (e as any).y, items });
};
export const [dialogs, setDialogs] = createSignal<(() => JSX.Element)[]>([]);
export const openDialog = (d: () => JSX.Element) => setDialogs(v => [...v, d]);
export const closeDialog = () => setDialogs(v => v.slice(0, -1));

// ---------- tabs beside the sheets (0.11.2): a list or a big window opened full size ----------
/** A tab beside the sheet tabs: a list (its bottom-panel key) or a window moved there. */
export type DocTab = { id: string; title: string; list?: string; render?: () => JSX.Element };
export const [docTabs, setDocTabs] = createSignal<DocTab[]>([]);
export const [activeDoc, setActiveDoc] = createSignal<string | null>(null);
export const openDoc = (t: DocTab) => { setDocTabs(v => (v.some(x => x.id === t.id) ? v : [...v, t])); setActiveDoc(t.id); };
export const closeDoc = (id: string) => { const v = docTabs(), i = v.findIndex(x => x.id === id); setDocTabs(v.filter(x => x.id !== id)); if (activeDoc() === id) setActiveDoc(v[i + 1]?.id ?? v[i - 1]?.id ?? null); };
const keepPref = (k: string, d: string) => { try { return localStorage.getItem(k) ?? d; } catch { return d; } };
const savePref = (k: string, v: string) => { try { localStorage.setItem(k, v); } catch { /* private window */ } };
/** Where lists open (the user's choice, kept): in the panel below the sheet, or as tabs beside the sheets. */
export const [listWhere, setListWhereRaw] = createSignal<"below" | "tab">(keepPref("ds.lists.where", "below") === "tab" ? "tab" : "below");
export const setListWhere = (v: "below" | "tab") => { setListWhereRaw(v); savePref("ds.lists.where", v); };
/** Where big windows (datasheets, loop diagrams, reports…) open: over the sheet, or as tabs. */
export const [bigWhere, setBigWhereRaw] = createSignal<"window" | "tab">(keepPref("ds.big.where", "window") === "tab" ? "tab" : "window");
export const setBigWhere = (v: "window" | "tab") => { setBigWhereRaw(v); savePref("ds.big.where", v); };
