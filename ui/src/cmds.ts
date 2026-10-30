// Commands: what the ribbon, menus, keys and Tell Me run.
import { Out, Pt, Item, Line, Note, NoteKind, Kind, Example, invoke } from "./api";
import { setOut, act, call, sel, setSel, setBackstage, loadOut, toast, proj, sheet, setSheetId, itemOf, lineOf, derived, symOf, setTool, openMenu, setHiNumber, hidden, setHidden, setPanel, setPanelOpen } from "./store";
import * as D from "./dialogs";
import { sheetSvg } from "./print";

let fitHook: () => void = () => {};
export const setFit = (f: () => void) => { fitHook = f; };
let centreHook: (ids: number[], sheet: number) => void = () => {};
export const setCentre = (f: (ids: number[], sheet: number) => void) => { centreHook = f; };
let clip: { items: Item[]; lines: Line[]; notes: Note[] } | null = null;
let copyHook: () => { items: Item[]; lines: Line[]; notes: Note[] } = () => ({ items: [], lines: [], notes: [] });
export const setCopy = (f: typeof copyHook) => { copyHook = f; };
let viewCentre: () => Pt = () => [150, 100];
export const setViewCentre = (f: () => Pt) => { viewCentre = f; };

const sid = () => sheet()?.id ?? 0;

export const C = {
  // ---------- file ----------
  save: async (asNew = false) => { const o = await call<Out | null>("save", { asNew }); if (o) { setOut(o); toast("Saved.", true); } },
  // Each of these replaces the open project, so unsaved changes are offered for saving first.
  open: () => D.keepChanges(() => loadOut("open", { path: null })),
  openPath: (path: string) => D.keepChanges(() => loadOut("open", { path })),
  newProject: (example: string | null) => D.keepChanges(() => loadOut("new_project", { example })),
  examples: async () => setBackstage("home"),
  print: () => setBackstage("print"),
  undo: async () => { const o = await call<Out>("history", { redo: false }); if (o) setOut(o); },
  redo: async () => { const o = await call<Out>("history", { redo: true }); if (o) setOut(o); },

  // ---------- placing and editing ----------
  /** Place a symbol at a point; an in-line symbol on a line (`on`) goes into it. */
  place: async (sym: string, at: Pt, on: number | null, tag?: string, obj?: number) => {
    const s = symOf(sym); if (!s || !sheet()) return;
    const item: Item = { id: 0, sheet: sid(), sym, x: at[0], y: at[1], rot: 0, flip: false, scale: 1, obj: 0, link: 0 };
    let o = await act({ AddItem: { item, on, tag: tag ?? "", obj: obj ?? null } });
    if (o && obj === undefined && (s.cat === "instrument" || s.cat === "primary")) {
      const placed = o.p.items.find(i => i.id === o!.created[0]);
      if (placed) o = (await act({ SetData: { ids: [placed.obj], key: "location", val: sym === "inst_dcs" ? "DCS" : sym === "inst_plc" ? "PLC" : sym === "inst_panel" ? "Control room panel" : sym === "inst_aux" ? "Local panel" : "Field" } }, true)) ?? o;
    }
    if (o?.created.length) setSel([o.created[0]]);
    if (o && on !== null && s.inline) toast(`${s.name} put into the line; both halves keep its data.`, true);
  },
  addNote: async (kind: NoteKind, at: Pt) => {
    const text = kind === "hold" ? "" : kind === "leader" ? "Note" : "Note";
    const o = await act({ AddNote: { note: { id: 0, sheet: sid(), kind, x: at[0], y: at[1], w: kind === "hold" ? 60 : 0, h: kind === "hold" ? 35 : 0, text, to: [at[0] + 20, at[1] + 15] } } });
    if (o?.created.length) { setSel([o.created[0]]); setTimeout(() => document.querySelector<HTMLTextAreaElement>(".insp textarea")?.focus(), 40); }
  },
  /** Delete the selection. When that would leave objects drawn nowhere, ask: delete the objects
   *  too, or only remove them from this sheet (they stay in the Plant panel). */
  del: () => {
    const ids = sel(); const p = proj();
    if (!ids.length || !p) return toast("Select a symbol, a line or a note first.");
    const set = new Set(ids);
    const objs = new Set([...p.items.filter(i => set.has(i.id)).map(i => i.obj), ...p.lines.filter(l => set.has(l.id) && l.obj).map(l => l.obj)]);
    const lastView = [...objs].filter(o => !p.items.some(i => i.obj === o && !set.has(i.id)) && !p.lines.some(l => l.obj === o && !set.has(l.id)));
    const run = (keep: boolean) => act({ Delete: { ids, keep } }).then(o => o && setSel([]));
    if (!lastView.length) return run(false);
    D.deleteChoice(lastView.length, () => run(false), () => run(true));
  },
  rotate: () => { const ids = sel().filter(id => itemOf(id)); if (!ids.length) return toast("Select a symbol to turn."); act({ Rotate: { ids } }); },
  flip: () => { const ids = sel().filter(id => itemOf(id)); if (!ids.length) return toast("Select a symbol to mirror."); act({ Flip: { ids } }); },
  copy: () => { clip = copyHook(); if (clip.items.length + clip.lines.length + clip.notes.length) toast(`Copied ${clip.items.length + clip.lines.length + clip.notes.length} things.`, true); },
  paste: async () => {
    if (!clip) return toast("Copy something first (Ctrl+C).");
    const c = viewCentre(), xs = clip.items.map(i => i.x), ys = clip.items.map(i => i.y);
    const at: Pt = xs.length ? [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2] : c;
    const g = proj()?.settings.grid || 2.5;
    const same = clip.items.some(i => i.sheet === sid());
    const [dx, dy] = same ? [10, 10] : [Math.round((c[0] - at[0]) / g) * g, Math.round((c[1] - at[1]) / g) * g];
    const o = await act({ Paste: { sheet: sid(), items: clip.items, lines: clip.lines, notes: clip.notes, dx, dy } });
    if (o) setSel(o.created);
  },
  duplicate: () => { C.copy(); C.paste(); },
  reverse: (id: number) => { const l = lineOf(id); if (l) act({ SetLine: { line: { ...l, flip: !l.flip } } }); },
  /** Every drawn piece of the selected line's number. */
  selectRun: (id: number) => {
    const r = derived()?.view?.lines.find(l => l.id === id)?.run;
    if (r !== null && r !== undefined) setSel(derived()!.runs[r]);
  },
  lineTool: (kind: Kind) => setTool({ k: "line", kind }),
  placeTool: (sym: string, tag?: string) => setTool({ k: "place", sym, tag }),
  noteTool: (kind: NoteKind) => setTool({ k: "note", kind }),
  quickInsert: (at?: Pt) => D.quickInsert(at ?? viewCentre()),
  renumber: () => act("Renumber").then(o => o && toast("Every tag numbered again: per prefix, per sheet, left to right.", true)),
  units: (u: "SI" | "US") => { const p = proj(); if (p && p.units !== u) act({ SetProject: { name: p.name, units: u } }); },

  // ---------- off-page connectors ----------
  goPartner: (id: number) => {
    const it = itemOf(id), p = proj(); if (!it || !p) return;
    const q = p.items.find(x => x.id === it.link);
    if (!q) return toast("This connector has no partner yet. Pair it with one on another sheet (right-click › Pair with…).");
    centreHook([q.id], q.sheet);
  },
  pairDialog: (id: number) => D.pairDialog(id),

  // ---------- sheets ----------
  addSheet: async (legend = false) => {
    const s = sheet();
    const o = await act({ AddSheet: { sheet: { id: 0, number: "", title: legend ? "Legend" : "Piping and instrument diagram", size: s?.size ?? "A3", rev: "A", date: "", scale: "NTS", revs: [], legend } } });
    if (o?.created.length) setSheetId(o.created[0]);
  },
  deleteSheet: () => { const s = sheet(); if (s) D.confirm(`Delete sheet ${s.number} and everything on it?`, "Delete", () => act({ DeleteSheet: { id: s.id } })); },
  moveSheet: (by: number) => { const p = proj(), s = sheet(); if (!p || !s) return; const i = p.sheets.findIndex(x => x.id === s.id); act({ MoveSheet: { id: s.id, to: Math.max(0, i + by) } }); },

  // ---------- selection, layers, views ----------
  selectType: (cat: string) => {
    const v = derived()?.view; if (!v) return;
    const ids = cat.startsWith("line:") ? v.lines.filter(l => l.kind === cat.slice(5)).map(l => l.id) : v.items.filter(i => i.cat === cat).map(i => i.id);
    setSel(ids); toast(`${ids.length} selected.`, true);
  },
  /** Highlight a line number on every sheet, and select its pieces on this one. */
  highlight: (n: string) => { setHiNumber(n); const v = derived()?.view; if (v) setSel(v.lines.filter(l => l.number === n).map(l => l.id)); },
  toggleLayer: (l: string) => setHidden(hidden().includes(l) ? hidden().filter(x => x !== l) : [...hidden(), l]),
  fit: () => fitHook(),
  panel: (k: string) => { setPanel(k); setPanelOpen(true); },
  goTo: (ids: number[], sheet: number) => centreHook(ids, sheet),

  // ---------- export ----------
  exportList: async (kind: string, format: "csv" | "xlsx") => { const f = await call<string | null>("export", { kind: format, what: kind, sheet: sid() }); if (f) toast(`Saved ${f}.`, true); },
  exportDatasheets: async () => { const f = await call<string | null>("export", { kind: "datasheets", what: "", sheet: sid() }); if (f) toast(`Saved ${f}.`, true); },
  exportCause: async () => { const f = await call<string | null>("export", { kind: "cause", what: "", sheet: 0 }); if (f) toast(`Saved ${f}.`, true); },
  exportDexpi: async () => { const f = await call<string | null>("export", { kind: "dexpi", what: "", sheet: sid() }); if (f) toast(`Saved ${f}: this sheet in the DEXPI P&ID exchange format.`, true); },
  exportDxf: async () => { const f = await call<string | null>("export", { kind: "dxf", what: "", sheet: sid() }); if (f) toast(`Saved ${f}. It opens in any CAD program.`, true); },
  exportSvg: async () => { const s = sheet(); if (!s) return; const svg = await sheetSvg(s.id); const f = await call<string | null>("save_file", { name: `${s.number}.svg`, data: svg }); if (f) toast(`Saved ${f}.`, true); },
  planned: (what: string, v: string) => toast(`${what} arrives in ${v}. File › Roadmap shows what comes when.`),
};

export const exampleMenu = async (e: MouseEvent) => {
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
  const list = await invoke<Example[]>("examples_list").catch(() => [] as Example[]);
  openMenu({ x: r.left, y: r.bottom + 2 }, list.length ? list.map(x => ({ label: x.name, run: () => C.newProject(x.id) })) : [{ label: "The examples arrive in 0.1.3", off: true }]);
};

export const ALL_CMDS: [string, () => void, string?][] = [
  ["Save", () => C.save(), "Ctrl+S"], ["Save As", () => C.save(true)], ["Open", C.open, "Ctrl+O"], ["New project", () => setBackstage("home"), "Ctrl+N"], ["Print / PDF", C.print, "Ctrl+P"],
  ["Undo", C.undo, "Ctrl+Z"], ["Redo", C.redo, "Ctrl+Y"], ["Delete", C.del, "Del"], ["Rotate", C.rotate, "R"], ["Mirror", C.flip, "M"],
  ["Copy", C.copy, "Ctrl+C"], ["Paste", C.paste, "Ctrl+V"], ["Duplicate with the next free tag", C.duplicate, "Ctrl+D"],
  ["Add a symbol (type its name)", () => C.quickInsert()], ["Draw a major process line", () => C.lineTool("major"), "L"], ["Draw a minor process line", () => C.lineTool("minor")], ["Draw a utility line", () => C.lineTool("utility")],
  ["Draw a pneumatic signal", () => C.lineTool("pneumatic")], ["Draw an electrical signal", () => C.lineTool("electric")], ["Draw a software or data link", () => C.lineTool("data")], ["Draw a capillary", () => C.lineTool("capillary")],
  ["Add a note", () => C.noteTool("note")], ["Add a leader note", () => C.noteTool("leader")], ["Add a Hold cloud", () => C.noteTool("hold")],
  ["Off-page connector", () => C.placeTool("offpage")], ["Renumber every tag", C.renumber],
  ["New sheet", () => C.addSheet()], ["Legend sheet", () => C.addSheet(true)], ["Delete this sheet", C.deleteSheet],
  ["SI units", () => C.units("SI")], ["US units", () => C.units("US")], ["Fit the sheet", C.fit, "F"],
  ["Checks panel", () => C.panel("checks")], ["Line list", () => C.panel("lines")], ["Equipment list", () => C.panel("equipment")], ["Instrument index", () => C.panel("instruments")], ["Valve list", () => C.panel("valves")], ["Datasheet index", () => C.panel("datasheets")], ["Loop diagrams", () => import("./loops").then(m => m.openLoops())], ["Cause and effect matrix", () => import("./cause").then(m => m.openCause())], ["Export DEXPI (P&ID exchange)", () => C.exportDexpi()], ["Import datasheets from Excel", () => import("./datasheet").then(m => m.importDatasheets())],
  ["Export DXF", C.exportDxf], ["Save the sheet as SVG", C.exportSvg],
  ["Options", () => setBackstage("options")], ["Check for updates", () => setBackstage("about")], ["Roadmap", () => setBackstage("roadmap")],
  ["Keyboard shortcuts", () => D.shortcuts(), "F1"], ["Getting started", () => D.gettingStarted()], ["Glossary", () => D.glossary()],
];
