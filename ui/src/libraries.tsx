// Symbol libraries: a client's own symbols, read from their legend sheet (DXF), reviewed here
// (what each symbol stands for, its connection points), kept on this computer and shared as a
// file. A project's drawing standard draws Datum Schema's symbols with a library's (engine:
// symlib.rs; the window only edits and shows).
import { For, Show, createMemo, createSignal } from "solid-js";
import { CPort, CustomSym, LibView, SymLibrary } from "./api";
import { call, act, proj, lib, libs, refreshLibs, openDialog, toast } from "./store";
import { Dialog, Pick } from "./ui";
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

/** The review of a library: what each symbol stands for, its description and points. */
const Editor = (p: { view: LibView; back: () => void }) => {
  const [l, setL] = createSignal<SymLibrary>(structuredClone(p.view.library));
  const [svgs, setSvgs] = createSignal<string[]>([...p.view.svgs]);
  const setSym = (i: number, c: CustomSym) => setL({ ...l(), symbols: l().symbols.map((s, j) => (j === i ? c : s)) });
  const restyle = async (i: number, like: string) => { const c = await call<CustomSym>("symbol_like", { sym: l().symbols[i], like }); if (c) setSym(i, c); };
  const remove = (i: number) => { setL({ ...l(), symbols: l().symbols.filter((_, j) => j !== i) }); setSvgs(svgs().filter((_, j) => j !== i)); };
  const unmatched = createMemo(() => l().symbols.filter(s => !s.like).length);
  const save = async () => {
    const v = await call<LibView>("library_save", { lib: l(), old: p.view.file || null });
    if (!v) return;
    await refreshLibs();
    toast(`Saved the library “${v.library.name}” with ${v.library.symbols.length} symbols.${v.warnings.length ? " " + v.warnings.join(" ") : ""}`, true);
    p.back();
  };
  return (
    <div class="libedit">
      <div class="libhead">
        <label>Library name <input value={l().name} onInput={e => setL({ ...l(), name: e.currentTarget.value })} /></label>
        <span class="muted small">{l().source ? `From ${l().source} · ` : ""}{l().symbols.length} symbols{unmatched() ? ` · ${unmatched()} not matched yet` : ""}</span>
        <span class="grow" />
        <button class="btn" onClick={p.back}>Cancel</button><button class="btn pri" onClick={save}>Save library</button>
      </div>
      <Show when={p.view.warnings.length}><p class="small warnline">{p.view.warnings.join(" ")}</p></Show>
      <p class="muted small">Check what each symbol stands for: it is then the same object as ours (class, tag prefix, data, checks and lists), only drawn the client's way. Connection points were suggested where lines meet the drawing; check them for anything that is not a simple valve.</p>
      <div class="libscroll"><table class="tbl libtbl">
        <thead><tr><th /><th>Description</th><th>Block</th><th>Stands for</th><th>Points</th><th>In-line</th><th /></tr></thead>
        <tbody><For each={l().symbols}>{(s, i) => <tr classList={{ warn: !s.like }}>
          <td><LibPic svg={svgs()[i()]} w={s.w} h={s.h} size={38} /></td>
          <td><input value={s.name} onChange={e => setSym(i(), { ...s, name: e.currentTarget.value })} /></td>
          <td class="mono small">{s.block}</td>
          <td><Pick value={s.like} set={v => restyle(i(), v)} options={likeOptions()} /></td>
          <td><button class="btn sm" onClick={() => portEditor(s, svgs()[i()], ports => setSym(i(), { ...s, ports, inline: s.inline && ports.filter(q => !q.signal).length === 2 }))}>{s.ports.length} · Edit…</button></td>
          <td class="c"><input type="checkbox" checked={s.inline} disabled={s.ports.filter(q => !q.signal).length !== 2} title="Goes into a line (two process connection points)" onChange={e => setSym(i(), { ...s, inline: e.currentTarget.checked })} /></td>
          <td><button class="x" title="Leave this symbol out" onClick={() => remove(i())}>×</button></td>
        </tr>}</For></tbody>
      </table></div>
    </div>
  );
};

/** File › Libraries. */
export const LibrariesPage = () => {
  const [editing, setEditing] = createSignal<LibView | null>(null);
  const scan = async () => { const v = await call<LibView | null>("legend_scan"); if (v) setEditing(v); };
  const add = async () => { const v = await call<LibView | null>("library_import"); if (v) { await refreshLibs(); toast(`Added the library “${v.library.name}”.`, true); } };
  return (
    <div class="libraries">
      <Show when={editing()} fallback={<>
        <h1>Symbol libraries <Help k="libraries" /></h1>
        <p class="muted">A client's own symbols, read from their legend sheet. Save a DWG legend as DXF from any CAD program, then import it here. Libraries are kept on this computer; send one to a colleague as a file.</p>
        <p class="libbtns"><button class="btn pri" onClick={scan}>Import a legend (DXF)…</button><button class="btn" onClick={add}>Add a library file…</button></p>
        <Show when={libs().length} fallback={<p class="muted">No libraries yet.</p>}>
          <div class="libcards"><For each={libs()}>{v => <div class="libcard">
            <div class="libpics"><For each={v.library.symbols.slice(0, 8)}>{(s, i) => <LibPic svg={v.svgs[i()]} w={s.w} h={s.h} size={28} />}</For></div>
            <b>{v.library.name}</b><span class="muted small">{v.library.symbols.length} symbols{v.library.source ? ` · from ${v.library.source}` : ""}</span>
            <div class="libcbtns">
              <button class="btn sm" onClick={() => setEditing(v)}>Review…</button>
              <button class="btn sm" onClick={async () => { const f = await call<string | null>("library_export", { file: v.file }); if (f) toast(`Saved ${f}.`, true); }}>Save a copy…</button>
              <button class="btn sm" onClick={() => D.confirm(`Delete the library “${v.library.name}” from this computer? Projects that use its symbols keep their own copies.`, "Delete", async () => { await call("library_delete", { file: v.file }); await refreshLibs(); })}>Delete</button>
            </div>
          </div>}</For></div>
        </Show>
      </>}>{v => <Editor view={v()} back={() => setEditing(null)} />}</Show>
    </div>
  );
};

/** Options › Drawing standard: draw this project with a library's symbols. */
export const StandardSec = () => {
  const st = () => proj()!.settings.standard;
  const [pick, setPick] = createSignal(st().library);
  const count = () => Object.keys(st().map).length;
  const apply = () => {
    const v = libs().find(x => x.library.name === pick());
    const map: Record<string, string> = {};
    for (const s of v?.library.symbols ?? []) if (s.like && !map[s.like]) map[s.like] = s.key;
    act({ SetSettings: { settings: { ...proj()!.settings, standard: { library: v ? v.library.name : "", map } } } }).then(o => o && toast(v ? `Drawn to ${v.library.name}: ${Object.keys(map).length} of our symbols use its drawings.` : "Drawn with Datum Schema's symbols again.", true));
  };
  return (
    <div class="stdsec">
      <div class="form">
        <label>Draw with</label>
        <div class="inline"><Pick value={pick()} set={setPick} options={[["", "Datum Schema's symbols"], ...libs().map(v => [v.library.name, v.library.name] as [string, string]), ...(st().library && !libs().some(v => v.library.name === st().library) ? [[st().library, `${st().library} (in this project)`] as [string, string]] : [])]} />
          <button class="btn sm pri" onClick={apply} disabled={pick() === st().library && !!st().library}>Apply</button></div>
      </div>
      <p class="muted small">{st().library ? `Now drawn to ${st().library}: ${count()} of our symbols use its drawings. Objects, tags, data, checks and lists stay the same; only the drawing changes.` : "Now drawn with Datum Schema's own symbols."}</p>
    </div>
  );
};
