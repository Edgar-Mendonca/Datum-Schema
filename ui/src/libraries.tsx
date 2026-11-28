// Symbol libraries: a client's own symbols, read from their legend sheet (DXF, DWG or PDF) or drawn
// in the symbol editor, reviewed here (what each symbol stands for, its connection points), named
// as the user likes, kept in a project, on this computer, or both, and shared as a file. A project's drawing standard draws Datum Schema's symbols with a library's (engine:
// symlib.rs; the window only edits and shows).
import { For, Show, createMemo, createSignal, createEffect, createRoot, on } from "solid-js";
import { CPort, CustomSym, LibView, SymLibrary, Settings, Cat, Code, CodeList, invoke } from "./api";
import { call, act, proj, lib, refreshLibs, openDialog, toast, allLibs, LibEntry, out, docTabs, activeDoc, setActiveDoc, openDoc } from "./store";
import { Dialog, Pick, Check } from "./ui";
import { Help } from "./help";
import * as D from "./dialogs";

/** A library symbol's picture, from its SVG. */
export const LibPic = (p: { svg: string; w: number; h: number; size?: number }) => {
  const m = Math.max(p.w, p.h) * 0.6 + 1;
  return <svg class="sym libpic" width={p.size ?? 30} height={p.size ?? 30} viewBox={`${-m} ${-m} ${2 * m} ${2 * m}`} innerHTML={p.svg} />;
};

/** Our symbols to choose "stands for" from, grouped by category. */
const likeOptions = () => {
  const syms = (lib()?.symbols ?? []).filter(s => !s.key.startsWith("x.") && s.key !== "unknown");
  return [["", "— nothing of ours (a new kind) —"] as [string, string], ...syms.map(s => [s.key, `${s.cat_name} · ${s.name}`] as [string, string])];
};

/** Connection points: click the drawing to add one, drag one to move it, pick one to name it. */
const portEditor = (sym: CustomSym, svg: string, done: (ports: CPort[]) => void) => openDialog(() => {
  const [ports, setPorts] = createSignal<CPort[]>(sym.ports.map(q => ({ ...q })));
  const [selI, setSelI] = createSignal(ports().length ? 0 : -1);
  const [drag, setDrag] = createSignal(false);
  const like = () => lib()?.symbols.find(s => s.key === sym.like);
  const names = () => like()?.ports.map(q => q.name) ?? [];
  const m = Math.max(sym.w, sym.h) / 2 + 3;
  let el!: SVGSVGElement;
  const at = (e: MouseEvent): [number, number] => { const pt = el.createSVGPoint(); pt.x = e.clientX; pt.y = e.clientY; const q = pt.matrixTransform(el.getScreenCTM()!.inverse()); return [q.x, q.y]; };
  const snapTo = async (i: number, x: number, y: number) => {
    const q = await call<CPort>("port_at", { draw: sym.draw, w: sym.w, h: sym.h, x, y });
    if (q) setPorts(ports().map((p, j) => (j === i ? { ...p, x: q.x, y: q.y, dir: q.dir } : p)));
  };
  const down = async (e: MouseEvent) => {
    const [x, y] = at(e);
    const hit = ports().findIndex(q => Math.hypot(q.x - x, q.y - y) < m * 0.08);
    if (hit >= 0) { setSelI(hit); setDrag(true); return; }
    const used = new Set(ports().map(q => q.name));
    const name = names().find(n => !used.has(n)) ?? `port ${ports().length + 1}`;
    setPorts([...ports(), { x, y, dir: 0, name, signal: false }]);
    const i = ports().length - 1;
    setSelI(i);
    await snapTo(i, x, y);
  };
  const move = (e: MouseEvent) => { if (!drag() || selI() < 0) return; const [x, y] = at(e); setPorts(ports().map((p, j) => (j === selI() ? { ...p, x, y } : p))); };
  const up = async () => { if (!drag()) return; setDrag(false); const q = ports()[selI()]; if (q) await snapTo(selI(), q.x, q.y); };
  const set = (patch: Partial<CPort>) => setPorts(ports().map((p, j) => (j === selI() ? { ...p, ...patch } : p)));
  const arrow = (d: number) => ["→", "↓", "←", "↑"][d % 4];
  return (
    <Dialog title={`Connection points · ${sym.name}`} ok={() => { done(ports()); }} okText="Use these" help="libraries">
      <div class="pedit">
        <svg ref={el} class="sym pedsvg" viewBox={`${-m} ${-m} ${2 * m} ${2 * m}`} onMouseDown={down} onMouseMove={move} onMouseUp={up} onMouseLeave={up}>
          <rect x={-sym.w / 2} y={-sym.h / 2} width={sym.w} height={sym.h} class="pedbox" />
          <g innerHTML={svg} />
          <For each={ports()}>{(q, i) => <g class="pedport" classList={{ on: i() === selI(), sig: q.signal }}>
            <circle cx={q.x} cy={q.y} r={m * 0.045} />
            <text x={q.x} y={q.y - m * 0.08} font-size={String(m * 0.07)} text-anchor="middle">{q.name} {arrow(q.dir)}</text>
          </g>}</For>
        </svg>
        <div class="pedside">
          <p class="muted small">Click the drawing to add a connection point; it snaps onto the line nearest the click. Drag a point to move it.{names().length ? ` Like our ${like()?.name.toLowerCase()}, this symbol's points are called ${names().join(", ")}: lines follow them by name when a drawing changes standard.` : ""}</p>
          <Show when={ports()[selI()]} fallback={<p class="muted">No point chosen.</p>}>{q => <div class="form">
            <label>Name</label><div><input value={q().name} list="pednames" onChange={e => set({ name: e.currentTarget.value })} /><datalist id="pednames"><For each={names()}>{n => <option value={n} />}</For></datalist></div>
            <label>Leaves</label><div><Pick value={String(q().dir)} set={v => set({ dir: +v })} options={[["0", "→ right"], ["1", "↓ down"], ["2", "← left"], ["3", "↑ up"]]} /></div>
            <label>Takes</label><div><Pick value={q().signal ? "1" : "0"} set={v => set({ signal: v === "1" })} options={[["0", "Process lines (fluid)"], ["1", "Signals"]]} /></div>
            <label /><div><button class="btn sm" onClick={() => { setPorts(ports().filter((_, j) => j !== selI())); setSelI(-1); }}>Remove this point</button></div>
          </div>}</Show>
        </div>
      </div>
    </Dialog>
  );
});

const CAT_NAMES: [Cat, string][] = [["vessel", "Vessels"], ["rotating", "Rotating equipment"], ["heat", "Heat transfer"], ["valve", "Manual valves"], ["control", "Control and relief valves"], ["fitting", "Fittings and specials"], ["instrument", "Instruments"], ["primary", "Primary elements"], ["mark", "Line marks and notes"], ["connector", "Connectors"]];

/** A symbol's drawing as shapes the editor can pick, move and delete (the symbol language parsed). */
type P = [number, number];
type Ent =
  | { k: "l" | "p" | "f" | "d"; pts: P[] }
  | { k: "c" | "fc"; c: P; r: number }
  | { k: "a"; c: P; r: number; a0: number; a1: number }
  | { k: "r"; a: P; b: P }
  | { k: "t"; p: P; h: number; s: string };
const r2 = (v: number) => Math.round(v * 100) / 100;
const parseDraw = (d: string): Ent[] => d.split(";").map(x => x.trim()).filter(Boolean).flatMap((cmd): Ent[] => {
  const [op, ...rest] = cmd.split(/\s+/);
  if (op === "t" || op === "tl" || op === "tr") { const n = rest.slice(0, 3).map(Number); return n.length === 3 && n.every(Number.isFinite) ? [{ k: "t", p: [n[0], n[1]], h: n[2], s: rest.slice(3).join(" ") }] : []; }
  const n = rest.map(Number).filter(Number.isFinite);
  const pts = (): P[] => { const v: P[] = []; for (let k = 0; k + 1 < n.length; k += 2) v.push([n[k], n[k + 1]]); return v; };
  if (["l", "p", "f", "d"].includes(op)) return [{ k: op as "l", pts: pts() }];
  if ((op === "c" || op === "fc") && n.length === 3) return [{ k: op, c: [n[0], n[1]], r: n[2] }];
  if (op === "a" && n.length === 5) return [{ k: "a", c: [n[0], n[1]], r: n[2], a0: n[3], a1: n[4] }];
  if (op === "r" && n.length === 4) return [{ k: "r", a: [n[0], n[1]], b: [n[0] + n[2], n[1] + n[3]] }];
  if (op === "e" && n.length === 6) { const v: P[] = []; for (let k = 0; k <= 16; k++) { const t = (n[4] + (n[5] - n[4]) * k / 16) * Math.PI / 180; v.push([r2(n[0] + n[2] * Math.cos(t)), r2(n[1] + n[3] * Math.sin(t))]); } return [{ k: "l", pts: v }]; }
  return [];
});
const entStr = (e: Ent): string => {
  switch (e.k) {
    case "c": case "fc": return `${e.k} ${r2(e.c[0])} ${r2(e.c[1])} ${r2(e.r)}`;
    case "a": return `a ${r2(e.c[0])} ${r2(e.c[1])} ${r2(e.r)} ${r2(e.a0)} ${r2(e.a1)}`;
    case "r": return `r ${r2(Math.min(e.a[0], e.b[0]))} ${r2(Math.min(e.a[1], e.b[1]))} ${r2(Math.abs(e.b[0] - e.a[0]))} ${r2(Math.abs(e.b[1] - e.a[1]))}`;
    case "t": return `t ${r2(e.p[0])} ${r2(e.p[1])} ${r2(e.h)} ${e.s.replace(/;/g, ",")}`;
    default: return `${e.k} ${e.pts.map(p => `${r2(p[0])} ${r2(p[1])}`).join(" ")}`;
  }
};
const arcPts = (c: P, r: number, a0: number, a1: number): P[] => { let b = a1; while (b <= a0) b += 360; const n = Math.max(6, Math.ceil((b - a0) / 6)); return Array.from({ length: n + 1 }, (_, k) => { const t = (a0 + (b - a0) * k / n) * Math.PI / 180; return [c[0] + r * Math.cos(t), c[1] + r * Math.sin(t)] as P; }); };
/** An entity's outline as polylines (for drawing, picking and intersections). */
const outline = (e: Ent): P[][] => {
  switch (e.k) {
    case "c": case "fc": return [arcPts(e.c, e.r, 0, 360)];
    case "a": return [arcPts(e.c, e.r, e.a0, e.a1)];
    case "r": return [[e.a, [e.b[0], e.a[1]], e.b, [e.a[0], e.b[1]], e.a]];
    case "t": return [];
    default: return [e.k === "l" || e.k === "d" ? e.pts : [...e.pts, e.pts[0]]];
  }
};
const dist = (a: P, b: P) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const segDist = (p: P, a: P, b: P) => { const dx = b[0] - a[0], dy = b[1] - a[1], L = dx * dx + dy * dy; const t = L ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / L)) : 0; return dist(p, [a[0] + t * dx, a[1] + t * dy]); };
const segX = (a: P, b: P, c: P, d: P): P | null => { const den = (b[0] - a[0]) * (d[1] - c[1]) - (b[1] - a[1]) * (d[0] - c[0]); if (Math.abs(den) < 1e-9) return null; const t = ((c[0] - a[0]) * (d[1] - c[1]) - (c[1] - a[1]) * (d[0] - c[0])) / den, u = ((c[0] - a[0]) * (b[1] - a[1]) - (c[1] - a[1]) * (b[0] - a[0])) / den; return t >= 0 && t <= 1 && u >= 0 && u <= 1 ? [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])] : null; };
/** Where a click can snap: ends, midpoints, centres, quadrants and crossings of what is drawn. */
const snapPoints = (es: Ent[]): [P, string][] => {
  const v: [P, string][] = [];
  const segs: [P, P][] = [];
  for (const e of es) {
    if (e.k === "c" || e.k === "fc") { v.push([e.c, "Centre"]); [0, 90, 180, 270].forEach(a => v.push([[e.c[0] + e.r * Math.cos(a * Math.PI / 180), e.c[1] + e.r * Math.sin(a * Math.PI / 180)], "Quadrant"])); continue; }
    if (e.k === "a") { const q = arcPts(e.c, e.r, e.a0, e.a1); v.push([e.c, "Centre"], [q[0], "End"], [q[q.length - 1], "End"], [q[Math.floor(q.length / 2)], "Midpoint"]); continue; }
    if (e.k === "t") { v.push([e.p, "Text"]); continue; }
    for (const o of outline(e)) { o.forEach(p => v.push([p, "End"])); for (let k = 0; k + 1 < o.length; k++) { v.push([[(o[k][0] + o[k + 1][0]) / 2, (o[k][1] + o[k + 1][1]) / 2], "Midpoint"]); segs.push([o[k], o[k + 1]]); } }
    if (e.k === "r") v.push([[(e.a[0] + e.b[0]) / 2, (e.a[1] + e.b[1]) / 2], "Centre"]);
  }
  for (let i = 0; i < segs.length && i < 400; i++) for (let j = i + 1; j < segs.length && j < 400; j++) { const x = segX(segs[i][0], segs[i][1], segs[j][0], segs[j][1]); if (x) v.push([x, "Crossing"]); }
  return v;
};
const pickDist = (e: Ent, p: P) => e.k === "t" ? dist(p, e.p) - 1 : e.k === "c" || e.k === "fc" ? (e.k === "fc" ? Math.max(0, dist(p, e.c) - e.r) : Math.abs(dist(p, e.c) - e.r)) : Math.min(...outline(e).flatMap(o => o.slice(1).map((q, k) => segDist(p, o[k], q))), Infinity);
const moveEnt = (e: Ent, d: P): Ent => mapEnt(e, p => [p[0] + d[0], p[1] + d[1]]);
/** An entity with every point mapped (a move, a mirror or a quarter turn); arcs keep their sense. */
const mapEnt = (e: Ent, f: (p: P) => P, angle?: (a: number) => number, swap = false): Ent => {
  switch (e.k) {
    case "c": case "fc": return { ...e, c: f(e.c) };
    case "a": { const a0 = angle ? angle(e.a0) : e.a0, a1 = angle ? angle(e.a1) : e.a1; return { ...e, c: f(e.c), a0: r2(swap ? a1 : a0), a1: r2(swap ? a0 : a1) }; }
    case "r": { const a = f(e.a), b = f(e.b); return { ...e, a: [Math.min(a[0], b[0]), Math.min(a[1], b[1])], b: [Math.max(a[0], b[0]), Math.max(a[1], b[1])] }; }
    case "t": return { ...e, p: f(e.p) };
    default: return { ...e, pts: e.pts.map(f) };
  }
};
const mirrorEnt = (e: Ent) => mapEnt(e, p => [r2(-p[0]), p[1]], a => 180 - a, true);
const turnEnt = (e: Ent) => mapEnt(e, p => [r2(-p[1]), r2(p[0])], a => a + 90);
const entPts = (e: Ent): P[] => (e.k === "t" ? [e.p] : outline(e).flat());

type STool = "sel" | "l" | "p" | "f" | "d" | "r" | "c" | "a" | "fc" | "t";
/** What each tool asks for, step by step (the hint over the drawing). */
const STEPS: Record<STool, string[]> = {
  sel: ["Click a shape, or drag a box round several; drag them to move; Delete removes them, Ctrl+D copies"],
  l: ["Click where the line starts", "Click the next point, or type its length (Tab: angle, Enter: place it); double-click or Enter ends the line"],
  d: ["Click where the dashed line starts", "Click the next point, or type its length (Tab: angle); double-click or Enter ends it"],
  p: ["Click the first corner", "Click the next corner, or type the side's length (Tab: angle); click the first corner, or Enter, to close it"],
  f: ["Click the first corner", "Click the next corner, or type the side's length (Tab: angle); click the first corner, or Enter, to close it"],
  r: ["Click one corner", "Click the opposite corner, or type its width and height (Tab between them, Enter to place)"],
  c: ["Click the centre", "Click a point on the circle, or type its diameter (Enter to place)"],
  fc: ["Click the centre", "Click a point on the dot's edge, or type its diameter (Enter to place)"],
  a: ["Click the arc's centre", "Click where it starts (that sets its radius)", "Click where it ends (it runs anticlockwise)"],
  t: ["Click where the text goes", "Type the text (Tab: its height in mm), Enter to place it"],
};
/** The sizes a tool takes typed in, by step. */
const SIZES: Partial<Record<STool, [string, string][]>> = {
  l: [["len", "Length"], ["ang", "Angle °"]], d: [["len", "Length"], ["ang", "Angle °"]], p: [["len", "Length"], ["ang", "Angle °"]], f: [["len", "Length"], ["ang", "Angle °"]],
  r: [["w", "Width"], ["h", "Height"]], c: [["dia", "Diameter"]], fc: [["dia", "Diameter"]], t: [["text", "Text"], ["th", "Height"]],
};

/** The symbol editor (0.11.7; first in 0.11.3), in a tab of its own: Datum Mesh's way of sketching,
 *  for symbols. Lines, shapes, filled shapes, dashed lines, rectangles, circles, arcs, dots and text,
 *  black on white. Points snap to the ends, midpoints, centres, quadrants and crossings of what is
 *  drawn; a line near level or upright is held there (a dashed guide and an H or V say so, Shift
 *  forces it); a point lined up with another is held in line with it (a dashed guide from it);
 *  else the 0.5 mm grid. The length and angle show as you draw; type them instead (Tab between,
 *  Enter places). Select picks shapes (a box picks several) to move, copy, mirror, turn or delete.
 *  Then say what it is: one of ours drawn your way, or a new kind of object. */
export const symbolEditor = (start: CustomSym | null, done: (c: CustomSym) => void) => {
  const body = () => <SymbolEditor start={start} done={done} />;
  if (!out()) return openDialog(body);
  const id = `symed:${start ? start.key || start.name : "new"}`;
  if (docTabs().some(t => t.id === id)) { setActiveDoc(id); return; }
  // Closed (used or cancelled), it goes back to the tab it was opened from (the Libraries tab).
  const from = activeDoc();
  openDoc({ id, title: start ? `Symbol · ${start.name}` : "New symbol", render: body });
  createRoot(dispose => createEffect(on(docTabs, v => { if (v.some(t => t.id === id)) return; dispose(); if (from && v.some(t => t.id === from)) setActiveDoc(from); }, { defer: true })));
};

const SymbolEditor = (props: { start: CustomSym | null; done: (c: CustomSym) => void }) => {
  const start = props.start, done = props.done;
  const [ents, setEntsRaw] = createSignal<Ent[]>(start ? parseDraw(start.draw) : []);
  const [undo, setUndo] = createSignal<Ent[][]>([]);
  const setEnts = (v: Ent[]) => { setUndo([...undo().slice(-49), ents()]); setEntsRaw(v); };
  const [tool, setToolRaw] = createSignal<STool>("l");
  const [cur, setCur] = createSignal<P[]>([]);
  const [hover, setHover] = createSignal<{ p: P; why: string; guides: [P, P][]; badge: string } | null>(null);
  const [picked, setPicked] = createSignal<number[]>([]);
  const [drag, setDrag] = createSignal<P | null>(null);
  const [box, setBox] = createSignal<{ a: P; b: P; add: boolean } | null>(null);
  const [typed, setTyped] = createSignal<Record<string, string>>({});
  const [view, setView] = createSignal({ x: 0, y: 0, half: 20 });
  const [c, setC] = createSignal<CustomSym>(start ? { ...start } : { key: "", name: "", library: "", block: "", like: "", class: "UnknownItem", cat: "mark", w: 10, h: 10, draw: "", ports: [], prefix: "", inline: false, terminal: false });
  const [svg, setSvg] = createSignal("");
  const G = 0.5;
  let el!: SVGSVGElement;
  const boxes: HTMLInputElement[] = [];
  const draw = () => ents().map(entStr).join("; ");
  createEffect(on(draw, d => { invoke<string>("symbol_svg", { draw: d }).then(setSvg, () => setSvg("")); }));
  const raw = (e: MouseEvent): P => { const pt = el.createSVGPoint(); pt.x = e.clientX; pt.y = e.clientY; const q = pt.matrixTransform(el.getScreenCTM()!.inverse()); return [q.x, q.y]; };
  // The view is square and fitted into the stage: its shorter side sets the scale.
  const mmPerPx = () => { const r = el?.getBoundingClientRect(); return 2 * view().half / Math.max(1, r ? Math.min(r.width, r.height) : 400); };
  const snaps = createMemo(() => snapPoints(ents()));
  const drawing = () => ["l", "d", "p", "f"].includes(tool());
  const setTool = (k: STool) => { finish(); setToolRaw(k); setPicked([]); setTyped({}); };
  /** The point the pointer means: a key point; else held level or upright from the last point, and in
   *  line with other points (each with its guide); else the grid. */
  const snap = (e: MouseEvent): { p: P; why: string; guides: [P, P][]; badge: string } => {
    let p = raw(e);
    const last = cur()[cur().length - 1], tol = 9 * mmPerPx(), guides: [P, P][] = [];
    let badge = "", lockX = false, lockY = false;
    if (e.shiftKey && last) { if (Math.abs(p[0] - last[0]) > Math.abs(p[1] - last[1])) { p = [p[0], last[1]]; lockY = true; badge = "H"; } else { p = [last[0], p[1]]; lockX = true; badge = "V"; } }
    if (!e.shiftKey) {
      const near = snaps().map(([q, why]) => ({ q, why, d: dist(q, p) })).filter(x => x.d < tol).sort((a, b) => a.d - b.d)[0];
      if (near) return { p: [r2(near.q[0]), r2(near.q[1])], why: near.why, guides, badge };
      if (cur().length > 2 && dist(cur()[0], p) < tol) return { p: cur()[0], why: "Close", guides, badge };
      if (last && tool() !== "sel") {
        if (Math.abs(p[1] - last[1]) < tol) { p = [p[0], last[1]]; lockY = true; badge = "H"; }
        else if (Math.abs(p[0] - last[0]) < tol) { p = [last[0], p[1]]; lockX = true; badge = "V"; }
      }
    }
    // In line with another point: the nearest key point (or a point of this chain) across or down.
    const keys: P[] = [...snaps().map(s => s[0]), ...cur().slice(0, -1), [0, 0]];
    if (!lockX) { const k = keys.filter(q => Math.abs(q[0] - p[0]) < tol).sort((a, b) => Math.abs(a[0] - p[0]) - Math.abs(b[0] - p[0]))[0]; if (k) { p = [k[0], p[1]]; lockX = true; guides.push([k, p]); } }
    if (!lockY) { const k = keys.filter(q => Math.abs(q[1] - p[1]) < tol).sort((a, b) => Math.abs(a[1] - p[1]) - Math.abs(b[1] - p[1]))[0]; if (k) { p = [p[0], k[1]]; lockY = true; guides.push([k, p]); } }
    const g = (v: number) => Math.round(v / G) * G;
    p = [lockX ? r2(p[0]) : g(p[0]), lockY ? r2(p[1]) : g(p[1])];
    for (const gd of guides) gd[1] = p;
    if (badge && last) guides.unshift([last, p]);
    return { p, why: badge ? (badge === "H" ? "Level" : "Upright") : guides.length ? "In line" : "Grid", guides, badge };
  };
  const finish = () => {
    const k = tool(), p = cur();
    if ((k === "l" || k === "d") && p.length >= 2) setEnts([...ents(), { k, pts: p }]);
    if ((k === "p" || k === "f") && p.length >= 3) setEnts([...ents(), { k, pts: dist(p[0], p[p.length - 1]) < 1e-6 ? p.slice(0, -1) : p }]);
    setCur([]); setTyped({});
  };
  /** A point given to the tool (clicked, or from the sizes typed). */
  const give = (p: P) => {
    const k = tool(), pts = [...cur(), p];
    if (k === "t") { setCur([p]); setTyped({ th: "2.5" }); queueMicrotask(() => boxes[0]?.focus()); return; }
    if (k === "c" || k === "fc") { if (pts.length === 2) { setEnts([...ents(), { k, c: pts[0], r: Math.max(k === "fc" ? 0.3 : 0.5, r2(dist(pts[0], pts[1]))) }]); setCur([]); setTyped({}); } else setCur(pts); return; }
    if (k === "r") { if (pts.length === 2) { if (dist(pts[0], pts[1]) > 0) setEnts([...ents(), { k: "r", a: [Math.min(pts[0][0], p[0]), Math.min(pts[0][1], p[1])], b: [Math.max(pts[0][0], p[0]), Math.max(pts[0][1], p[1])] }]); setCur([]); setTyped({}); } else setCur(pts); return; }
    if (k === "a") {
      if (pts.length === 3) { const ang = (q: P) => r2(Math.atan2(q[1] - pts[0][1], q[0] - pts[0][0]) * 180 / Math.PI); setEnts([...ents(), { k: "a", c: pts[0], r: r2(dist(pts[0], pts[1])), a0: ang(pts[1]), a1: ang(pts[2]) }]); setCur([]); }
      else setCur(pts);
      return;
    }
    // A shape closes when the chain comes back to its start.
    if ((k === "p" || k === "f") && pts.length > 3 && dist(pts[0], p) < 1e-6) { setCur(pts); finish(); return; }
    if (cur().length && dist(cur()[cur().length - 1], p) < 1e-6) return;
    setCur(pts); setTyped({});
  };
  /** The sizes typed, as the next point (or the text placed). */
  const num = (k: string) => { const v = typed()[k]; return v !== undefined && v.trim() !== "" && Number.isFinite(+v) ? +v : null; };
  const placeTyped = () => {
    const k = tool(), last = cur()[cur().length - 1];
    if (k === "t") { const s = (typed().text ?? "").trim(); if (s && last) setEnts([...ents(), { k: "t", p: last, h: Math.max(0.5, num("th") ?? 2.5), s: s.slice(0, 12) }]); setCur([]); setTyped({}); el.focus(); return; }
    if (!last) return;
    if (k === "c" || k === "fc") { const d = num("dia"); if (d && d > 0) give([r2(last[0] + d / 2), last[1]]); el.focus(); return; }
    if (k === "r") { const h0 = hover()?.p ?? last; const w = num("w") ?? Math.abs(h0[0] - last[0]), h = num("h") ?? Math.abs(h0[1] - last[1]); if (w > 0 && h > 0) give([r2(last[0] + w), r2(last[1] + h)]); el.focus(); return; }
    const hp = hover()?.p;
    const len = num("len") ?? (hp ? dist(last, hp) : null), ang = num("ang") ?? (hp ? Math.atan2(-(hp[1] - last[1]), hp[0] - last[0]) * 180 / Math.PI : 0);
    if (len && len > 0) give([r2(last[0] + len * Math.cos(ang * Math.PI / 180)), r2(last[1] - len * Math.sin(ang * Math.PI / 180))]);
    el.focus();
  };
  /** The live size by the pointer: length and angle, width and height, or diameter. */
  const readout = () => {
    const h = hover()?.p, last = cur()[cur().length - 1], k = tool(); if (!h || !last || k === "t" || k === "sel") return "";
    if (k === "r") return `${r2(Math.abs(h[0] - last[0]))} × ${r2(Math.abs(h[1] - last[1]))} mm`;
    if (k === "c" || k === "fc") return `⌀ ${r2(2 * dist(last, h))} mm`;
    if (k === "a") return cur().length === 1 ? `r ${r2(dist(last, h))} mm` : "";
    let a = Math.atan2(-(h[1] - last[1]), h[0] - last[0]) * 180 / Math.PI; if (a < 0) a += 360;
    return `${r2(dist(last, h))} mm · ${Math.round(a * 10) / 10}°`;
  };
  const down = (e: MouseEvent) => {
    if (e.button === 1) { e.preventDefault(); const a = raw(e), v0 = view(); const mv = (m: MouseEvent) => { const b = raw(m); setView({ ...v0, x: view().x - (b[0] - a[0]), y: view().y - (b[1] - a[1]) }); }; const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); }; addEventListener("mousemove", mv); addEventListener("mouseup", up); return; }
    if (e.button !== 0) return;
    el.focus();
    if (tool() === "sel") {
      const p = raw(e), tol = 8 * mmPerPx();
      const best = ents().map((x, i) => ({ i, d: pickDist(x, p) })).filter(x => x.d < tol).sort((a, b) => a.d - b.d)[0];
      if (!best) { setBox({ a: p, b: p, add: e.shiftKey }); if (!e.shiftKey) setPicked([]); return; }
      if (e.shiftKey) { setPicked(picked().includes(best.i) ? picked().filter(i => i !== best.i) : [...picked(), best.i]); return; }
      if (!picked().includes(best.i)) setPicked([best.i]);
      setUndo([...undo().slice(-49), ents()]); setDrag(snap(e).p);
      return;
    }
    give(snap(e).p);
  };
  const move = (e: MouseEvent) => {
    const b0 = box(); if (b0) { setBox({ ...b0, b: raw(e) }); return; }
    const s = snap(e);
    setHover(s);
    const d0 = drag();
    if (d0 && picked().length) { const d: P = [s.p[0] - d0[0], s.p[1] - d0[1]]; if (d[0] || d[1]) { setEntsRaw(ents().map((x, i) => (picked().includes(i) ? moveEnt(x, d) : x))); setDrag(s.p); } }
  };
  const up = () => {
    setDrag(null);
    const b0 = box(); if (!b0) return; setBox(null);
    const [x0, x1] = [Math.min(b0.a[0], b0.b[0]), Math.max(b0.a[0], b0.b[0])], [y0, y1] = [Math.min(b0.a[1], b0.b[1]), Math.max(b0.a[1], b0.b[1])];
    if (x1 - x0 < 0.2 && y1 - y0 < 0.2) return;
    const inside = ents().map((e, i) => ({ i, ok: entPts(e).every(q => q[0] >= x0 && q[0] <= x1 && q[1] >= y0 && q[1] <= y1) })).filter(x => x.ok).map(x => x.i);
    setPicked(b0.add ? [...new Set([...picked(), ...inside])] : inside);
  };
  const wheel = (e: WheelEvent) => { e.preventDefault(); const p = raw(e), v = view(), f = e.deltaY > 0 ? 1.2 : 1 / 1.2, half = Math.min(80, Math.max(3, v.half * f)); setView({ half, x: p[0] - (p[0] - v.x) * half / v.half, y: p[1] - (p[1] - v.y) * half / v.half }); };
  /** The picked shapes (or, with none picked, the whole drawing) mirrored, turned or copied. */
  const onPicked = (f: (e: Ent) => Ent, copy = false) => {
    const all = !picked().length, ids = all ? ents().map((_, i) => i) : picked();
    if (!ids.length) return;
    if (copy) { const n = ents().length; setEnts([...ents(), ...ids.map(i => f(ents()[i]))]); setPicked(ids.map((_, k) => n + k)); return; }
    setEnts(ents().map((x, i) => (ids.includes(i) ? f(x) : x)));
  };
  const delPicked = () => { if (!picked().length) return; setEnts(ents().filter((_, i) => !picked().includes(i))); setPicked([]); };
  const doUndo = () => { const u = undo(); if (u.length) { setEntsRaw(u[u.length - 1]); setUndo(u.slice(0, -1)); setPicked([]); } };
  const key = (e: KeyboardEvent) => {
    const ctrl = e.ctrlKey || e.metaKey, k = e.key.toLowerCase();
    // The keys the editor uses stay in it (Enter must not close the tab, Ctrl+Z must not undo the project).
    if (["Enter", "Escape", "Delete", "Backspace"].includes(e.key) || (ctrl && ["z", "a", "d", "y"].includes(k)) || /^[0-9.\-]$/.test(e.key)) e.stopPropagation();
    if (e.key === "Enter") { e.preventDefault(); if (SIZES[tool()] && cur().length && Object.values(typed()).some(v => v.trim())) placeTyped(); else finish(); return; }
    if (e.key === "Escape") { if (cur().length) { setCur([]); setTyped({}); } else setPicked([]); return; }
    if ((e.key === "Delete" || e.key === "Backspace") && picked().length) { e.preventDefault(); delPicked(); return; }
    if (ctrl && k === "z") { e.preventDefault(); doUndo(); return; }
    if (ctrl && k === "a") { e.preventDefault(); setToolRaw("sel"); setPicked(ents().map((_, i) => i)); return; }
    if (ctrl && k === "d") { e.preventDefault(); onPicked(x => moveEnt(x, [1, 1]), true); return; }
    // A size typed while a shape is drawn goes into its first box.
    const sz = SIZES[tool()];
    if (sz && cur().length && /^[0-9.\-]$/.test(e.key) && boxes[0]) { e.preventDefault(); boxes[0].focus(); setTyped({ ...typed(), [sz[0][0]]: e.key }); return; }
  };
  const boxKey = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") { e.preventDefault(); placeTyped(); }
    if (e.key === "Escape") { setTyped({}); el.focus(); }
  };
  const wh = (): [number, number] => {
    let w = 0, h = 0;
    for (const e of ents()) for (const p of entPts(e)) { w = Math.max(w, 2 * Math.abs(p[0])); h = Math.max(h, 2 * Math.abs(p[1])); }
    return [Math.max(1, r2(w)), Math.max(1, r2(h))];
  };
  const fromSym = (key: string) => { invoke<string>("symbol_draw", { key }).then(d => { setEnts(parseDraw(d)); setPicked([]); setCur([]); }, () => toast("That symbol's drawing could not be read.")); };
  const fit = () => { const [w, h] = wh(); setView({ x: 0, y: 0, half: Math.max(6, Math.min(80, Math.max(w, h) * 0.75 + 3)) }); };
  const classes = () => (lib()?.classes ?? []).filter(k => k.name !== "UnknownItem").map(k => [k.name, k.label || k.name] as [string, string]).sort((a, b) => a[1].localeCompare(b[1]));
  const tools: [STool, string, string][] = [["sel", "Select", "Pick shapes to move, copy, mirror, turn or delete (a box picks several)"], ["l", "Line", "A line through each point you click"], ["p", "Shape", "A closed outline"], ["f", "Filled", "A closed outline, filled in black"], ["d", "Dashed", "A dashed line"], ["r", "Rectangle", "Two opposite corners"], ["c", "Circle", "Its centre, then its edge"], ["a", "Arc", "Its centre, its start and its end"], ["fc", "Dot", "A filled circle"], ["t", "Text", "A letter or a short word, such as M for a motor"]];
  const ok = () => {
    if (!c().name.trim()) { toast("Give the symbol a name."); return false; }
    if (!draw().trim()) { toast("Draw the symbol first."); return false; }
    const [w, h] = wh();
    done({ ...c(), name: c().name.trim(), block: c().block || c().name.trim().toUpperCase().replace(/[^A-Z0-9]+/g, "_").slice(0, 40), draw: draw(), w, h, inline: c().inline && c().ports.filter(q => !q.signal).length === 2 });
  };
  const rubber = () => { const p = cur(), h = hover()?.p; if (!p.length || !h) return ""; const k = tool(); if (k === "c" || k === "fc") { const r = dist(p[0], h); return `M${p[0][0] + r},${p[0][1]}A${r},${r} 0 1 1 ${p[0][0] - r},${p[0][1]}A${r},${r} 0 1 1 ${p[0][0] + r},${p[0][1]}`; } if (k === "r") return `M${p[0][0]},${p[0][1]}H${h[0]}V${h[1]}H${p[0][0]}Z`; if (k === "a" || k === "t") return ""; return `M${[...p, h].map(q => `${q[0]},${q[1]}`).join("L")}`; };
  const step = () => { const s = STEPS[tool()]; return s[Math.min(cur().length, s.length - 1)]; };
  const vb = () => { const v = view(); return `${v.x - v.half} ${v.y - v.half} ${2 * v.half} ${2 * v.half}`; };
  const fs = () => 11 * mmPerPx();
  return (
    <Dialog title={start ? `Symbol · ${start.name}` : "New symbol"} wide help="symboleditor" ok={ok} okText="Use this symbol">
      <div class="symed">
        <div class="symtools">
          <For each={tools}>{([k, l, t]) => <button class="btn sm" data-t={k} classList={{ on: tool() === k }} title={t} onClick={() => setTool(k)}>{l}</button>}</For>
          <span class="sep" />
          <button class="btn sm" data-a="mirror" title="Mirror left to right about the cross (the picked shapes, or the whole drawing)" onClick={() => onPicked(mirrorEnt)}>Mirror</button>
          <button class="btn sm" data-a="turn" title="Turn a quarter clockwise about the cross (the picked shapes, or the whole drawing)" onClick={() => onPicked(turnEnt)}>Turn</button>
          <button class="btn sm" data-a="copy" title="A copy of the picked shapes, 1 mm across and down (Ctrl+D)" onClick={() => onPicked(x => moveEnt(x, [1, 1]), true)} disabled={!picked().length}>Copy</button>
          <button class="btn sm" data-a="del" title="Delete the picked shapes (Delete)" onClick={delPicked} disabled={!picked().length}>Delete</button>
          <span class="grow" />
          <button class="btn sm" title="Show the whole symbol" onClick={fit}>Fit</button>
          <button class="btn sm" title="Take back the last change (Ctrl+Z)" onClick={doUndo}>Undo</button>
          <button class="btn sm" title="Start again from nothing" onClick={() => { setCur([]); setPicked([]); setEnts([]); }}>Clear</button>
        </div>
        <div class="symstage">
          <div class="symstep"><b>{tools.find(t => t[0] === tool())?.[1]}</b><span>{step()}</span></div>
          <svg ref={el} class="symedsvg" viewBox={vb()} tabindex="0" onMouseDown={down} onMouseUp={up} onDblClick={() => drawing() && finish()} onMouseMove={move} onMouseLeave={() => { setHover(null); }} onKeyDown={key} onWheel={wheel} onContextMenu={e => { e.preventDefault(); finish(); }}>
            <defs>
              <pattern id="symgrid" width="1" height="1" patternUnits="userSpaceOnUse"><path d="M1 0H0V1" fill="none" class="symgridl" /></pattern>
              <pattern id="symgrid5" width="5" height="5" patternUnits="userSpaceOnUse"><path d="M5 0H0V5" fill="none" class="symgridm" /></pattern>
            </defs>
            <rect x={-200} y={-200} width={400} height={400} fill="#fff" />
            <rect x={-200} y={-200} width={400} height={400} fill="url(#symgrid)" />
            <rect x={-200} y={-200} width={400} height={400} fill="url(#symgrid5)" />
            <line x1={-200} y1={0} x2={200} y2={0} class="symaxis" /><line x1={0} y1={-200} x2={0} y2={200} class="symaxis" />
            <g class="symink" innerHTML={svg()} />
            <For each={picked().map(i => ents()[i]).filter(Boolean)}>{e => <g class="sympick"><For each={outline(e)}>{o => <polyline points={o.map(q => q.join(",")).join(" ")} />}</For><Show when={e.k === "t"}><circle cx={(e as { p: P }).p[0]} cy={(e as { p: P }).p[1]} r={1.2} /></Show></g>}</For>
            <Show when={box()}>{b => <rect class="symbox" x={Math.min(b().a[0], b().b[0])} y={Math.min(b().a[1], b().b[1])} width={Math.abs(b().b[0] - b().a[0])} height={Math.abs(b().b[1] - b().a[1])} />}</Show>
            <Show when={hover()}>{h => <For each={h().guides}>{g => <line class="symguide" x1={g[0][0]} y1={g[0][1]} x2={g[1][0]} y2={g[1][1]} />}</For>}</Show>
            <Show when={rubber()}><path d={rubber()} class="symcur" /></Show>
            <For each={cur()}>{q => <circle cx={q[0]} cy={q[1]} r={0.35} class="symvert" />}</For>
            <Show when={hover()}>{h => <g class="symsnap">
              <Show when={h().why !== "Grid"} fallback={<circle cx={h().p[0]} cy={h().p[1]} r={0.3} class="symhov" />}><rect x={h().p[0] - 0.6} y={h().p[1] - 0.6} width={1.2} height={1.2} class="symsq" />
                <Show when={!h().badge} fallback={<g class="symbadge"><rect x={h().p[0] + 1} y={h().p[1] + 0.8} width={fs() * 1.4} height={fs() * 1.4} rx={fs() * 0.2} /><text x={h().p[0] + 1 + fs() * 0.7} y={h().p[1] + 0.8 + fs() * 1.05} style={{ "font-size": `${fs()}px` }} text-anchor="middle">{h().badge}</text></g>}>
                  <text x={h().p[0] + 1} y={h().p[1] - 1} class="symwhy" style={{ "font-size": `${fs()}px` }}>{h().why}</text></Show></Show>
              <Show when={readout()}><text x={h().p[0] + 1} y={h().p[1] + (h().badge ? 1 + fs() * 3 : fs() * 1.6)} class="symread" style={{ "font-size": `${fs()}px` }}>{readout()}</text></Show>
            </g>}</Show>
          </svg>
          <Show when={SIZES[tool()] && cur().length}>
            <div class="symsizes">
              <For each={SIZES[tool()]!}>{([k, l], i) => <label><span>{l}</span><input class="mono" classList={{ wide: k === "text" }} ref={x => { boxes[i()] = x; }} value={typed()[k] ?? ""} placeholder={k === "th" ? "2.5" : ""} onInput={e => setTyped({ ...typed(), [k]: e.currentTarget.value })} onKeyDown={boxKey} /></label>}</For>
              <span class="muted small">{tool() === "t" ? "Enter places it" : "mm · Tab to the next, Enter places it, Esc goes back to the mouse"}</span>
            </div>
          </Show>
          <p class="muted small symhint">Points snap to ends, midpoints, centres, quadrants and crossings; a dashed guide shows a point held level, upright or in line with another; else the 0.5 mm grid. The wheel zooms, the middle button pans. Esc cancels; Ctrl+Z undoes.</p>
        </div>
        <div class="symprops form">
          <label>Name</label><div><input value={c().name} placeholder="Skid package, flame arrester…" onInput={e => setC({ ...c(), name: e.currentTarget.value })} /></div>
          <label>Start from</label><div><Pick value="" set={v => v && fromSym(v)} options={[["", "— a copy of a symbol… —"], ...(lib()?.symbols ?? []).filter(x => x.key !== "unknown").map(x => [x.key, `${x.cat_name} · ${x.name}`] as [string, string])]} /></div>
          <label>Stands for</label><div><Pick value={c().like} set={v => { const x = lib()?.symbols.find(s => s.key === v); setC({ ...c(), like: v, class: v ? (lib()?.classes.find(k => k.symbols.includes(v))?.name ?? c().class) : c().class, cat: (x?.cat as Cat) ?? c().cat, prefix: v ? x?.prefix ?? "" : c().prefix }); }} options={likeOptions()} /></div>
          <Show when={!c().like}>
            <label>Kind of object</label><div><Pick value={c().class} set={v => setC({ ...c(), class: v })} options={[["UnknownItem", "— choose: what it is in the lists —"], ...classes()]} /></div>
            <label>Category</label><div><Pick value={c().cat} set={v => setC({ ...c(), cat: v as Cat })} options={CAT_NAMES} /></div>
            <label>Tag prefix</label><div><input class="mono" value={c().prefix} placeholder="X, PK, FA…" onInput={e => setC({ ...c(), prefix: e.currentTarget.value.toUpperCase().slice(0, 6) })} /></div>
          </Show>
          <label>Points</label><div><button class="btn sm" onClick={() => { const [w, h] = wh(); portEditor({ ...c(), draw: draw(), w, h }, svg(), ports => setC({ ...c(), ports })); }}>{c().ports.length} · Connection points…</button></div>
          <label /><div><Check label="Goes into a line (two process connection points)" value={c().inline} set={v => setC({ ...c(), inline: v })} /></div>
          <p class="muted small">{(() => { const [w, h] = wh(); return `${w} × ${h} mm, centred on the cross; ${ents().length} shapes. A grid square is 1 mm.`; })()}</p>
        </div>
      </div>
    </Dialog>
  );
};

/** The review of a library: what each symbol stands for, its description and points. */
const Editor = (p: { view: LibView; inProject?: boolean; onComputer?: boolean; back: () => void }) => {
  const [l, setL] = createSignal<SymLibrary>(structuredClone(p.view.library));
  const [svgs, setSvgs] = createSignal<string[]>([...p.view.svgs]);
  const [inProject, setInProject] = createSignal(p.inProject ?? (!p.view.file && !!proj() && !p.onComputer));
  const [onComputer, setOnComputer] = createSignal(p.onComputer ?? (!!p.view.file || !proj()));
  const setSym = (i: number, c: CustomSym) => setL({ ...l(), symbols: l().symbols.map((s, j) => (j === i ? c : s)) });
  const restyle = async (i: number, like: string) => { const c = await call<CustomSym>("symbol_like", { sym: l().symbols[i], like }); if (c) setSym(i, c); };
  const remove = (i: number) => { setL({ ...l(), symbols: l().symbols.filter((_, j) => j !== i) }); setSvgs(svgs().filter((_, j) => j !== i)); };
  const addSym = (c: CustomSym) => invoke<string>("symbol_svg", { draw: c.draw }).then(sv => { setL({ ...l(), symbols: [...l().symbols, c] }); setSvgs([...svgs(), sv]); });
  const editSym = (i: number) => symbolEditor(l().symbols[i], c => invoke<string>("symbol_svg", { draw: c.draw }).then(sv => { setSym(i, c); setSvgs(svgs().map((x, j) => (j === i ? sv : x))); }));
  const unmatched = createMemo(() => l().symbols.filter(s => !s.like && s.class === "UnknownItem").length);
  const save = async () => {
    if (!inProject() && !onComputer()) { toast("Tick where to keep it: this project, all your projects, or both."); return; }
    if (!l().symbols.length) { toast("The library has no symbols yet: add one (New symbol…) or import a legend."); return; }
    const v = await call<LibView>("library_validate", { lib: l() });
    if (!v) return;
    const kept = v.library;
    if (onComputer()) { const w = await call<LibView>("library_save", { lib: kept, old: p.view.file || null }); if (!w) return; }
    else if (p.view.file) await call("library_delete", { file: p.view.file });
    if (proj()) {
      const st = proj()!.settings, others = (st.libraries ?? []).filter(x => x.id !== kept.id);
      if ((inProject() || (st.libraries ?? []).some(x => x.id === kept.id)) && !(await act({ SetSettings: { settings: { ...st, libraries: inProject() ? [...others, kept] : others } } }))) return;
    }
    await refreshLibs();
    toast(`Saved the library “${kept.name}” with ${kept.symbols.length} symbols${inProject() && onComputer() ? ", in this project and for all your projects" : inProject() ? ", in this project only" : ", for all your projects"}.${v.warnings.length ? " " + v.warnings.join(" ") : ""}`, true);
    p.back();
  };
  return (
    <div class="libedit">
      <div class="libhead">
        <label>Library name <input value={l().name} placeholder="Client A legend, Job 4711…" onInput={e => setL({ ...l(), name: e.currentTarget.value })} /></label>
        <span class="muted small">{l().source ? `From ${l().source} · ` : ""}{l().symbols.length} symbols{unmatched() ? ` · ${unmatched()} not matched yet` : ""}</span>
        <span class="grow" />
        <span class="libwhere"><label class="chk" title={proj() ? "Kept inside this project's file: it travels with the project and is used only here" : "Open a project to keep a library in it"}><input type="checkbox" checked={inProject()} disabled={!proj()} onChange={e => setInProject(e.currentTarget.checked)} /> This project only</label>
        <label class="chk" title="Kept on this computer, for every project"><input type="checkbox" checked={onComputer()} onChange={e => setOnComputer(e.currentTarget.checked)} /> All my projects</label></span>
        <button class="btn" onClick={p.back}>Cancel</button><button class="btn pri" onClick={save}>Save library</button>
      </div>
      <Show when={p.view.warnings.length}><p class="small warnline">{p.view.warnings.join(" ")}</p></Show>
      <p class="muted small">Check what each symbol stands for: it is then the same object as ours (class, tag prefix, data, checks and lists), only drawn the client's way. A symbol that is nothing of ours is a new kind: open it (Drawing…) to say what kind of object it is, its category and tag prefix. Connection points were suggested where lines meet the drawing; check them for anything that is not a simple valve.</p>
      <p class="libbtns"><button class="btn sm" onClick={() => symbolEditor(null, addSym)}>New symbol…</button></p>
      <div class="libscroll"><table class="tbl libtbl">
        <thead><tr><th /><th>Description</th><th>Block</th><th>Stands for</th><th>Points</th><th>In-line</th><th /><th /></tr></thead>
        <tbody><For each={l().symbols}>{(s, i) => <tr classList={{ warn: !s.like && s.class === "UnknownItem" }}>
          <td><LibPic svg={svgs()[i()]} w={s.w} h={s.h} size={38} /></td>
          <td><input value={s.name} onChange={e => setSym(i(), { ...s, name: e.currentTarget.value })} /></td>
          <td class="mono small">{s.block}</td>
          <td><Pick value={s.like} set={v => restyle(i(), v)} options={likeOptions()} /><Show when={!s.like && s.class !== "UnknownItem"}><div class="muted small">A new kind: {lib()?.classes.find(k => k.name === s.class)?.label ?? s.class}{s.prefix ? ` · ${s.prefix}` : ""}</div></Show></td>
          <td><button class="btn sm" onClick={() => portEditor(s, svgs()[i()], ports => setSym(i(), { ...s, ports, inline: s.inline && ports.filter(q => !q.signal).length === 2 }))}>{s.ports.length} · Edit…</button></td>
          <td class="c"><input type="checkbox" checked={s.inline} disabled={s.ports.filter(q => !q.signal).length !== 2} title="Goes into a line (two process connection points)" onChange={e => setSym(i(), { ...s, inline: e.currentTarget.checked })} /></td>
          <td><button class="btn sm" title="Change its drawing, or what kind of object it is" onClick={() => editSym(i())}>Drawing…</button></td>
          <td><button class="x" title="Leave this symbol out" onClick={() => remove(i())}>×</button></td>
        </tr>}</For></tbody>
      </table></div>
    </div>
  );
};

/** File › Libraries: every library, on this computer and in this project, each named as you like,
 *  with tick boxes for where it is kept and whether this project's Symbols panel shows it. */
export const LibrariesPage = () => {
  const [editing, setEditing] = createSignal<{ v: LibView; inProject?: boolean; onComputer?: boolean } | null>(null);
  const [codesEdit, setCodesEdit] = createSignal<{ list: CodeList; warnings: string[]; file: string; inProject: boolean; onComputer: boolean } | null>(null);
  const [savedCodes, setSavedCodes] = createSignal<[string, CodeList][]>([]);
  const reloadCodes = () => invoke<[string, CodeList][]>("codes_saved").then(setSavedCodes, () => {});
  reloadCodes();
  const scan = async () => { const v = await call<LibView | null>("legend_scan"); if (v) setEditing({ v }); };
  const add = async () => { const v = await call<LibView | null>("library_import"); if (v) { await refreshLibs(); toast(`Added the library “${v.library.name}”.`, true); } };
  const fresh = () => setEditing({ v: { file: "", library: { format: 1, name: "", id: "", source: "", symbols: [] }, svgs: [], warnings: [] } });
  const idOf = (e: LibEntry) => e.library.id || e.library.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  const setSt = (patch: Partial<Settings>) => act({ SetSettings: { settings: { ...proj()!.settings, ...patch } } });
  const toProject = async (e: LibEntry, on: boolean) => {
    const st = proj()!.settings, others = (st.libraries ?? []).filter(x => x.id !== idOf(e));
    if (on) { const v = await call<LibView>("library_validate", { lib: e.library }); if (v) await setSt({ libraries: [...others, v.library] }); }
    else if (!e.onComputer) D.confirm(`Take the library “${e.library.name}” out of this project? It is kept nowhere else: tick All my projects first to keep it. Symbols already drawn keep their own copies.`, "Take it out", () => setSt({ libraries: others }));
    else await setSt({ libraries: others });
  };
  const toComputer = async (e: LibEntry, on: boolean) => {
    if (on) { const w = await call<LibView>("library_save", { lib: e.library, old: null }); if (w) { await refreshLibs(); toast(`“${e.library.name}” is now kept for all your projects.`, true); } }
    else D.confirm(`Delete the library “${e.library.name}” from this computer?${e.inProject ? " This project keeps its own copy." : " Projects that use its symbols keep their own copies."}`, "Delete", async () => { await call("library_delete", { file: e.file }); await refreshLibs(); });
  };
  const rename = async (e: LibEntry, name: string) => {
    name = name.trim(); if (!name || name === e.library.name) return;
    const lib2 = { ...e.library, id: idOf(e), name };
    if (e.onComputer) { const w = await call<LibView>("library_save", { lib: lib2, old: e.file }); if (!w) return; }
    if (e.inProject && proj()) { const v = await call<LibView>("library_validate", { lib: lib2 }); if (v) await setSt({ libraries: [...(proj()!.settings.libraries ?? []).filter(x => x.id !== idOf(e)), v.library] }); }
    await refreshLibs(); toast(`Renamed to “${name}”: its symbols and the drawings that use them stay as they are.`, true);
  };
  const show = (e: LibEntry, on: boolean) => { const off = (proj()!.settings.libs_off ?? []).filter(x => x !== idOf(e)); setSt({ libs_off: on ? off : [...off, idOf(e)] }); };
  return (
    <div class="libraries">
      <Show when={!codesEdit()} fallback={<CodesEditor {...codesEdit()!} back={() => setCodesEdit(null)} saved={reloadCodes} />}>
      <Show when={editing()} fallback={<>
        <h1>Symbol libraries <Help k="libraries" /></h1>
        <p class="muted">A client's or company's own symbols: read from their legend sheet (DXF, DWG or PDF), or drawn here. Name each library as you like. Keep it in this project only (it travels with the project file), for all your projects (on this computer; send it to a colleague as a file), or both, and tick the ones this project's Symbols panel shows.</p>
        <p class="libbtns"><button class="btn pri" onClick={scan}>Import a legend (DXF, DWG or PDF)…</button><button class="btn" onClick={fresh}>New library…</button><button class="btn" onClick={add}>Add a library file…</button></p>
        <Show when={allLibs().length} fallback={<p class="muted">No libraries yet.</p>}>
          <div class="libcards"><For each={allLibs()}>{e => <div class="libcard">
            <div class="libpics"><For each={e.library.symbols.slice(0, 8)}>{(s, i) => <LibPic svg={e.svgs[i()] ?? ""} w={s.w} h={s.h} size={28} />}</For></div>
            <input class="libname" value={e.library.name} title="Rename it: its symbols and the drawings that use them stay as they are" onChange={ev => rename(e, ev.currentTarget.value)} />
            <span class="muted small">{e.library.symbols.length} symbols{e.library.source ? ` · from ${e.library.source}` : ""}</span>
            <div class="libwhere">
              <label class="chk" title={proj() ? "Kept inside this project's file" : "Open a project first"}><input type="checkbox" checked={e.inProject} disabled={!proj()} onChange={ev => toProject(e, ev.currentTarget.checked)} /> This project</label>
              <label class="chk" title="Kept on this computer, for every project"><input type="checkbox" checked={e.onComputer} onChange={ev => toComputer(e, ev.currentTarget.checked)} /> All my projects</label>
              <label class="chk" title="This project's Symbols panel shows it"><input type="checkbox" checked={e.shown} disabled={!proj()} onChange={ev => show(e, ev.currentTarget.checked)} /> Shown in the Symbols panel</label>
            </div>
            <div class="libcbtns">
              <button class="btn sm" onClick={() => setEditing({ v: { file: e.file, library: e.library, svgs: e.svgs, warnings: [] }, inProject: e.inProject, onComputer: e.onComputer })}>Review…</button>
              <Show when={e.onComputer}><button class="btn sm" onClick={async () => { const f = await call<string | null>("library_export", { file: e.file }); if (f) toast(`Saved ${f}.`, true); }}>Save a copy…</button></Show>
            </div>
          </div>}</For></div>
        </Show>
        <CodesSection edit={setCodesEdit} saved={savedCodes} reload={reloadCodes} />
      </>}>{x => <Editor view={x().v} inProject={x().inProject} onComputer={x().onComputer} back={() => setEditing(null)} />}</Show>
      </Show>
    </div>
  );
};

const CODE_KINDS: [string, string][] = [["fluid", "Fluid and service codes"], ["class", "Pipe classes"], ["insulation", "Insulation and tracing codes"], ["equipment", "Equipment tag prefixes"], ["instrument", "Instrument letters"], ["other", "Other abbreviations"]];
const idOfList = (l: CodeList) => l.id || l.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

/** A list of codes to check and correct: kind, code and meaning per row; where it is kept. */
const CodesEditor = (p: { list: CodeList; warnings: string[]; file: string; inProject: boolean; onComputer: boolean; back: () => void; saved: () => void }) => {
  const [l, setL] = createSignal<CodeList>(structuredClone(p.list));
  const [inProject, setInProject] = createSignal(p.inProject);
  const [onComputer, setOnComputer] = createSignal(p.onComputer);
  const setRow = (i: number, c: Code) => setL({ ...l(), codes: l().codes.map((x, j) => (j === i ? c : x)) });
  const csv = () => ["Kind,Code,Meaning", ...l().codes.map(c => [CODE_KINDS.find(k => k[0] === c.kind)?.[1] ?? c.kind, c.code, c.meaning].map(x => /[",;\n]/.test(x) ? `"${x.replace(/"/g, '""')}"` : x).join(","))].join("\r\n");
  const save = async () => {
    if (!inProject() && !onComputer()) { toast("Tick where to keep it: this project, all your projects, or both."); return; }
    const list = { ...l(), id: idOfList(l()) };
    if (onComputer()) { if (!(await call<string>("codes_save", { list }))) return; }
    else if (p.file) await call("codes_delete", { file: p.file });
    if (proj()) {
      const st = proj()!.settings, others = (st.codes ?? []).filter(x => x.id !== list.id);
      if ((inProject() || (st.codes ?? []).some(x => x.id === list.id)) && !(await act({ SetSettings: { settings: { ...st, codes: inProject() ? [...others, list] : others } } }))) return;
    }
    p.saved();
    toast(`Saved “${list.name}”: ${list.codes.length} codes${inProject() ? "; this project's drawing is checked against them (check 28) and its legend sheet lists the ones it uses" : ""}.`, true);
    p.back();
  };
  return (
    <div class="libedit">
      <div class="libhead">
        <label>Name <input value={l().name} placeholder="Client A codes, Job 4711…" onInput={e => setL({ ...l(), name: e.currentTarget.value })} /></label>
        <span class="muted small">{l().source ? `From ${l().source} · ` : ""}{l().codes.length} codes</span>
        <span class="grow" />
        <span class="libwhere"><label class="chk"><input type="checkbox" checked={inProject()} disabled={!proj()} onChange={e => setInProject(e.currentTarget.checked)} /> This project</label>
        <label class="chk"><input type="checkbox" checked={onComputer()} onChange={e => setOnComputer(e.currentTarget.checked)} /> All my projects</label></span>
        <button class="btn" onClick={async () => { const f = await call<string | null>("save_file", { name: `${l().name || "Codes"}.csv`, data: csv() }); if (f) toast(`Saved ${f}: open it in Excel, change it, and import it again.`, true); }}>Save as CSV…</button>
        <button class="btn" onClick={p.back}>Cancel</button><button class="btn pri" onClick={save}>Save codes</button>
      </div>
      <Show when={p.warnings.length}><p class="small warnline">{p.warnings.join(" ")}</p></Show>
      <p class="muted small">Check each code: its kind (from the table's heading), the code and its meaning. Fluid codes, pipe classes and insulation codes are checked on every line number, equipment prefixes and instrument first letters on every tag; the Inspector suggests them as you type.</p>
      <p class="libbtns"><button class="btn sm" onClick={() => setL({ ...l(), codes: [...l().codes, { kind: "fluid", code: "", meaning: "" }] })}>Add a code</button></p>
      <div class="libscroll"><table class="tbl libtbl codetbl">
        <thead><tr><th>Kind</th><th>Code</th><th>Meaning</th><th /></tr></thead>
        <tbody><For each={l().codes}>{(c, i) => <tr>
          <td><Pick value={c.kind} set={v => setRow(i(), { ...c, kind: v })} options={CODE_KINDS} /></td>
          <td><input class="mono" value={c.code} onChange={e => setRow(i(), { ...c, code: e.currentTarget.value })} /></td>
          <td><input value={c.meaning} onChange={e => setRow(i(), { ...c, meaning: e.currentTarget.value })} /></td>
          <td><button class="x" title="Remove this code" onClick={() => setL({ ...l(), codes: l().codes.filter((_, j) => j !== i()) })}>×</button></td>
        </tr>}</For></tbody>
      </table></div>
    </div>
  );
};

/** File › Libraries › Abbreviations and codes: lists named as the user likes, in this project, on
 *  this computer, or both. */
export const CodesSection = (p: { edit: (x: { list: CodeList; warnings: string[]; file: string; inProject: boolean; onComputer: boolean }) => void; saved: () => [string, CodeList][] ; reload: () => void }) => {
  const entries = createMemo(() => {
    const v: { list: CodeList; file: string; inProject: boolean; onComputer: boolean }[] = p.saved().map(([file, list]) => ({ list, file, inProject: false, onComputer: true }));
    for (const l of proj()?.settings.codes ?? []) { const e = v.find(x => idOfList(x.list) === idOfList(l)); if (e) { e.inProject = true; e.list = l; } else v.push({ list: l, file: "", inProject: true, onComputer: false }); }
    return v.sort((a, b) => a.list.name.localeCompare(b.list.name));
  });
  const read = async () => { const r = await call<[CodeList, string[]] | null>("codes_read"); if (r) p.edit({ list: r[0], warnings: r[1], file: "", inProject: !!proj(), onComputer: !proj() }); };
  const toProject = async (e: { list: CodeList; onComputer: boolean }, on: boolean) => {
    const st = proj()!.settings, others = (st.codes ?? []).filter(x => x.id !== idOfList(e.list));
    if (on || e.onComputer) await act({ SetSettings: { settings: { ...st, codes: on ? [...others, { ...e.list, id: idOfList(e.list) }] : others } } });
    else D.confirm(`Take “${e.list.name}” out of this project? It is kept nowhere else: tick All my projects first to keep it.`, "Take it out", () => act({ SetSettings: { settings: { ...st, codes: others } } }));
  };
  const toComputer = async (e: { list: CodeList; file: string; inProject: boolean }, on: boolean) => {
    if (on) { if (await call<string>("codes_save", { list: { ...e.list, id: idOfList(e.list) } })) p.reload(); }
    else D.confirm(`Delete “${e.list.name}” from this computer?${e.inProject ? " This project keeps its own copy." : ""}`, "Delete", async () => { await call("codes_delete", { file: e.file }); p.reload(); });
  };
  const count = (l: CodeList) => CODE_KINDS.map(([k, n]) => [n, l.codes.filter(c => c.kind === k).length] as [string, number]).filter(x => x[1]).map(([n, c]) => `${c} ${n.toLowerCase()}`).join(" · ");
  return (
    <section class="codesec">
      <h2>Abbreviations and codes <Help k="codes" /></h2>
      <p class="muted">A company's or client's fluid and service codes, pipe classes, insulation codes, equipment prefixes and instrument letters: read from the table on their legend sheet (DXF, DWG or PDF) or from Excel or CSV, checked here, named as you like. A project's drawing is checked against the lists it keeps, and its legend sheet lists the codes it uses.</p>
      <p class="libbtns"><button class="btn pri" onClick={read}>Import codes (DXF, DWG, PDF, Excel or CSV)…</button><button class="btn" onClick={() => p.edit({ list: { name: "", id: "", source: "", codes: [] }, warnings: [], file: "", inProject: !!proj(), onComputer: !proj() })}>New list…</button></p>
      <Show when={entries().length} fallback={<p class="muted">No code lists yet.</p>}>
        <div class="libcards"><For each={entries()}>{e => <div class="libcard codecard">
          <b>{e.list.name}</b><span class="muted small">{count(e.list) || "no codes"}</span>
          <div class="libwhere">
            <label class="chk" title="This project's drawing follows it"><input type="checkbox" checked={e.inProject} disabled={!proj()} onChange={ev => toProject(e, ev.currentTarget.checked)} /> This project</label>
            <label class="chk" title="Kept on this computer, for every project"><input type="checkbox" checked={e.onComputer} onChange={ev => toComputer(e, ev.currentTarget.checked)} /> All my projects</label>
          </div>
          <div class="libcbtns"><button class="btn sm" onClick={() => p.edit({ list: e.list, warnings: [], file: e.file, inProject: e.inProject, onComputer: e.onComputer })}>Review…</button></div>
        </div>}</For></div>
      </Show>
    </section>
  );
};

/** Options › Drawing standard: draw this project with a library's symbols. */
export const StandardSec = () => {
  const st = () => proj()!.settings.standard;
  const [pick, setPick] = createSignal(st().library);
  const [sets, setSets] = createSignal<LibView[]>([]);
  invoke<LibView[]>("builtin_sets").then(setSets, () => {});
  const all = () => [...sets(), ...allLibs().map(e => ({ file: e.file, library: e.library, svgs: e.svgs, warnings: [] }) as LibView)];
  const count = () => Object.keys(st().map).length;
  const apply = () => {
    const v = all().find(x => x.library.name === pick());
    const map: Record<string, string> = {};
    for (const s of v?.library.symbols ?? []) if (s.like && !map[s.like]) map[s.like] = s.key;
    act({ SetSettings: { settings: { ...proj()!.settings, standard: { library: v ? v.library.name : "", map } } } }).then(o => o && toast(v ? `Drawn to ${v.library.name}: ${Object.keys(map).length} of our symbols use its drawings.` : "Drawn with Datum Schema's symbols again.", true));
  };
  return (
    <div class="stdsec">
      <div class="form">
        <label>Draw with</label>
        <div class="inline"><Pick value={pick()} set={setPick} options={[["", "Datum Schema's symbols"], ...sets().map(v => [v.library.name, `${v.library.name} (built in)`] as [string, string]), ...allLibs().map(v => [v.library.name, `${v.library.name}${v.inProject && !v.onComputer ? " (this project)" : ""}`] as [string, string]), ...(st().library && !all().some(v => v.library.name === st().library) ? [[st().library, `${st().library} (in this project)`] as [string, string]] : [])]} />
          <button class="btn sm pri" onClick={apply} disabled={pick() === st().library && !!st().library}>Apply</button></div>
      </div>
      <p class="muted small">{st().library ? `Now drawn to ${st().library}: ${count()} of our symbols use its drawings. Objects, tags, data, checks and lists stay the same; only the drawing changes.` : "Now drawn with Datum Schema's own symbols."}</p>
    </div>
  );
};
