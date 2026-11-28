// The symbol palette: every symbol by category, searchable ("ball valve"), with the common
// instrument functions ready-lettered. Drag a symbol onto the sheet, or click it and then click the
// sheet.
import { AssembliesPanel } from "./assemblies";
import { For, Show, createSignal, createMemo, JSX } from "solid-js";
import { lib, setTool, tool, allLibs, proj } from "./store";
import { SymInfo, Cat } from "./api";
import { PrimEl } from "./canvas";

/** Common instruments: letters and the bubble they usually sit in (ISA-5.1). */
export const PRESETS: [string, string, string][] = [
  ["PT", "inst_field", "Pressure transmitter"], ["PI", "inst_field", "Pressure gauge"], ["FT", "inst_field", "Flow transmitter"], ["FI", "inst_field", "Flow indicator"],
  ["TT", "inst_field", "Temperature transmitter"], ["TI", "inst_field", "Temperature gauge"], ["LT", "inst_field", "Level transmitter"], ["LG", "inst_field", "Level gauge"],
  ["FIC", "inst_dcs", "Flow controller"], ["LIC", "inst_dcs", "Level controller"], ["PIC", "inst_dcs", "Pressure controller"], ["TIC", "inst_dcs", "Temperature controller"],
  ["UIC", "inst_dcs", "Anti-surge controller"], ["PSH", "inst_field", "Pressure switch, high"], ["PSL", "inst_field", "Pressure switch, low"], ["LAH", "inst_dcs", "Level alarm, high"],
  ["LAL", "inst_dcs", "Level alarm, low"], ["TSH", "inst_field", "Temperature switch, high"], ["HS", "inst_panel", "Hand switch"], ["XL", "inst_panel", "Indicating light"],
];

const CATS: Cat[] = ["vessel", "rotating", "heat", "valve", "control", "fitting", "instrument", "primary", "mark", "connector"];

/** A small picture of a symbol, fitted in a box. */
export const SymPic = (p: { s: SymInfo; size?: number; letters?: string }) => {
  const box = () => Math.max(p.s.w, p.s.h) * 1.15;
  return <svg class="sympic" viewBox={`${-box() / 2} ${-box() / 2} ${box()} ${box()}`} width={p.size ?? 30} height={p.size ?? 30}>
    <g class="sym" innerHTML={p.s.svg} />
    <For each={p.s.texts}>{t => <PrimEl p={t} />}</For>
    <Show when={p.letters}><text x={0} y={-0.3} font-size={String(Math.min(3.2, 9 / (p.letters!.length + 0.5)))} class="pt b" text-anchor="middle" dominant-baseline="central">{p.letters}</text></Show>
  </svg>;
};

/** The left pane's width (dragged, kept) and whether it is folded to a strip (as in every Datum app). */
const keep = (k: string, d: string) => { try { return localStorage.getItem(k) ?? d; } catch { return d; } };
const store = (k: string, v: string) => { try { localStorage.setItem(k, v); } catch { /* private window */ } };
export const [paneW, setPaneW] = createSignal(Math.min(480, Math.max(190, +keep("ds.pane.w", "250") || 250)));
export const [paneFolded, setPaneFolded] = createSignal(keep("ds.pane.folded", "0") === "1");
export const foldPane = (v: boolean) => { setPaneFolded(v); store("ds.pane.folded", v ? "1" : "0"); window.dispatchEvent(new Event("resize")); };
/** A side panel's edge, dragged to widen or narrow it (Datum Mesh's grip). */
export const grip = (e: PointerEvent, w: () => number, set: (v: number) => void, key: string, dir: 1 | -1, lo: number, hi: number) => {
  const x0 = e.clientX, w0 = w(), el = e.currentTarget as HTMLElement; el.setPointerCapture(e.pointerId);
  const mv = (m: PointerEvent) => set(Math.min(hi, Math.max(lo, w0 + dir * (m.clientX - x0))));
  const up = () => { el.removeEventListener("pointermove", mv); el.removeEventListener("pointerup", up); store(key, String(w())); window.dispatchEvent(new Event("resize")); };
  el.addEventListener("pointermove", mv); el.addEventListener("pointerup", up);
};

/** A branch of the tree (Datum Mesh's): a heading with its count that folds. */
const Branch = (p: { t: string; n: number; open: boolean; toggle: () => void; children: JSX.Element }) => (
  <div class="tgrp" classList={{ open: p.open }}>
    <button class="tsum" onClick={p.toggle}><span class="tt">{p.t}</span><span class="pill">{p.n}</span></button>
    <Show when={p.open}><div class="tlist">{p.children}</div></Show>
  </div>
);

export const Palette = () => {
  const [q, setQ] = createSignal("");
  const [open, setOpen] = createSignal<Record<string, boolean>>(JSON.parse(keep("ds.pane.open", '{"vessel":true,"rotating":true,"valve":true,"presets":true}')));
  const toggle = (k: string) => { const v = { ...open(), [k]: !open()[k] }; setOpen(v); store("ds.pane.open", JSON.stringify(v)); };
  const isOpen = (k: string) => !!q().trim() || !!open()[k];
  const match = (x: { name: string; cat_name?: string; key?: string }) => { const s = q().trim().toLowerCase(); return !s || x.name.toLowerCase().includes(s) || (x.cat_name ?? "").toLowerCase().includes(s) || (x.key ?? "").includes(s); };
  const groups = createMemo(() => {
    const all = lib()?.symbols.filter(x => x.key !== "unknown" && !x.key.startsWith("x.")) ?? [];
    return CATS.map(c => ({ c, name: all.find(x => x.cat === c)?.cat_name ?? c, syms: all.filter(x => x.cat === c && match(x)).sort((a, b) => Number(a.rare) - Number(b.rare)) })).filter(g => g.syms.length);
  });
  const presets = createMemo(() => { const s = q().trim().toLowerCase(); return PRESETS.filter(p => !s || p[0].toLowerCase().includes(s) || p[2].toLowerCase().includes(s)); });
  const pick = (sym: string, tag?: string) => setTool({ k: "place", sym, tag });
  const drag = (e: DragEvent, sym: string, tag?: string) => { e.dataTransfer?.setData("text/x-datum-sym", sym); if (tag) e.dataTransfer?.setData("text/x-datum-tag", tag); e.dataTransfer!.effectAllowed = "copy"; };
  const on = (sym: string, tag?: string) => { const t = tool(); return t.k === "place" && t.sym === sym && (t.tag ?? "") === (tag ?? ""); };
  const sym = (k: string) => lib()?.symbols.find(s => s.key === k);
  // With a drawing standard, our symbols are shown as the library draws them.
  const shown = (s: SymInfo) => { const k = proj()?.settings.standard.map[s.key]; return (k && sym(k)) || s; };
  const libGroups = createMemo(() => allLibs().filter(v => v.shown).map(v => ({ name: v.library.name, syms: v.library.symbols.map(c => sym(c.key)).filter((x): x is SymInfo => !!x && match(x)) })).filter(g => g.syms.length));
  const asm = () => (proj()?.settings.assemblies ?? []).filter(a => match({ name: a.name }));
  const Row = (r: { s: SymInfo; pic: SymInfo; label: string; tag?: string; title?: string }) => (
    <button class="ti psym" classList={{ on: on(r.s.key, r.tag), rare: r.s.rare }} draggable={true} title={r.title ?? `${r.s.name}${r.s.inline ? " (goes in a line)" : ""}`} onDragStart={e => drag(e, r.s.key, r.tag)} onClick={() => pick(r.s.key, r.tag)}>
      <SymPic s={r.pic} size={20} letters={r.tag} /><span>{r.label}</span></button>
  );
  return (
    <div class="palette">
      <label class="tfilter"><svg class="ic"><use href="#i-search" /></svg><input class="psearch" type="search" placeholder="Find a symbol (ball valve, pump…)" value={q()} onInput={e => setQ(e.currentTarget.value)} /></label>
      <div class="pbody tbody">
        <For each={groups()}>{g => <Branch t={g.name} n={g.syms.length} open={isOpen(g.c)} toggle={() => toggle(g.c)}>
          <For each={g.syms}>{s => <Row s={s} pic={shown(s)} label={s.name} />}</For></Branch>}</For>
        <Show when={presets().length}><Branch t="Common instruments" n={presets().length} open={isOpen("presets")} toggle={() => toggle("presets")}>
          <For each={presets()}>{([l, k, name]) => <Show when={sym(k)}>{s => <Row s={s()} pic={s()} label={name} tag={l} title={`${name} (${l})`} />}</Show>}</For></Branch></Show>
        <For each={libGroups()}>{g => <Branch t={g.name} n={g.syms.length} open={isOpen(`lib:${g.name}`)} toggle={() => toggle(`lib:${g.name}`)}>
          <For each={g.syms}>{s => <Row s={s} pic={s} label={s.name} />}</For></Branch>}</For>
        <Show when={asm().length}><Branch t="Assemblies" n={asm().length} open={isOpen("asm")} toggle={() => toggle("asm")}><AssembliesPanel /></Branch></Show>
        <Show when={!groups().length && !presets().length && !libGroups().length && !asm().length}><p class="muted pnone">No symbol is called that.</p></Show>
      </div>
    </div>
  );
};
