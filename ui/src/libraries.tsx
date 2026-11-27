// Symbol libraries: a client's own symbols, read from their legend sheet (DXF, DWG or PDF) or drawn
// in the symbol editor, reviewed here (what each symbol stands for, its connection points), named
// as the user likes, kept in a project, on this computer, or both, and shared as a file. A project's drawing standard draws Datum Schema's symbols with a library's (engine:
// symlib.rs; the window only edits and shows).
import { For, Show, createMemo, createSignal, createEffect, on } from "solid-js";
import { CPort, CustomSym, LibView, SymLibrary, Settings, Cat, invoke } from "./api";
import { call, act, proj, lib, refreshLibs, openDialog, toast, allLibs, LibEntry } from "./store";
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
const fmt1 = (v: number) => String(Math.round(v * 100) / 100);

/** The symbol editor: draw a symbol of your own (lines, shapes, circles, dots, text on a snap grid),
 *  or start from a copy of any symbol, then say what it is: one of ours drawn your way, or a new
 *  kind of object with its class, category and tag prefix. */
export const symbolEditor = (start: CustomSym | null, done: (c: CustomSym) => void) => openDialog(() => {
  type Shape = { k: "l" | "p" | "f" | "d" | "c" | "fc" | "r" | "t"; pts: [number, number][]; r?: number; text?: string };
  const [base, setBase] = createSignal(start?.draw ?? "");
  const [baseWH, setBaseWH] = createSignal<[number, number]>(start ? [start.w, start.h] : [0, 0]);
  const [shapes, setShapes] = createSignal<Shape[]>([]);
  const [tool, setTool] = createSignal<Shape["k"]>("l");
  const [cur, setCur] = createSignal<[number, number][]>([]);
  const [hover, setHover] = createSignal<[number, number] | null>(null);
  const [c, setC] = createSignal<CustomSym>(start ? { ...start } : { key: "", name: "", library: "", block: "", like: "", class: "UnknownItem", cat: "mark", w: 10, h: 10, draw: "", ports: [], prefix: "", inline: false, terminal: false });
  const [svg, setSvg] = createSignal("");
  const M = 20, G = 0.5;
  let el!: SVGSVGElement;
  const at = (e: MouseEvent): [number, number] => { const pt = el.createSVGPoint(); pt.x = e.clientX; pt.y = e.clientY; const q = pt.matrixTransform(el.getScreenCTM()!.inverse()); return [Math.round(q.x / G) * G, Math.round(q.y / G) * G]; };
  const str = (sh: Shape) => sh.k === "c" || sh.k === "fc" ? `${sh.k} ${fmt1(sh.pts[0][0])} ${fmt1(sh.pts[0][1])} ${fmt1(sh.r ?? 1)}` : sh.k === "r" ? `r ${fmt1(Math.min(sh.pts[0][0], sh.pts[1][0]))} ${fmt1(Math.min(sh.pts[0][1], sh.pts[1][1]))} ${fmt1(Math.abs(sh.pts[1][0] - sh.pts[0][0]))} ${fmt1(Math.abs(sh.pts[1][1] - sh.pts[0][1]))}` : sh.k === "t" ? `t ${fmt1(sh.pts[0][0])} ${fmt1(sh.pts[0][1])} 2.5 ${(sh.text ?? "").replace(/;/g, ",")}` : `${sh.k} ${sh.pts.map(p => `${fmt1(p[0])} ${fmt1(p[1])}`).join(" ")}`;
  const draw = () => [base(), ...shapes().map(str)].filter(x => x.trim()).join("; ");
  const wh = (): [number, number] => {
    let [w, h] = baseWH();
    for (const sh of shapes()) for (const p of sh.pts) { const r = sh.k === "c" || sh.k === "fc" ? sh.r ?? 0 : 0; w = Math.max(w, 2 * (Math.abs(p[0]) + r)); h = Math.max(h, 2 * (Math.abs(p[1]) + r)); }
    return [Math.max(1, w), Math.max(1, h)];
  };
  createEffect(on(draw, d => { invoke<string>("symbol_svg", { draw: d }).then(setSvg, () => setSvg("")); }));
  const finish = () => { const k = tool(), p = cur(); if ((k === "l" || k === "d") && p.length >= 2) setShapes([...shapes(), { k, pts: p }]); if ((k === "p" || k === "f") && p.length >= 3) setShapes([...shapes(), { k, pts: p }]); setCur([]); };
  const click = (e: MouseEvent) => {
    const q = at(e), k = tool(), p = [...cur(), q];
    if (k === "t") { const text = window.prompt("Text (a letter or a short word):", "M"); if (text) setShapes([...shapes(), { k: "t", pts: [q], text: text.slice(0, 12) }]); return; }
    if (k === "c" || k === "fc") { if (p.length === 2) { setShapes([...shapes(), { k, pts: [p[0]], r: k === "fc" ? Math.max(0.3, Math.hypot(p[1][0] - p[0][0], p[1][1] - p[0][1])) : Math.max(0.5, Math.hypot(p[1][0] - p[0][0], p[1][1] - p[0][1])) }]); setCur([]); } else setCur(p); return; }
    if (k === "r") { if (p.length === 2) { setShapes([...shapes(), { k, pts: p }]); setCur([]); } else setCur(p); return; }
    setCur(p);
  };
  const tools: [Shape["k"], string, string][] = [["l", "Line", "Click each point; double-click or Enter ends it"], ["p", "Shape", "A closed outline: click each corner; double-click ends it"], ["f", "Filled shape", "A filled outline (an arrowhead, a solid triangle)"], ["d", "Dashed line", "A dashed line: click each point; double-click ends it"], ["r", "Rectangle", "Click two corners"], ["c", "Circle", "Click the centre, then a point on the circle"], ["fc", "Dot", "A filled circle: click the centre, then its edge"], ["t", "Text", "Click where a letter or word goes"]];
  const fromSym = (key: string) => { const x = lib()?.symbols.find(s => s.key === key); if (!x) return; invoke<string>("symbol_draw", { key }).then(d => { setBase(d); setBaseWH([x.w, x.h]); setShapes([]); }, () => toast("That symbol's drawing could not be read.")); };
  const classes = () => (lib()?.classes ?? []).filter(k => k.name !== "UnknownItem").map(k => [k.name, k.label || k.name] as [string, string]).sort((a, b) => a[1].localeCompare(b[1]));
  const ok = () => {
    if (!c().name.trim()) { toast("Give the symbol a name."); return false; }
    if (!draw().trim()) { toast("Draw the symbol first."); return false; }
    const [w, h] = wh();
    done({ ...c(), name: c().name.trim(), block: c().block || c().name.trim().toUpperCase().replace(/[^A-Z0-9]+/g, "_").slice(0, 40), draw: draw(), w, h, inline: c().inline && c().ports.filter(q => !q.signal).length === 2 });
  };
  return (
    <Dialog title={start ? `Symbol · ${start.name}` : "New symbol"} wide help="symboleditor" ok={ok} okText="Use this symbol">
      <div class="symed">
        <div class="symtools">
          <For each={tools}>{([k, l, t]) => <button class="btn sm" classList={{ on: tool() === k }} title={t} onClick={() => { finish(); setTool(k); }}>{l}</button>}</For>
          <span class="grow" />
          <button class="btn sm" title="Take away the last thing drawn" onClick={() => { setCur([]); setShapes(shapes().slice(0, -1)); }}>Undo</button>
          <button class="btn sm" title="Start again from nothing" onClick={() => { setCur([]); setShapes([]); setBase(""); setBaseWH([0, 0]); }}>Clear</button>
        </div>
        <svg ref={el} class="sym symedsvg" viewBox={`${-M} ${-M} ${2 * M} ${2 * M}`} tabindex="0" onClick={click} onDblClick={finish} onMouseMove={e => setHover(at(e))} onMouseLeave={() => setHover(null)} onKeyDown={e => { if (e.key === "Enter") finish(); if (e.key === "Escape") { e.stopPropagation(); setCur([]); } }}>
          <defs><pattern id="symgrid" width="1" height="1" patternUnits="userSpaceOnUse"><path d="M1 0H0V1" fill="none" class="symgridl" /></pattern></defs>
          <rect x={-M} y={-M} width={2 * M} height={2 * M} fill="url(#symgrid)" />
          <line x1={-M} y1={0} x2={M} y2={0} class="symaxis" /><line x1={0} y1={-M} x2={0} y2={M} class="symaxis" />
          <g innerHTML={svg()} />
          <Show when={cur().length}><polyline points={[...cur(), ...(hover() ? [hover()!] : [])].map(p => p.join(",")).join(" ")} class="symcur" /></Show>
          <Show when={hover()}>{h => <circle cx={h()[0]} cy={h()[1]} r={0.35} class="symhov" />}</Show>
        </svg>
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
          <p class="muted small">{(() => { const [w, h] = wh(); return `${fmt1(w)} × ${fmt1(h)} mm, centred on the cross. A grid square is 1 mm; points snap to 0.5 mm.`; })()}</p>
        </div>
      </div>
    </Dialog>
  );
});

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
      if (inProject() || (st.libraries ?? []).some(x => x.id === kept.id)) await act({ SetSettings: { settings: { ...st, libraries: inProject() ? [...others, kept] : others } } });
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
      </>}>{x => <Editor view={x().v} inProject={x().inProject} onComputer={x().onComputer} back={() => setEditing(null)} />}</Show>
    </div>
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
