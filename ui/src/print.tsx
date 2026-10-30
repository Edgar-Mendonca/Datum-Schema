// File › Print: settings on the left, a live page preview on the right (FAMILY §8). Pages are drawn
// as SVG at real size and printed with the WebView's print dialog, where "Save as PDF" makes a PDF.
// The sheets print at their own size or fitted to A3; the lists flow over A3 pages with a header
// and footer on every page; the legend comes last. The same SVG writer saves a sheet as SVG.
import { For, Show, JSX, createSignal, createMemo, createResource, onMount, onCleanup } from "solid-js";
import { Portal } from "solid-js/web";
import { SheetView, Prim, List, Datasheet } from "./api";
import { call, proj, lib, sheet, setBackstage, symOf } from "./store";
import { Help } from "./help";
import { Check, Pick } from "./ui";
import { DatasheetTable, RevBlock, revLine } from "./datasheet";

export const MM = 96 / 25.4;
const esc = (s: string) => s.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
const n = (v: number) => String(Math.round(v * 1000) / 1000);

const prim = (p: Prim, w?: number): string => {
  switch (p.k) {
    case "poly": {
      const pts = p.pts.map(q => `${n(q[0])},${n(q[1])}`).join(" ");
      const st = `stroke-width="${n(w ?? p.w)}"${p.dash ? ' stroke-dasharray="1.2 0.8"' : ""}${p.fill ? ' fill="#000"' : ""}`;
      return p.closed ? `<polygon points="${pts}" ${st}/>` : `<polyline points="${pts}" ${st}/>`;
    }
    case "circle": return `<circle cx="${n(p.c[0])}" cy="${n(p.c[1])}" r="${n(p.r)}" stroke-width="${n(p.w)}"${p.fill ? ' fill="#000"' : ""}/>`;
    case "arc": {
      const pt = (a: number) => [p.c[0] + p.r * Math.cos(a * Math.PI / 180), p.c[1] + p.r * Math.sin(a * Math.PI / 180)];
      const [x0, y0] = pt(p.a0), [x1, y1] = pt(p.a1), large = ((p.a1 - p.a0) % 360 + 360) % 360 > 180 ? 1 : 0;
      return `<path d="M${n(x0)} ${n(y0)}A${n(p.r)} ${n(p.r)} 0 ${large} 1 ${n(x1)} ${n(y1)}" stroke-width="${n(p.w)}"/>`;
    }
    case "text": return `<text x="${n(p.p[0])}" y="${n(p.p[1])}" font-size="${n(p.h)}" text-anchor="${["start", "middle", "end"][p.anchor] ?? "middle"}" dominant-baseline="central" fill="#000" stroke="none"${p.bold ? ' font-weight="600"' : ""}>${esc(p.s)}</text>`;
  }
};

const WIDTH: Record<string, number> = { major: 0.6, minor: 0.35, utility: 0.45, pneumatic: 0.25, electric: 0.25, data: 0.25, capillary: 0.25 };

/** A sheet as a standalone SVG drawing (millimetres, black on white). */
export const svgString = (v: SheetView, opts: { width?: number; height?: number } = {}): string => {
  const used = new Set([...v.items.map(i => i.sym), ...v.legend_rows.map(r => r.sym).filter(Boolean) as string[]]);
  const defs = (lib()?.symbols ?? []).filter(s => used.has(s.key)).map(s => `<g id="s-${s.key}" stroke-width="0.25">${s.svg.replace(/class="sf"/g, 'fill="#000"')}</g>`).join("");
  const body: string[] = [];
  body.push(`<rect width="${n(v.w)}" height="${n(v.h)}" fill="#fff" stroke="none"/>`);
  body.push(`<g>${v.frame.map(p => prim(p)).join("")}</g>`);
  for (const l of v.lines) {
    const dash = l.kind === "electric" ? ' stroke-dasharray="1.5 1"' : "";
    body.push(`<g stroke-width="${WIDTH[l.kind] ?? 0.35}"${dash}>${l.segs.map(s => `<polyline points="${s.map(q => `${n(q[0])},${n(q[1])}`).join(" ")}"/>`).join("")}</g>`);
    body.push(`<g>${l.marks.map(m => prim(m)).join("")}</g>`);
  }
  for (const it of v.items) {
    body.push(`<g transform="translate(${n(it.x)} ${n(it.y)}) rotate(${it.rot * 90}) scale(${n(it.flip ? -it.scale : it.scale)} ${n(it.scale)})"><use href="#s-${it.sym}"/></g>`);
    body.push(it.texts.map(t => prim(t)).join(""));
  }
  for (const nt of v.notes) body.push(nt.prims.map(p => prim(p)).join(""));
  for (const r of v.legend_rows) {
    const s = r.sym ? symOf(r.sym) : undefined;
    if (s) { const sc = Math.min(1, 30 / s.w); body.push(`<g transform="translate(${n(r.x + 15)} ${n(r.y)}) scale(${n(sc)})"><use href="#s-${s.key}"/>${s.texts.map(t => prim(t)).join("")}</g>`); }
    if (r.kind) body.push(`<polyline points="${n(r.x)},${n(r.y)} ${n(r.x + 30)},${n(r.y)}" stroke-width="${WIDTH[r.kind] ?? 0.35}"${r.kind === "electric" ? ' stroke-dasharray="1.5 1"' : ""}/>`);
    const header = !r.sym && !r.kind;
    body.push(`<text x="${n(header ? r.x : r.x + 36)}" y="${n(r.y)}" font-size="${header ? 3 : 2.5}" dominant-baseline="central" fill="#000" stroke="none"${header ? ' font-weight="600"' : ""}>${esc(r.name)}</text>`);
  }
  const w = opts.width ?? v.w, h = opts.height ?? v.h;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${n(v.w)} ${n(v.h)}" width="${n(w)}mm" height="${n(h)}mm" fill="none" stroke="#000" stroke-linecap="round" stroke-linejoin="round" font-family="IBM Plex Sans, Segoe UI, Arial, sans-serif"><defs>${defs}</defs>${body.join("")}</svg>`;
};

/** One sheet as an SVG file's text. */
export const sheetSvg = async (id: number) => {
  const vs = await call<SheetView[]>("views");
  const v = vs?.find(x => x.id === id) ?? vs?.[0];
  return v ? `<?xml version="1.0" encoding="UTF-8"?>\n${svgString(v)}` : "";
};

type Page = { w: number; h: number; body: () => JSX.Element; label: string };
const LISTS: [string, string][] = [["lines", "Line list"], ["equipment", "Equipment list"], ["instruments", "Instrument index"], ["valves", "Valve list"], ["datasheets", "Datasheet index"]];
const [printing, setPrinting] = createSignal(false);
/** Set by a datasheet's Print button: print only that object's datasheet. */
export const [printOnly, setPrintOnly] = createSignal<number | null>(null);

export const PrintPage = () => {
  const [views] = createResource(() => call<SheetView[]>("views"));
  const [which, setWhich] = createSignal<"all" | "this">("all");
  const [fitA3, setFitA3] = createSignal(false);
  const [want, setWant] = createSignal<Record<string, boolean>>({ lines: true, equipment: true, instruments: true, valves: true, datasheets: false, legend: true, sheets_ds: false });
  const [datasheets] = createResource(() => call<Datasheet[]>("datasheets_all"));
  onCleanup(() => setPrintOnly(null));
  const [lists] = createResource(async () => Promise.all(LISTS.map(([k]) => call<List>("list", { kind: k }).then(l => l ?? null).catch(() => null))));
  const [zoomP, setZoomP] = createSignal(0.4);
  const sheets = createMemo(() => (views() ?? []).filter(v => which() === "all" || v.id === sheet()?.id));
  /** Rows per A3 landscape list page. */
  const ROWS = 36;
  /** A datasheet on A4 portrait, with a header and the title block. */
  const dsPage = (d: Datasheet): Page => {
    const p = proj();
    return { w: 210, h: 297, label: `Datasheet ${d.tag}`, body: () => (
      <div class="plist pds">
        <header><b>{d.title} · {d.tag}</b><span>{p?.title.project || p?.name}</span><span class="grow" /><span>{revLine(d)}</span></header>
        <DatasheetTable d={d} />
        <RevBlock d={d} />
        <table class="ptb"><tbody><tr>
          <td><small>COMPANY</small>{p?.title.company || "—"}</td><td><small>PROJECT</small>{p?.title.project || p?.name}</td><td><small>TAG</small>{d.tag}</td><td><small>REV</small>{d.rev || "—"}</td>
          <td><small>BY</small>{p?.title.by || "—"}</td><td><small>CHECKED</small>{p?.title.checked || "—"}</td><td><small>DATE</small>{new Date().toLocaleDateString()}</td>
        </tr></tbody></table>
      </div>) };
  };
  const pages = createMemo<Page[]>(() => {
    const out: Page[] = [];
    const only = printOnly();
    if (only !== null) return (datasheets() ?? []).filter(d => d.obj === only || d.item === only).map(dsPage);
    for (const v of sheets()) {
      const [w, h] = fitA3() ? [420, 297] : [v.w, v.h];
      out.push({ w, h, label: v.number, body: () => <div innerHTML={svgString(v, { width: w, height: h })} /> });
    }
    const p = proj();
    (lists() ?? []).forEach((l, i) => {
      if (!l || !want()[LISTS[i][0]] || !l.rows.length) return;
      const chunks = Math.max(1, Math.ceil(l.rows.length / ROWS));
      for (let c = 0; c < chunks; c++) out.push({ w: 420, h: 297, label: `${l.title} ${c + 1}/${chunks}`, body: () => (
        <div class="plist">
          <header><b>{l.title}</b><span>{p?.title.project || p?.name}</span><span>{p?.title.company}</span><span>Page {c + 1} of {chunks}</span></header>
          <table class="tbl small"><thead><tr><For each={l.cols}>{col => <th classList={{ r: col.num }}>{col.label}</th>}</For></tr></thead>
            <tbody><For each={l.rows.slice(c * ROWS, (c + 1) * ROWS)}>{r => <tr><For each={r.cells}>{(x, j) => <td classList={{ r: l.cols[j()].num }}>{x}</td>}</For></tr>}</For></tbody></table>
          <table class="ptb"><tbody><tr>
            <td><small>COMPANY</small>{p?.title.company || "—"}</td><td><small>PROJECT</small>{p?.title.project || p?.name}</td><td><small>DOCUMENT</small>{l.title}</td>
            <td><small>BY</small>{p?.title.by || "—"}</td><td><small>CHECKED</small>{p?.title.checked || "—"}</td><td><small>DATE</small>{new Date().toLocaleDateString()}</td><td><small>PAGE</small>{c + 1} of {chunks}</td>
          </tr></tbody></table>
        </div>) });
    });
    if (want().sheets_ds) for (const d of datasheets() ?? []) out.push(dsPage(d));
    if (want().legend && !(views() ?? []).some(v => v.legend)) {
      const used = [...new Set((p?.items ?? []).map(i => i.sym))].map(k => symOf(k)).filter(Boolean);
      if (used.length) out.push({ w: 420, h: 297, label: "Legend", body: () => (
        <div class="plist"><header><b>Legend</b><span>{p?.title.project || p?.name}</span></header>
          <div class="plegend"><For each={used}>{s => <div><svg viewBox={`${-Math.max(s!.w, s!.h) * 0.6} ${-Math.max(s!.w, s!.h) * 0.6} ${Math.max(s!.w, s!.h) * 1.2} ${Math.max(s!.w, s!.h) * 1.2}`} width="40" height="40" class="sym print" innerHTML={s!.svg} /><span>{s!.name}</span></div>}</For></div></div>) });
    }
    return out;
  });
  const doPrint = () => {
    setPrinting(true);
    const css = document.createElement("style");
    // One page size per print: each sheet at its size needs the printer's own sizes; A3 is the common one.
    const big = pages().reduce((m, p) => (p.w * p.h > m.w * m.h ? p : m), { w: 420, h: 297 } as { w: number; h: number });
    css.textContent = `@page { size: ${big.w}mm ${big.h}mm; margin: 0 }`;
    document.head.append(css);
    document.documentElement.classList.add("printing");
    setTimeout(() => { window.print(); document.documentElement.classList.remove("printing"); css.remove(); setPrinting(false); }, 300);
  };
  onMount(() => setZoomP(0.4));
  const Sheet = (p: { pg: Page }) => <div class="ppage" style={{ width: `${p.pg.w * MM}px`, height: `${p.pg.h * MM}px` }}>{p.pg.body()}</div>;
  return (
    <div class="printpage">
      <div class="pset">
        <h1>Print <Help k="export" /></h1>
        <div class="pbtns"><button class="btn pri big" onClick={doPrint}><svg class="ic"><use href="#i-print" /></svg> Print / Save as PDF</button></div>
        <p class="muted">In the print window choose your printer, or <b>Save as PDF</b>. {pages().length} {pages().length === 1 ? "page" : "pages"}.</p>
        <Show when={printOnly() !== null}><p class="small">Printing one datasheet. <button class="btn sm" onClick={() => setPrintOnly(null)}>Choose what to print</button></p></Show>
        <Show when={printOnly() === null}>
        <h4>Sheets</h4>
        <Pick value={which()} set={v => setWhich(v as any)} options={[["all", "Every sheet"], ["this", "This sheet only"]]} />
        <Check label="Fit every sheet to A3" value={fitA3()} set={setFitA3} />
        <h4>Lists</h4>
        <For each={LISTS}>{([k, l]) => <Check label={l} value={!!want()[k]} set={v => setWant({ ...want(), [k]: v })} />}</For>
        <Check label="Legend (when there is no legend sheet)" value={!!want().legend} set={v => setWant({ ...want(), legend: v })} />
        <Check label={`Datasheets (${datasheets()?.length ?? 0}, one A4 page each)`} value={!!want().sheets_ds} set={v => setWant({ ...want(), sheets_ds: v })} />
        </Show>
        <p class="muted small">Sheets print at their own size (A3, A1, ANSI B, ANSI D) unless fitted to A3; the lists print on A3 landscape with a header and page numbers; datasheets on A4 portrait.</p>
        <p><button class="btn sm" onClick={() => setBackstage(null)}>Back to the drawing</button></p>
      </div>
      <div class="pprev">
        <div class="pzoom"><span class="muted">Preview</span><input type="range" min="0.15" max="1" step="0.05" value={zoomP()} onInput={e => setZoomP(+e.currentTarget.value)} /><span class="muted">{Math.round(zoomP() * 100)}%</span></div>
        <For each={pages()} fallback={<p class="pempty">Nothing chosen to print.</p>}>{(pg, i) =>
          <div class="pwrap" style={{ width: `${pg.w * MM * zoomP()}px`, height: `${pg.h * MM * zoomP()}px` }}><div style={{ transform: `scale(${zoomP()})`, "transform-origin": "0 0" }}><Sheet pg={pg} /></div><small>Page {i() + 1} of {pages().length} · {pg.label}</small></div>}</For>
      </div>
      <Show when={printing()}><Portal mount={document.body}><div id="printroot"><For each={pages()}>{pg => <Sheet pg={pg} />}</For></div></Portal></Show>
    </div>
  );
};

