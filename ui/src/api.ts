// Calls into the Rust shell and the shapes it returns (they mirror crates/core/src/model.rs, view.rs,
// checks.rs, hydraulics.rs and lists.rs).
declare global {
  interface Window { __TAURI__?: { core: { invoke: (cmd: string, args?: object) => Promise<any> }; event: { listen: (e: string, f: (x: { payload: any }) => void) => Promise<() => void> } } }
}

export const invoke = <T = any>(cmd: string, args?: object): Promise<T> =>
  window.__TAURI__ ? window.__TAURI__.core.invoke(cmd, args) : Promise.reject("Datum Schema must run inside its desktop window.");
export const listen = (e: string, f: (p: any) => void) => window.__TAURI__?.event.listen(e, x => f(x.payload));

export type Pt = [number, number];
export type Val = number | string;
export type Data = Record<string, Val>;
export interface Title { company: string; project: string; client: string; unit: string; by: string; checked: string; approved: string; extra: Record<string, string> }
/** A title block cell in mm from the block's top left; `field` is a key, "text:…" or "f:<own field>". */
export interface TbCell { x: number; y: number; w: number; h: number; label: string; field: string; size: number; bold: boolean }
export interface TitleBlock { on: boolean; name: string; w: number; h: number; cells: TbCell[] }
export interface Rev { rev: string; date: string; desc: string; by: string; checked: string; approved: string }
export interface Sheet { id: number; number: string; title: string; size: string; rev: string; date: string; scale: string; revs: Rev[]; legend: boolean; underlay: Underlay | null; pfd: boolean; snaps: Snap[]; clouds: string }
/** A sheet as issued at a revision (its contents stay in the engine). */
export interface Snap { rev: string; date: string }
export interface SheetChange { kind: "added" | "removed" | "changed"; what: string; ids: number[]; bbox: [number, number, number, number] }
export interface Compare { rev: string; next: string; changes: SheetChange[]; areas: [number, number, number, number][] }
/** An old drawing behind a sheet, placed at (x, y) mm and scaled by k; its drawing is fetched by rev. */
export interface Underlay { name: string; w: number; h: number; x: number; y: number; k: number; show: boolean; print: boolean; rev: string }
export interface Item { id: number; sheet: number; sym: string; x: number; y: number; rot: number; flip: boolean; scale: number; obj: number; link: number; base: string }
/** A plant object: the master of a tag and its data; symbols and line pieces are views of it. */
export interface Obj { id: number; uid: string; class: string; tag: string; node: number; data: Data }
export interface PlantNode { id: number; parent: number; level: "plant" | "unit" | "area"; name: string }
export interface Change { seq: number; time: number; user: string; what: string; obj: number; uid: string; key: string; old: string; new: string }
export type Kind = "major" | "minor" | "utility" | "pneumatic" | "electric" | "data" | "capillary";
export type End = { at: "port"; item: number; port: number } | { at: "line"; line: number; x: number; y: number } | { at: "free"; x: number; y: number };
export interface Line { id: number; sheet: number; kind: Kind; a: End; b: End; pts: Pt[]; flip: boolean; obj: number }
export type NoteKind = "note" | "leader" | "hold";
export interface Note { id: number; sheet: number; kind: NoteKind; x: number; y: number; w: number; h: number; text: string; to: Pt }
export interface Limit { service: string; name: string; vmin: number; vmax: number; dp: number }
export interface Settings { line_format: string; off: number[]; limits: Limit[]; roughness: number; grid: number; hidden: string[]; user_attrs: UserAttr[]; ds_layout: DsLayout; standard: Standard; title_block: TitleBlock; specs: PipeSpec[]; components: string[] }
/** A pipe class: limits in SI (Pa, K, m), sizes as NPS text, and the components it allows by size. */
export interface SpecItem { kind: string; min: string; max: string; desc: string; code: string; ends: string }
/** What each end of a type brings with it (per end), in a size range. */
export interface Implied { what: string; ends: string; per: number; min: string; max: string; desc: string; code: string }
export interface PipeSpec { name: string; desc: string; rating: string; material: string; corrosion: number; max_p: number; max_t: number; min: string; max: string; sched: string; items: SpecItem[]; implied: Implied[] }
/** A drawing standard: a library, and for each of our symbols the library symbol that draws it. */
export interface Standard { library: string; map: Record<string, string> }
/** A connection point of a library symbol (dir: 0 right, 1 down, 2 left, 3 up). */
export interface CPort { x: number; y: number; dir: number; name: string; signal: boolean }
/** A library symbol: drawn from a client's legend; `like` is the Datum Schema symbol it stands for. */
export interface CustomSym { key: string; name: string; library: string; block: string; like: string; class: string; cat: Cat; w: number; h: number; draw: string; ports: CPort[]; prefix: string; inline: boolean; terminal: boolean }
export interface SymLibrary { format: number; name: string; source: string; symbols: CustomSym[] }
/** A library as listed: its file (empty: not saved yet), the library, each symbol's picture, notes. */
export interface LibView { file: string; library: SymLibrary; svgs: string[]; warnings: string[] }
/** The company's datasheet layout: form number, logo (data URL), footer, rows left out ("Section › Label"), section order. */
export interface DsLayout { form: string; logo: string; footer: string; hidden: string[]; order: string[] }
export interface Project { format: number; name: string; units: "SI" | "US"; title: Title; nodes: PlantNode[]; objects: Obj[]; sheets: Sheet[]; items: Item[]; lines: Line[]; notes: Note[]; comments: Comment[]; settings: Settings; next: number }
/** A review comment pinned to a sheet (times in seconds since 1970). */
export interface Reply { uid: string; by: string; time: number; text: string }
export interface Comment { uid: string; sheet: number; x: number; y: number; by: string; time: number; text: string; on: number[]; replies: Reply[]; resolved: boolean; resolved_by: string; status_time: number }
export interface Pin { uid: string; n: number; x: number; y: number; resolved: boolean }
export interface Out { p: Project; path: string | null; dirty: boolean; undo: boolean; redo: boolean; created: number[]; warnings: string[] }

// ---------- what the engine draws (view.rs) ----------
export type Prim =
  | { k: "poly"; pts: Pt[]; closed: boolean; fill: boolean; dash: boolean; w: number }
  | { k: "circle"; c: Pt; r: number; fill: boolean; w: number }
  | { k: "arc"; c: Pt; r: number; a0: number; a1: number; w: number }
  | { k: "text"; p: Pt; h: number; s: string; anchor: number; bold: boolean };
export type Cat = "vessel" | "rotating" | "heat" | "valve" | "control" | "fitting" | "instrument" | "primary" | "mark" | "connector";
export interface PortView { p: Pt; dir: number; signal: boolean; used: boolean }
export interface ItemView { id: number; obj: number; sym: string; cat: Cat; x: number; y: number; rot: number; flip: boolean; scale: number; tag: string; texts: Prim[]; bbox: [number, number, number, number]; ports: PortView[]; layer: string }
export interface LineView { id: number; obj: number; kind: Kind; path: Pt[]; segs: Pt[][]; marks: Prim[]; number: string; badge: number | null; run: number | null; layer: string }
export interface NoteView { id: number; kind: NoteKind; prims: Prim[]; bbox: [number, number, number, number] }
export interface LegendRow { sym: string | null; kind: Kind | null; x: number; y: number; name: string }
export interface SheetView { id: number; number: string; title: string; w: number; h: number; legend: boolean; frame: Prim[]; items: ItemView[]; lines: LineView[]; notes: NoteView[]; legend_rows: LegendRow[]; underlay: Underlay | null; clouds: Prim[]; logo: [number, number, number, number] | null; pins: Pin[] }
export interface Issue { rule: number; level: "error" | "warning" | "note"; msg: string; ids: number[]; sheet: number; at: Pt }
export interface Hyd { id: number; v: number; re: number; f: number; dp100: number; dp: number; length: number; service: string; service_name: string; vmin: number; vmax: number; dpmax: number; status: number; reason: string }
export interface Derived { view: SheetView | null; issues: Issue[]; hyd: (Hyd | null)[]; runs: number[][]; numbers: Record<string, string>; attrs: Record<string, Field[]> }

// ---------- library ----------
export interface Port { x: number; y: number; dir: number; name: string; signal: boolean }
export interface Field { key: string; label: string; q: string; type: string; choices: string[]; dexpi: string; required: boolean; from: string }
export interface ClassInfo { name: string; label: string; parent: string | null; symbols: string[]; attrs: Field[] }
export interface UserAttr { class: string; attr: Field }
export interface SymInfo { key: string; name: string; cat: Cat; cat_name: string; w: number; h: number; ports: Port[]; prefix: string; inline: boolean; terminal: boolean; rare: boolean; svg: string; texts: Prim[]; class: string; fields: Field[] }
export interface Size { nps: string; od: number; schedules: [string, number][] }
export interface Library { symbols: SymInfo[]; kinds: [Kind, string][]; line_fields: Field[]; classes: ClassInfo[]; sizes: Size[]; sizes_source: string; limits: Limit[] }
export interface Shown { q: string; label: string; factor: number; offset: number }
export interface Example { id: string; name: string; what: string }
export interface Col { key: string; label: string; q: string; edit: boolean; num: boolean }
export interface Row { ids: number[]; sheet: number; cells: string[]; status: number | null }
export interface List { kind: string; title: string; cols: Col[]; rows: Row[] }
export interface Options { autosave: number; beta: boolean; theme: string; openLast: boolean; units: "SI" | "US"; grid: boolean; paper: string; company: string; user: string; defaults: Settings | null }

/** A number typed by a person: "−" and "," are understood ("1,5" is 1.5, "1,200" is 1200); empty or
 * not a number gives null. */
export const parseNum = (s: string): number | null => {
  let t = s.trim().replace(/[−–]/g, "-").replace(/\s/g, "");
  if (!t) return null;
  if (t.includes(",")) t = t.includes(".") || /^-?\d{1,3}(,\d{3})+$/.test(t) ? t.replace(/,/g, "") : t.replace(",", ".");
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
};

/** Numbers for people: up to 4 significant digits, no trailing zeros. */
export const num = (v: number, sig = 4) => {
  if (!isFinite(v)) return v > 0 ? "∞" : v < 0 ? "−∞" : "—";
  if (v === 0) return "0";
  const a = Math.abs(v);
  if (a >= 1e7 || a < 1e-3) return v.toExponential(Math.max(0, sig - 1)).replace(/\.?0+e/, "e").replace("-", "−");
  return String(+v.toPrecision(Math.max(sig, Math.ceil(Math.log10(a + 1))))).replace("-", "−");
};

/** A datasheet row: the object's own value (key set, editable) or one taken from elsewhere (source set). */
export interface DsRow { key: string; label: string; value: string; unit: string; q: string; choices: string[]; required: boolean; source: string; changed: boolean; was: string }
export interface DsIssue { rev: string; status: string; date: string; by: string; note: string }
/** A datasheet with its revision state: the last issue (rev empty: never issued), the revision block, the Δ count and the next revision per status. */
export interface Datasheet { obj: number; item: number; sheet: number; tag: string; title: string; sections: { title: string; rows: DsRow[] }[]; rev: string; status: string; date: string; by: string; issues: DsIssue[]; changed: number; next_rev: Record<string, string>; form: string; logo: string; footer: string }
/** One value of a datasheet import: set (will be written), same, conflict (lives on the line, vessel or drawing), bad. */
export interface ImportChange { obj: number; tag: string; key: string; label: string; old: string; new: string; val: Val | null; kind: "set" | "same" | "conflict" | "bad"; note: string; sheet: string }
export interface ImportPlan { changes: ImportChange[]; unmatched: string[]; unknown: string[]; sheets: number; used: number }
export interface LoopInfo { key: string; tags: string[] }
/** A loop member and its wiring, in the order of LOOP_WIRING. */
export interface LoopMember { obj: number; item: number; sheet: number; tag: string; place: "field" | "system"; what: string; wiring: string[] }
export interface LoopView { key: string; number: string; title: string; w: number; h: number; prims: Prim[]; members: LoopMember[]; missing: string[] }
/** The wiring attributes of a loop member (loops::WIRING). */
export const LOOP_WIRING: [string, string][] = [["jb", "Junction box"], ["jb_terminals", "JB terminals"], ["cable", "Cable"], ["system", "Control system"], ["io", "I/O channel"]];
/** The statuses a datasheet is issued with, in order (model::STATUSES). */
export const STATUSES = ["Preliminary", "Issued for Enquiry", "Issued for Purchase", "As-built"];
/** The cause-and-effect matrix: causes down, effects across; cells name the interlock linking them. */
export interface CauseRow { obj: number; item: number; sheet: number; tag: string; what: string; setting: string }
export interface EffectCol { obj: number; item: number; sheet: number; tag: string; what: string; action: string }
export interface CauseMatrix { causes: CauseRow[]; effects: EffectCol[]; cells: string[][]; open: string[] }
