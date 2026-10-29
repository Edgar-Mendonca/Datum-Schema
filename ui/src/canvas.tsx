// The drawing canvas, built from Datum Sim's block canvas: the sheet on a whiteboard, pan and zoom,
// snap, marquee and multi-select, moving symbols, drawing lines port to port (or onto a line, or to
// an open end), dragging a line's segments, dropping symbols from the palette (an in-line symbol
// dropped on a line goes into it). Everything drawn comes from the engine (view.rs): this file
// draws it and sends edits.
import { For, Show, JSX, createSignal, createMemo, createEffect, onMount, onCleanup, batch, on } from "solid-js";
import { Pt, Prim, ItemView, LineView, End, Kind, Item, Line, Note } from "./api";
import { derived, sel, setSel, act, proj, sheet, opts, tool, setTool, symOf, lib, openMenu, MenuItem, hiNumber, hidden, setSheetId, lineOf, onFit } from "./store";
import { C } from "./cmds";
import { helpCard } from "./help";

export const [zoom, setZoom] = createSignal(2);
export const [pan, setPan] = createSignal<Pt>([20, 20]);
let svgEl: SVGSVGElement | undefined;
export const svgNode = () => svgEl;

const grid = () => proj()?.settings.grid || 2.5;
const snap = (v: number) => Math.round(v / grid()) * grid();
export const toSheet = (cx: number, cy: number): Pt => {
  const r = svgEl!.getBoundingClientRect();
  return [(cx - r.left - pan()[0]) / zoom(), (cy - r.top - pan()[1]) / zoom()];
};
/** Sheet millimetres to window pixels (the UI tests use it to aim the mouse). */
export const toScreen = (x: number, y: number): Pt => {
  const r = svgEl!.getBoundingClientRect();
  return [r.left + pan()[0] + x * zoom(), r.top + pan()[1] + y * zoom()];
};
(window as any).__toScreen = (x: number, y: number) => (svgEl ? toScreen(x, y) : null);
export const viewCenter = (): Pt => {
  if (!svgEl) return [150, 100];
  const r = svgEl.getBoundingClientRect();
  return toSheet(r.left + r.width / 2, r.top + r.height / 2);
};
const fitBox = (x0: number, y0: number, x1: number, y1: number) => {
  if (!svgEl) return;
  const r = svgEl.getBoundingClientRect();
  if (r.width < 10) return;
  const z = Math.min(40, r.width / (x1 - x0), r.height / (y1 - y0));
  batch(() => { setZoom(z); setPan([(r.width - (x1 + x0) * z) / 2, (r.height - (y1 + y0) * z) / 2]); });
};
/** Fit the whole sheet in the window. */
export const fitSheet = () => { const v = derived()?.view; if (v) fitBox(-10, -10, v.w + 10, v.h + 10); };
/** Fit the drawing (everything on the sheet) in the window; a blank sheet fits whole. */
export const fit = () => {
  const v = derived()?.view; if (!v) return;
  const xs: number[] = [], ys: number[] = [];
  for (const i of v.items) { xs.push(i.bbox[0], i.bbox[2]); ys.push(i.bbox[1], i.bbox[3] + 6); }
  for (const l of v.lines) for (const q of l.path) { xs.push(q[0]); ys.push(q[1]); }
  for (const nt of v.notes) { xs.push(nt.bbox[0], nt.bbox[2]); ys.push(nt.bbox[1], nt.bbox[3]); }
  if (!xs.length || v.legend) return fitSheet();
  const pad = 12;
  fitBox(Math.min(...xs) - pad, Math.min(...ys) - pad, Math.max(...xs) + pad, Math.max(...ys) + pad);
};
/** Centre the window on a point of the sheet (zooming in if it is far out). */
export const centreOn = (p: Pt, z?: number) => {
  if (!svgEl) return;
  const r = svgEl.getBoundingClientRect();
  const zz = z ?? Math.max(zoom(), 3);
  batch(() => { setZoom(zz); setPan([r.width / 2 - p[0] * zz, r.height / 2 - p[1] * zz]); });
};
export const zoomBy = (f: number, at?: Pt) => {
  const r = svgEl?.getBoundingClientRect(); if (!r) return;
  const [cx, cy] = at ?? [r.width / 2, r.height / 2];
  const z0 = zoom(), z = Math.min(40, Math.max(0.3, z0 * f));
  batch(() => { setPan([cx - (cx - pan()[0]) * z / z0, cy - (cy - pan()[1]) * z / z0]); setZoom(z); });
};

// ---------- primitives ----------
const n = (v: number) => Math.round(v * 1000) / 1000;
const arcD = (c: Pt, r: number, a0: number, a1: number) => {
  const p = (a: number) => [c[0] + r * Math.cos(a * Math.PI / 180), c[1] + r * Math.sin(a * Math.PI / 180)];
  const [x0, y0] = p(a0), [x1, y1] = p(a1);
  const large = ((a1 - a0) % 360 + 360) % 360 > 180 ? 1 : 0;
  return `M${n(x0)} ${n(y0)}A${n(r)} ${n(r)} 0 ${large} 1 ${n(x1)} ${n(y1)}`;
};
/** One engine primitive as SVG. Text is always text (never markup). */
export const PrimEl = (p: { p: Prim; cls?: string }): JSX.Element => {
  const x = p.p;
  switch (x.k) {
    case "poly": {
      const pts = x.pts.map(q => `${n(q[0])},${n(q[1])}`).join(" ");
      return x.closed ? <polygon points={pts} class={p.cls} classList={{ pf: x.fill }} stroke-width={x.w} stroke-dasharray={x.dash ? "1.2 0.8" : undefined} />
        : <polyline points={pts} class={p.cls} stroke-width={x.w} stroke-dasharray={x.dash ? "1.2 0.8" : undefined} />;
    }
    case "circle": return <circle cx={x.c[0]} cy={x.c[1]} r={x.r} class={p.cls} classList={{ pf: x.fill }} stroke-width={x.w} />;
    case "arc": return <path d={arcD(x.c, x.r, x.a0, x.a1)} class={p.cls} stroke-width={x.w} />;
    case "text": return <text x={x.p[0]} y={x.p[1]} font-size={String(x.h)} class="pt" classList={{ b: x.bold }} text-anchor={(["start", "middle", "end"] as const)[x.anchor] ?? "middle"} dominant-baseline="central">{x.s}</text>;
  }
};
const pathD = (pts: Pt[]) => pts.map((p, i) => `${i ? "L" : "M"}${n(p[0])} ${n(p[1])}`).join("");
export const itemTransform = (it: { x: number; y: number; rot: number; flip: boolean; scale: number }) => `translate(${it.x} ${it.y}) rotate(${it.rot * 90}) scale(${it.flip ? -it.scale : it.scale} ${it.scale})`;

/** The symbol sprite: one <g id="s-key"> per symbol, used by every placed item. */
export const SymbolDefs = () => (
  <defs>
    <For each={lib()?.symbols ?? []}>{s => <g id={`s-${s.key}`} innerHTML={s.svg} />}</For>
    <marker id="ldr" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" /></marker>
  </defs>
);

// ---------- geometry for picking ----------
const nearestSeg = (a: Pt, b: Pt, p: Pt): Pt => {
  const dx = b[0] - a[0], dy = b[1] - a[1], l = dx * dx + dy * dy;
  const t = l ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l)) : 0;
  return [a[0] + t * dx, a[1] + t * dy];
};
const distTo = (path: Pt[], p: Pt) => {
  let best = Infinity, at: Pt = path[0] ?? p, seg = 0;
  for (let i = 0; i + 1 < path.length; i++) { const q = nearestSeg(path[i], path[i + 1], p), d = Math.hypot(q[0] - p[0], q[1] - p[1]); if (d < best) { best = d; at = q; seg = i; } }
  return { d: best, at, seg };
};
/** The line under a point (within a few pixels). */
const lineAt = (p: Pt, lines: LineView[], tol: number) => {
  let best: { l: LineView; at: Pt; seg: number } | null = null, bd = tol;
  for (const l of lines) { const r = distTo(l.path, p); if (r.d < bd) { bd = r.d; best = { l, at: r.at, seg: r.seg }; } }
  return best;
};
/** The path with segment i moved sideways, its neighbours stretched to meet it (Sim's wire drag). */
const movedPath = (path: Pt[], i: number, dx: number, dy: number): Pt[] | null => {
  if (i < 1 || i > path.length - 3) return null;
  const a = path[i], b = path[i + 1], horiz = Math.abs(a[1] - b[1]) < 1e-9;
  const out = path.map(q => [q[0], q[1]] as Pt);
  if (horiz) { const y = snap(a[1] + dy); out[i][1] = y; out[i + 1][1] = y; }
  else { const x = snap(a[0] + dx); out[i][0] = x; out[i + 1][0] = x; }
  return out;
};

const BADGE: Record<string, string> = { "✓": "ok", "!": "warn", "✗": "bad" };

/** The line number of the one selected line: highlighted wherever it runs, on every sheet. */
const selNumber = createMemo(() => { const s = sel(); return s.length === 1 ? derived()?.numbers[s[0]] ?? "" : ""; });

// ---------- the canvas ----------
type Drag =
  | { k: "move"; ids: number[]; a: Pt; dx: number; dy: number }
  | { k: "wire"; from: End; a: Pt; b: Pt; kind: Kind }
  | { k: "box"; a: Pt; b: Pt; add: boolean }
  | { k: "pan"; cx: number; cy: number; p0: Pt }
  | { k: "bend"; id: number; a: Pt; b: Pt; seg: number }
  | { k: "end"; id: number; which: "a" | "b"; b: Pt };

/** The ids the issues point at, by level. */
const issueSets = createMemo(() => {
  const err = new Set<number>(), warn = new Set<number>();
  for (const i of derived()?.issues ?? []) for (const id of i.ids) (i.level === "error" ? err : warn).add(id);
  return { err, warn };
});

export const Canvas = () => {
  const [drag, setDrag] = createSignal<Drag | null>(null);
  const [space, setSpace] = createSignal(false);
  const [hover, setHover] = createSignal<number | null>(null);
  const [mouse, setMouse] = createSignal<Pt | null>(null);
  const v = () => derived()?.view;
  const selSet = createMemo(() => new Set(sel()));
  const vis = (layer: string) => !hidden().includes(layer);
  const items = createMemo(() => (v()?.items ?? []).filter(i => vis(i.layer)));
  const lines = createMemo(() => (v()?.lines ?? []).filter(l => vis(l.layer)));
  const notes = createMemo(() => (vis("notes") ? v()?.notes ?? [] : []));
  const itemAt = (p: Pt) => [...items()].reverse().find(i => p[0] >= i.bbox[0] - 0.8 && p[0] <= i.bbox[2] + 0.8 && p[1] >= i.bbox[1] - 0.8 && p[1] <= i.bbox[3] + 0.8);
  const noteAt = (p: Pt) => [...notes()].reverse().find(i => p[0] >= i.bbox[0] && p[0] <= i.bbox[2] && p[1] >= i.bbox[1] && p[1] <= i.bbox[3]);
  /** The port nearest a point (within a few pixels), on any item. */
  const portAt = (p: Pt): { it: ItemView; k: number } | null => {
    let best: { it: ItemView; k: number } | null = null, bd = Math.max(1.6, 7 / zoom());
    for (const it of items()) it.ports.forEach((q, k) => { const d = Math.hypot(q.p[0] - p[0], q.p[1] - p[1]); if (d < bd) { bd = d; best = { it, k }; } });
    return best;
  };
  const tol = () => Math.max(1.2, 5 / zoom());
  const endAt = (p: Pt): End => {
    const po = portAt(p);
    if (po) return { at: "port", item: po.it.id, port: po.k };
    const ln = lineAt(p, lines(), tol());
    if (ln) return { at: "line", line: ln.l.id, x: ln.at[0], y: ln.at[1] };
    return { at: "free", x: snap(p[0]), y: snap(p[1]) };
  };
  const endPt = (e: End): Pt => {
    if (e.at === "port") { const it = v()?.items.find(i => i.id === e.item); return it?.ports[e.port]?.p ?? [0, 0]; }
    return [e.x, e.y];
  };
  const kindFor = (from: End): Kind => {
    const t = tool();
    if (t.k === "line") return t.kind;
    if (from.at === "port") { const it = v()?.items.find(i => i.id === from.item); if (it?.ports[from.port]?.signal) return "electric"; }
    return "major";
  };

  const down = (e: MouseEvent) => {
    if (!v()) return;
    (e.currentTarget as HTMLElement).focus?.();
    const p = toSheet(e.clientX, e.clientY);
    if (e.button === 1 || space()) { e.preventDefault(); setDrag({ k: "pan", cx: e.clientX, cy: e.clientY, p0: pan() }); return; }
    if (e.button !== 0) return;
    const t = tool();
    const add = e.shiftKey || e.ctrlKey || e.metaKey;
    if (t.k === "place") { placeAt(t.sym, p, t.tag, t.obj); if (!e.shiftKey || t.obj) setTool({ k: "select" }); return; }
    if (t.k === "note") { C.addNote(t.kind, [snap(p[0]), snap(p[1])]); setTool({ k: "select" }); return; }
    // Ports start lines (the line tool also starts them anywhere).
    const po = portAt(p);
    if (po && (t.k === "line" || !selSet().has(po.it.id) || e.altKey || hover() === po.it.id)) {
      const from: End = { at: "port", item: po.it.id, port: po.k };
      setDrag({ k: "wire", from, a: po.it.ports[po.k].p, b: p, kind: kindFor(from) }); return;
    }
    if (t.k === "line") {
      const from = endAt(p);
      setDrag({ k: "wire", from, a: endPt(from), b: p, kind: t.kind }); return;
    }
    const it = itemAt(p);
    if (it) {
      let ids = sel();
      if (add) ids = selSet().has(it.id) ? ids.filter(x => x !== it.id) : [...ids, it.id];
      else if (!selSet().has(it.id)) ids = [it.id];
      setSel(ids);
      if (ids.includes(it.id)) setDrag({ k: "move", ids, a: p, dx: 0, dy: 0 });
      return;
    }
    const nt = noteAt(p);
    if (nt) {
      const ids = add ? [...sel(), nt.id] : selSet().has(nt.id) ? sel() : [nt.id];
      setSel(ids); setDrag({ k: "move", ids, a: p, dx: 0, dy: 0 }); return;
    }
    const ln = lineAt(p, lines(), tol());
    if (ln) {
      const id = ln.l.id;
      // Dragging a selected line's open end moves the end.
      const l = lineOf(id);
      if (l && selSet().has(id)) for (const w of ["a", "b"] as const) { const e2 = l[w]; if (e2.at !== "port" && Math.hypot(e2.x - p[0], e2.y - p[1]) < tol() * 1.5) { setDrag({ k: "end", id, which: w, b: p }); return; } }
      setSel(add ? (selSet().has(id) ? sel().filter(x => x !== id) : [...sel(), id]) : [id]);
      setDrag({ k: "bend", id, a: p, b: p, seg: ln.seg });
      return;
    }
    if (!add) setSel([]);
    setDrag({ k: "box", a: p, b: p, add });
  };

  const move = (e: MouseEvent) => {
    if (svgEl) { const r = svgEl.getBoundingClientRect(); if (e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) setMouse(toSheet(e.clientX, e.clientY)); }
    const d = drag(); if (!d) return;
    const p = svgEl ? toSheet(e.clientX, e.clientY) : [0, 0] as Pt;
    if (d.k === "pan") setPan([d.p0[0] + e.clientX - d.cx, d.p0[1] + e.clientY - d.cy]);
    else if (d.k === "move") setDrag({ ...d, dx: snap(p[0] - d.a[0]), dy: snap(p[1] - d.a[1]) });
    else if (d.k === "wire" || d.k === "box" || d.k === "bend" || d.k === "end") setDrag({ ...d, b: p } as Drag);
  };

  const up = async (e: MouseEvent) => {
    const d = drag(); setDrag(null); if (!d) return;
    const p = svgEl ? toSheet(e.clientX, e.clientY) : [0, 0] as Pt;
    if (d.k === "move" && (d.dx || d.dy)) act({ Move: { ids: d.ids, dx: d.dx, dy: d.dy } });
    else if (d.k === "wire") {
      if (Math.hypot(p[0] - d.a[0], p[1] - d.a[1]) < 1.5) return;
      const to = endAt(p);
      if (JSON.stringify(to) === JSON.stringify(d.from)) return;
      const s = sheet(); if (!s) return;
      const o = await act({ AddLine: { line: { id: 0, sheet: s.id, kind: d.kind, a: d.from, b: to, pts: [], flip: false, obj: 0 } as Line } });
      if (o?.created.length) setSel([o.created[0]]);
    } else if (d.k === "bend") {
      if (Math.hypot(d.b[0] - d.a[0], d.b[1] - d.a[1]) > 4 / zoom()) {
        const lv = v()?.lines.find(x => x.id === d.id); if (!lv) return;
        const moved = movedPath(lv.path, d.seg, d.b[0] - d.a[0], d.b[1] - d.a[1]);
        const l = lineOf(d.id);
        act({ SetPts: { id: d.id, pts: moved ? moved.slice(1, -1) : [...(l?.pts ?? []), [snap(d.b[0]), snap(d.b[1])]] } });
      }
    } else if (d.k === "end") {
      const l = lineOf(d.id); if (!l) return;
      act({ SetLine: { line: { ...l, [d.which]: endAt(p) } } });
    } else if (d.k === "box") {
      const [x0, x1] = [Math.min(d.a[0], d.b[0]), Math.max(d.a[0], d.b[0])], [y0, y1] = [Math.min(d.a[1], d.b[1]), Math.max(d.a[1], d.b[1])];
      if (x1 - x0 < 1 && y1 - y0 < 1) return;
      const inside = (b: number[]) => b[0] >= x0 && b[2] <= x1 && b[1] >= y0 && b[3] <= y1;
      const ids = [...items().filter(i => inside(i.bbox)).map(i => i.id), ...notes().filter(i => inside(i.bbox)).map(i => i.id),
        ...lines().filter(l => l.path.every(q => q[0] >= x0 && q[0] <= x1 && q[1] >= y0 && q[1] <= y1)).map(l => l.id)];
      setSel(d.add ? [...new Set([...sel(), ...ids])] : ids);
    }
  };

  const dbl = (e: MouseEvent) => {
    const p = toSheet(e.clientX, e.clientY);
    const it = itemAt(p);
    if (it?.cat === "connector") { C.goPartner(it.id); return; }
    if (it) { setSel([it.id]); setTimeout(() => document.querySelector<HTMLInputElement>(".insp input")?.focus(), 30); return; }
    const ln = lineAt(p, lines(), tol());
    if (ln) {
      // Double-click a bend point to remove it.
      const l = lineOf(ln.l.id);
      const k = l?.pts.findIndex(q => Math.hypot(q[0] - p[0], q[1] - p[1]) < tol() * 1.5) ?? -1;
      if (l && k >= 0) act({ SetPts: { id: l.id, pts: l.pts.filter((_, i) => i !== k) } });
      return;
    }
    if (!noteAt(p)) C.quickInsert(p);
  };

  const wheel = (e: WheelEvent) => {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) { const r = svgEl!.getBoundingClientRect(); zoomBy(Math.exp(-e.deltaY / 400), [e.clientX - r.left, e.clientY - r.top]); }
    else if (e.altKey) { const r = svgEl!.getBoundingClientRect(); zoomBy(Math.exp(-e.deltaY / 400), [e.clientX - r.left, e.clientY - r.top]); }
    else setPan([pan()[0] - (e.shiftKey ? e.deltaY : e.deltaX), pan()[1] - (e.shiftKey ? 0 : e.deltaY)]);
  };

  const keys = (e: KeyboardEvent) => {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement) return;
    const ctrl = e.ctrlKey || e.metaKey, k = e.key.toLowerCase();
    if (k === " ") { setSpace(true); return; }
    if (e.key === "Delete" || e.key === "Backspace") { e.preventDefault(); C.del(); }
    else if (ctrl && k === "c") { e.preventDefault(); C.copy(); }
    else if (ctrl && k === "x") { e.preventDefault(); C.copy(); C.del(); }
    else if (ctrl && k === "v") { e.preventDefault(); C.paste(); }
    else if (ctrl && k === "d") { e.preventDefault(); C.duplicate(); }
    else if (ctrl && k === "a") { e.preventDefault(); setSel([...items().map(i => i.id), ...lines().map(l => l.id), ...notes().map(x => x.id)]); }
    else if (!ctrl && k === "r") { e.preventDefault(); C.rotate(); }
    else if (!ctrl && k === "m") { e.preventDefault(); C.flip(); }
    else if (!ctrl && k === "l") { e.preventDefault(); setTool({ k: "line", kind: "major" }); }
    else if (e.key === "Escape") { if (tool().k !== "select") setTool({ k: "select" }); else setSel([]); }
    else if (e.key.startsWith("Arrow") && sel().length) {
      e.preventDefault();
      const s = e.shiftKey ? grid() * 4 : grid(), dx = e.key === "ArrowLeft" ? -s : e.key === "ArrowRight" ? s : 0, dy = e.key === "ArrowUp" ? -s : e.key === "ArrowDown" ? s : 0;
      act({ Move: { ids: sel(), dx, dy } });
    }
  };
  const keyup = (e: KeyboardEvent) => { if (e.key === " ") setSpace(false); };

  // A newly opened sheet is fitted to the window once it arrives.
  onFit(fit);
  // The first drawing is fitted when it arrives.
  const hasView = createMemo(() => !!v());
  createEffect(on(hasView, has => { if (has) requestAnimationFrame(fit); }));
  onMount(() => { addEventListener("mousemove", move); addEventListener("mouseup", up); addEventListener("keyup", keyup); });
  onCleanup(() => { removeEventListener("mousemove", move); removeEventListener("mouseup", up); removeEventListener("keyup", keyup); });

  const ctx = (e: MouseEvent) => {
    e.preventDefault();
    const p = toSheet(e.clientX, e.clientY);
    const it = itemAt(p), ln = it ? null : lineAt(p, lines(), tol());
    if (it && !selSet().has(it.id)) setSel([it.id]);
    if (ln && !selSet().has(ln.l.id)) setSel([ln.l.id]);
    const items_: MenuItem[] = it ? [
      ...(it.cat === "connector" ? [{ label: "Go to partner", run: () => C.goPartner(it.id) } as MenuItem, { label: "Pair with…", run: () => C.pairDialog(it.id) } as MenuItem, "-" as MenuItem] : []),
      { label: "Rotate", icon: "rotate", run: C.rotate, keys: "R" }, { label: "Mirror", icon: "flip", run: C.flip, keys: "M" },
      "-", { label: "Copy", icon: "copy", run: C.copy, keys: "Ctrl+C" }, { label: "Duplicate", run: C.duplicate, keys: "Ctrl+D" }, { label: "Delete", icon: "del", run: C.del, keys: "Del" },
      "-", { label: `About ${symOf(it.sym)?.name ?? "this symbol"}`, icon: "help", run: () => helpCard(`cat:${it.cat}`) },
    ] : ln ? [
      { label: "Reverse the flow", icon: "flow", run: () => C.reverse(ln.l.id) }, { label: "Straighten (route automatically)", run: () => act({ SetPts: { id: ln.l.id, pts: [] } }), off: !lineOf(ln.l.id)?.pts.length },
      { label: "Select the whole line number", run: () => C.selectRun(ln.l.id), off: ln.l.run === null },
      "-", { label: "Delete", icon: "del", run: C.del, keys: "Del" },
    ] : [
      { label: "Add a symbol here…", icon: "symbols", run: () => C.quickInsert(p) }, { label: "Paste", icon: "paste", run: C.paste, keys: "Ctrl+V" },
      "-", { label: "Select all", run: () => setSel([...items().map(i => i.id), ...lines().map(l => l.id)]), keys: "Ctrl+A" }, { label: "Fit the sheet", icon: "fit", run: fit, keys: "F" },
    ];
    openMenu(e, items_);
  };

  // Drop from the palette (HTML drag and drop).
  const drop = (e: DragEvent) => {
    e.preventDefault();
    const key = e.dataTransfer?.getData("text/x-datum-sym"); if (!key) return;
    const tag = e.dataTransfer?.getData("text/x-datum-tag") || undefined;
    const obj = Number(e.dataTransfer?.getData("text/x-datum-obj")) || undefined;
    placeAt(key, toSheet(e.clientX, e.clientY), tag, obj);
  };
  const placeAt = (key: string, p: Pt, tag?: string, obj?: number) => {
    const s = symOf(key), sh = sheet(); if (!s || !sh) return;
    const on = s.inline ? lineAt(p, lines().filter(l => !["pneumatic", "electric", "data", "capillary"].includes(l.kind)), Math.max(2.5, 8 / zoom())) : null;
    const at: Pt = on ? on.at : [snap(p[0]), snap(p[1])];
    C.place(key, at, on?.l.id ?? null, tag, obj);
  };

  const off = (id: number) => { const d = drag(); return d?.k === "move" && d.ids.includes(id) ? [d.dx, d.dy] : [0, 0]; };
  const preview = (l: LineView): string | null => {
    const d = drag();
    if (d?.k === "bend" && d.id === l.id) { const m = movedPath(l.path, d.seg, d.b[0] - d.a[0], d.b[1] - d.a[1]); return m ? pathD(m) : null; }
    return null;
  };
  const showPorts = () => tool().k === "line" || drag()?.k === "wire";
  const cls = (id: number) => ({ sel: selSet().has(id), err: issueSets().err.has(id), warn: !issueSets().err.has(id) && issueSets().warn.has(id) });

  return (
    <div class="canvas" classList={{ grid: opts().grid, panning: space() || drag()?.k === "pan", drawing: tool().k !== "select" }} tabindex="0" onKeyDown={keys}
      onDragOver={e => e.preventDefault()} onDrop={drop}
      style={{ "background-size": `${grid() * 4 * zoom()}px ${grid() * 4 * zoom()}px`, "background-position": `${pan()[0]}px ${pan()[1]}px` }}>
      <svg ref={svgEl} class="dsvg" onMouseDown={down} onDblClick={dbl} onWheel={wheel} onContextMenu={ctx} onMouseLeave={() => setMouse(null)}>
        <SymbolDefs />
        <Show when={v()}>{sv => <g transform={`translate(${pan()[0]} ${pan()[1]}) scale(${zoom()})`}>
          <rect x={0} y={0} width={sv().w} height={sv().h} class="paper" />
          <g class="frame"><For each={sv().frame}>{p => <PrimEl p={p} />}</For></g>
          <Show when={sv().legend}><g class="legend"><For each={sv().legend_rows}>{r => <LegendEl r={r} />}</For></g></Show>
          <g class="lines"><For each={lines()}>{l => {
            const pv = () => preview(l);
            return <g class={`ln ${l.kind}`} classList={{ ...cls(l.id), hi: !!l.number && l.number === (hiNumber() || selNumber()) }}>
              <path d={pathD(l.path)} class="lhit" />
              <Show when={pv()} fallback={<For each={l.segs}>{s => <polyline points={s.map(q => `${n(q[0])},${n(q[1])}`).join(" ")} class="lstroke" />}</For>}><path d={pv()!} class="lstroke" /></Show>
              <For each={l.marks}>{m => m.k === "text" && BADGE[m.s] ? <g class={`badge ${BADGE[m.s]}`}><circle cx={m.p[0]} cy={m.p[1]} r={1.9} /><PrimEl p={m} /></g> : <PrimEl p={m} cls="lmark" />}</For>
              <Show when={l.badge !== null}><title>{(() => { const h = l.run !== null ? derived()?.hyd[l.run] : null; return h ? `${["Within limits", "Close to or just outside the limits", "Outside the limits"][h.status]}${h.reason ? ": " + h.reason : ""}` : ""; })()}</title></Show>
            </g>;
          }}</For></g>
          <g class="items"><For each={items()}>{it => {
            const o = () => off(it.id);
            return <g class={`it ${it.cat}`} classList={{ ...cls(it.id), hover: hover() === it.id }} onMouseEnter={() => setHover(it.id)} onMouseLeave={() => setHover(null)} transform={`translate(${o()[0]} ${o()[1]})`}>
              <rect x={it.bbox[0] - 0.6} y={it.bbox[1] - 0.6} width={it.bbox[2] - it.bbox[0] + 1.2} height={it.bbox[3] - it.bbox[1] + 1.2} class="ihit" />
              <g transform={itemTransform(it)} class="sym"><use href={`#s-${it.sym}`} /></g>
              <For each={it.texts}>{t => <PrimEl p={t} />}</For>
              <Show when={showPorts() || hover() === it.id || selSet().has(it.id)}><For each={it.ports}>{q => <circle cx={q.p[0]} cy={q.p[1]} r={0.9} class="port" classList={{ used: q.used, sig: q.signal }} />}</For></Show>
            </g>;
          }}</For></g>
          <g class="notes"><For each={notes()}>{nt => {
            const o = () => off(nt.id);
            return <g class={`note ${nt.kind}`} classList={cls(nt.id)} transform={`translate(${o()[0]} ${o()[1]})`}>
              <rect x={nt.bbox[0] - 1} y={nt.bbox[1] - 1} width={nt.bbox[2] - nt.bbox[0] + 2} height={nt.bbox[3] - nt.bbox[1] + 2} class="nhit" />
              <For each={nt.prims}>{p => <PrimEl p={p} />}</For>
            </g>;
          }}</For></g>
          <Show when={drag()}>{d => {
            const x = d();
            if (x.k === "wire") return <path d={`M${x.a[0]} ${x.a[1]}H${(x.a[0] + x.b[0]) / 2}V${x.b[1]}H${x.b[0]}`} class="temp" />;
            if (x.k === "box") return <rect x={Math.min(x.a[0], x.b[0])} y={Math.min(x.a[1], x.b[1])} width={Math.abs(x.b[0] - x.a[0])} height={Math.abs(x.b[1] - x.a[1])} class="marquee" />;
            if (x.k === "bend" || x.k === "end") return <circle cx={x.b[0]} cy={x.b[1]} r={1} class="bendpt" />;
            return null;
          }}</Show>
          <Show when={tool().k === "place" && mouse()}>{m => { const t = tool() as { k: "place"; sym: string }; return <g transform={`translate(${snap(m()[0])} ${snap(m()[1])})`} class="ghost"><use href={`#s-${t.sym}`} /></g>; }}</Show>
        </g>}</Show>
      </svg>
      <Show when={v() && !v()!.items.length && !v()!.lines.length && !v()!.legend}>
        <div class="empty"><b>A blank sheet</b><span>Drag a symbol from the palette on the left (or double-click the sheet and type its name). Then drag from a port (the small circles on a symbol) to another to draw a line. Every symbol and line carries its data; the lists and checks below follow the drawing.</span>
          <span><button class="btn sm" onClick={() => C.examples()}>Open an example</button></span></div>
      </Show>
      <div class="zoomctl"><button title="Zoom out" onClick={() => zoomBy(1 / 1.25)}>−</button><span>{Math.round(zoom() / 3.78 * 100)}%</span><button title="Zoom in" onClick={() => zoomBy(1.25)}>+</button><button title="Fit the drawing in the window (F)" onClick={fit}>Fit</button><button title="The whole sheet (Shift+F)" onClick={fitSheet}>Sheet</button></div>
      <Show when={mouse()}>{m => <div class="coords">{v() && sheet() ? zoneOf(m()) : ""} · {Math.round(m()[0])}, {Math.round(m()[1])} mm</div>}</Show>
      <Show when={tool().k !== "select"}><div class="toolbadge" onClick={() => setTool({ k: "select" })}>{toolText()} · Esc to stop</div></Show>
    </div>
  );
};

const toolText = () => {
  const t = tool();
  if (t.k === "line") return `Drawing ${lib()?.kinds.find(k => k[0] === t.kind)?.[1].toLowerCase() ?? "lines"}: drag from a port, a line or anywhere`;
  if (t.k === "place") return `Placing ${symOf(t.sym)?.name.toLowerCase() ?? "a symbol"}${t.tag ? ` (${t.tag})` : ""}: click the sheet (Shift keeps placing)`;
  if (t.k === "note") return t.kind === "hold" ? "Click where the Hold cloud goes" : "Click where the note goes";
  return "";
};

/** The grid zone under a point (A–H across, 1–8 down), as the frame draws them. */
const zoneOf = (p: Pt) => {
  const v = derived()?.view; if (!v) return "";
  const big = v.w > 600, m = big ? 21 : 15;
  const c = Math.min(7, Math.max(0, Math.floor((p[0] - m) / (v.w - 2 * m) * 8))), r = Math.min(7, Math.max(0, Math.floor((p[1] - m) / (v.h - 2 * m) * 8)));
  return `${"ABCDEFGH"[c]}${r + 1}`;
};

/** One legend row: the symbol (scaled to fit) or a sample of the line type, and its name. */
export const LegendEl = (p: { r: { sym: string | null; kind: Kind | null; x: number; y: number; name: string } }) => {
  const s = () => (p.r.sym ? symOf(p.r.sym) : undefined);
  const header = () => !p.r.sym && !p.r.kind;
  return <g>
    <Show when={s()}>{sym => { const sc = Math.min(1, 30 / sym().w); return <g transform={`translate(${p.r.x + 15} ${p.r.y}) scale(${sc})`} class="sym"><use href={`#s-${sym().key}`} /><For each={sym().texts}>{t => <PrimEl p={t} />}</For></g>; }}</Show>
    <Show when={p.r.kind}>{k => <g class={`ln ${k()}`}><polyline points={`${p.r.x},${p.r.y} ${p.r.x + 30},${p.r.y}`} class="lstroke" /></g>}</Show>
    <text x={header() ? p.r.x : p.r.x + 36} y={p.r.y} font-size={header() ? "3" : "2.5"} class="pt" classList={{ b: header() }} dominant-baseline="central">{p.r.name}</text>
  </g>;
};

/** The selected things' data for the clipboard. */
export const copySelection = (): { items: Item[]; lines: Line[]; notes: Note[] } => {
  const p = proj(); const s = new Set(sel());
  if (!p) return { items: [], lines: [], notes: [] };
  return { items: p.items.filter(i => s.has(i.id)), lines: p.lines.filter(l => s.has(l.id)), notes: p.notes.filter(x => s.has(x.id)) };
};

/** Jump to the item or line an issue or list row points at: open its sheet, select it, centre it. */
export const goTo = (ids: number[], sheetId: number) => {
  const p = proj(); if (!p || !ids.length) return;
  if (sheet()?.id !== sheetId) setSheetId(sheetId);
  setSel(ids);
  const tryCentre = (n: number) => {
    const v = derived()?.view;
    if (!v || v.id !== sheetId) { if (n < 30) setTimeout(() => tryCentre(n + 1), 40); return; }
    const it = v.items.find(i => ids.includes(i.id)), ln = v.lines.find(l => ids.includes(l.id));
    const at: Pt | null = it ? [it.x, it.y] : ln ? ln.path[Math.floor(ln.path.length / 2)] : null;
    if (at) centreOn(at);
  };
  tryCentre(0);
};
