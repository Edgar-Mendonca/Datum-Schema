// The symbol palette: every symbol by category, searchable ("ball valve"), with the common
// instrument functions ready-lettered. Drag a symbol onto the sheet, or click it and then click the
// sheet.
import { For, Show, createSignal, createMemo } from "solid-js";
import { lib, setTool, tool } from "./store";
import { SymInfo, Cat } from "./api";
import { PrimEl } from "./canvas";
import { Help } from "./help";
import { PlantPanel } from "./plant";

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

export const Palette = () => {
  const [q, setQ] = createSignal("");
  const [open, setOpen] = createSignal<Record<string, boolean>>({ vessel: true, rotating: true, valve: true, presets: true });
  const groups = createMemo(() => {
    const s = q().trim().toLowerCase();
    const all = lib()?.symbols.filter(x => x.key !== "unknown") ?? [];
    const match = (x: SymInfo) => !s || x.name.toLowerCase().includes(s) || x.cat_name.toLowerCase().includes(s) || x.key.includes(s);
    return CATS.map(c => ({ c, name: all.find(x => x.cat === c)?.cat_name ?? c, syms: all.filter(x => x.cat === c && match(x)).sort((a, b) => Number(a.rare) - Number(b.rare)) })).filter(g => g.syms.length);
  });
  const presets = createMemo(() => { const s = q().trim().toLowerCase(); return PRESETS.filter(p => !s || p[0].toLowerCase().includes(s) || p[2].toLowerCase().includes(s)); });
  const pick = (sym: string, tag?: string) => setTool({ k: "place", sym, tag });
  const drag = (e: DragEvent, sym: string, tag?: string) => { e.dataTransfer?.setData("text/x-datum-sym", sym); if (tag) e.dataTransfer?.setData("text/x-datum-tag", tag); e.dataTransfer!.effectAllowed = "copy"; };
  const on = (sym: string, tag?: string) => { const t = tool(); return t.k === "place" && t.sym === sym && (t.tag ?? "") === (tag ?? ""); };
  const sym = (k: string) => lib()?.symbols.find(s => s.key === k);
  const [side, setSide] = createSignal<"symbols" | "plant">("symbols");
  return (
    <aside class="palette">
      <header><button classList={{ on: side() === "symbols" }} onClick={() => setSide("symbols")}>Symbols</button><button classList={{ on: side() === "plant" }} onClick={() => setSide("plant")}>Plant</button><Help k={side() === "plant" ? "plant" : "palette"} /></header>
      <Show when={side() === "plant"}><PlantPanel /></Show>
      <Show when={side() === "symbols"}>
      <input class="psearch" placeholder="Find a symbol (ball valve, pump…)" value={q()} onInput={e => setQ(e.currentTarget.value)} />
      <div class="pbody">
        <For each={groups()}>{g => <section>
          <button class="pcat" onClick={() => setOpen({ ...open(), [g.c]: !(open()[g.c] || !!q()) })}><span>{open()[g.c] || q() ? "▾" : "▸"}</span>{g.name}<small>{g.syms.length}</small></button>
          <Show when={open()[g.c] || q()}>
            <div class="pgrid">
              <For each={g.syms}>{s => <button class="psym" classList={{ on: on(s.key), rare: s.rare }} draggable={true} title={`${s.name}${s.inline ? " (goes in a line)" : ""}`} onDragStart={e => drag(e, s.key)} onClick={() => pick(s.key)}>
                <SymPic s={s} /><span>{s.name}</span></button>}</For>
            </div>
          </Show>
        </section>}</For>
        <Show when={presets().length}><section>
          <button class="pcat" onClick={() => setOpen({ ...open(), presets: !open().presets })}><span>{open().presets || q() ? "▾" : "▸"}</span>Common instruments<small>{presets().length}</small></button>
          <Show when={open().presets || q()}>
            <div class="pgrid presets"><For each={presets()}>{([l, k, name]) => <Show when={sym(k)}>{s => <button class="psym" classList={{ on: on(k, l) }} draggable={true} title={`${name} (${l})`} onDragStart={e => drag(e, k, l)} onClick={() => pick(k, l)}><SymPic s={s()} letters={l} /><span>{name}</span></button>}</Show>}</For></div>
          </Show>
        </section></Show>
        <Show when={!groups().length && !presets().length}><p class="muted pnone">No symbol is called that.</p></Show>
      </div>
      </Show>
    </aside>
  );
};
