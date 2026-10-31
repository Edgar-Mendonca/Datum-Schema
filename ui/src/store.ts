// Window state: the open project, the open sheet, what the engine works out from it (the sheet's
// drawing, the checks, the hydraulics), the selection, the drawing tool, menus, dialogs and messages.
import { createSignal, createMemo, batch, JSX } from "solid-js";
import { invoke, Out, Options, Project, Derived, Library, SymInfo, Kind, NoteKind, Item, Line, Obj, LibView } from "./api";

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
export type Tool = { k: "select" } | { k: "line"; kind: Kind } | { k: "place"; sym: string; tag?: string; obj?: number } | { k: "note"; kind: NoteKind } | { k: "comment" };
export const [tool, setTool] = createSignal<Tool>({ k: "select" });
/** The line number highlighted on every sheet (Select › By line number). */
export const [hiNumber, setHiNumber] = createSignal("");
/** The bottom panel's tab, and whether it is open. */
export const [panel, setPanel] = createSignal("checks");
export const [panelOpen, setPanelOpen] = createSignal(true);
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
export const toast = (text: string, ok = false) => { setMsg({ text, ok }); clearTimeout(timer); timer = window.setTimeout(() => setMsg(null), ok ? 3000 : 7000); };
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
