// The Plant panel: the plant hierarchy (plant › unit › area) with every object in it, drawn or
// not. Click an object to go to its symbol; drag it onto a sheet to show it there (an object can
// appear on several sheets); drag it onto an area to move it; add, rename and remove levels.
import { For, Show, createSignal, createMemo } from "solid-js";
import { Obj, PlantNode } from "./api";
import { proj, derived, act, lib, symOf, toast, openMenu, setSel, setTool } from "./store";
import { C } from "./cmds";
import { Help } from "./help";
import * as D from "./dialogs";
import { openDatasheet } from "./datasheet";

/** Where an object is shown: its symbols, or for a line number its drawn pieces. */
export const viewsOf = (o: Obj) => {
  const p = proj(); if (!p) return [] as { id: number; sheet: number }[];
  return [...p.items.filter(i => i.obj === o.id), ...p.lines.filter(l => l.obj === o.id)].map(v => ({ id: v.id, sheet: v.sheet }));
};
/** The symbol to draw an object with: the one it is drawn with already, else its class's first. */
export const symbolFor = (o: Obj) => proj()?.items.find(i => i.obj === o.id)?.sym ?? lib()?.classes.find(c => c.name === o.class)?.symbols[0];
export const labelOf = (o: Obj) => o.tag || (o.class === "PipingNetworkSystem" ? (proj()?.lines.find(l => l.obj === o.id) ? derived()?.numbers[proj()!.lines.find(l => l.obj === o.id)!.id] : "") || "Line without a number" : lib()?.classes.find(c => c.name === o.class)?.label ?? o.class);

export const PlantPanel = () => {
  const [q, setQ] = createSignal("");
  const [open, setOpen] = createSignal<Record<number, boolean>>({});
  const [editing, setEditing] = createSignal<number | null>(null);
  const nodes = () => proj()?.nodes ?? [];
  const kids = (id: number) => nodes().filter(n => n.parent === id);
  const objs = createMemo(() => {
    const s = q().trim().toLowerCase();
    return (proj()?.objects ?? []).filter(o => !s || labelOf(o).toLowerCase().includes(s) || o.class.toLowerCase().includes(s));
  });
  const inArea = (id: number) => objs().filter(o => o.node === id).sort((a, b) => labelOf(a).localeCompare(labelOf(b), undefined, { numeric: true }));
  const isOpen = (id: number) => open()[id] ?? true;
  const go = (o: Obj) => {
    const v = viewsOf(o);
    if (!v.length) return toast(`${labelOf(o)} is not drawn yet: drag it onto a sheet.`);
    C.goTo([v[0].id], v[0].sheet);
  };
  const addNode = (level: "plant" | "unit" | "area", parent: number) => act({ AddNode: { node: { id: 0, parent, level, name: "" } } });
  const rename = (n: PlantNode, name: string) => { setEditing(null); if (name.trim() && name !== n.name) act({ SetNode2: { node: { ...n, name } } }); };
  const menu = (e: MouseEvent, n: PlantNode) => openMenu(e, [
    ...(n.level === "plant" ? [{ label: "Add a unit", run: () => addNode("unit", n.id) }] : n.level === "unit" ? [{ label: "Add an area", run: () => addNode("area", n.id) }] : []),
    { label: "Rename", run: () => setEditing(n.id) }, "-",
    { label: "Delete", icon: "del", run: () => act({ DeleteNode: { id: n.id } }) },
  ]);
  const objMenu = (e: MouseEvent, o: Obj) => openMenu(e, [
    { label: "Go to its symbol", run: () => go(o), off: !viewsOf(o).length },
    { label: "Datasheet…", run: () => openDatasheet(o.id), off: !viewsOf(o).length || !lib()?.classes.some(c => c.name === o.class && c.symbols.some(k => ["instrument", "primary", "control"].includes(symOf(k)?.cat ?? ""))) },
    { label: "Draw it on this sheet", run: () => { const s = symbolFor(o); if (s) setTool({ k: "place", sym: s, obj: o.id }); }, off: o.class === "PipingNetworkSystem" || !symbolFor(o) },
    "-", { label: "Delete the object (and its symbols)", icon: "del", run: () => D.confirm(`Delete ${labelOf(o)} and every symbol that shows it?`, "Delete", () => act({ DeleteObjects: { ids: [o.id] } }).then(r => r && setSel([]))) },
  ]);
  const drop = (e: DragEvent, area: number) => { const id = Number(e.dataTransfer?.getData("text/x-datum-obj")); if (id) { e.preventDefault(); act({ SetNode: { ids: [id], node: area } }); } };
  const Node = (p: { n: PlantNode; depth: number }) => (
    <div class="pnode">
      <div class={`pnrow ${p.n.level}`} style={{ "padding-left": `${p.depth * 12 + 4}px` }} onContextMenu={e => menu(e, p.n)}
        onDragOver={e => p.n.level === "area" && e.preventDefault()} onDrop={e => p.n.level === "area" && drop(e, p.n.id)}>
        <button class="tw" onClick={() => setOpen({ ...open(), [p.n.id]: !isOpen(p.n.id) })}>{isOpen(p.n.id) ? "▾" : "▸"}</button>
        <Show when={editing() === p.n.id} fallback={<span class="pnname" onDblClick={() => setEditing(p.n.id)}>{p.n.name}</span>}>
          <input class="pnedit" value={p.n.name} ref={el => setTimeout(() => el.select())} onKeyDown={e => { if (e.key === "Enter") rename(p.n, e.currentTarget.value); if (e.key === "Escape") setEditing(null); }} onBlur={e => rename(p.n, e.currentTarget.value)} />
        </Show>
        <small>{p.n.level === "area" ? inArea(p.n.id).length : ""}</small>
      </div>
      <Show when={isOpen(p.n.id)}>
        <For each={kids(p.n.id)}>{k => <Node n={k} depth={p.depth + 1} />}</For>
        <Show when={p.n.level === "area"}>
          <For each={inArea(p.n.id)}>{o => {
            const v = () => viewsOf(o).length;
            const s = () => symbolFor(o);
            return <div class="porow" style={{ "padding-left": `${p.depth * 12 + 22}px` }} classList={{ undrawn: !v() }} draggable={true}
              onDragStart={e => { e.dataTransfer?.setData("text/x-datum-obj", String(o.id)); const sy = s(); if (sy && o.class !== "PipingNetworkSystem") e.dataTransfer?.setData("text/x-datum-sym", sy); }}
              onClick={() => go(o)} onContextMenu={e => objMenu(e, o)} title={`${lib()?.classes.find(c => c.name === o.class)?.label ?? o.class}${v() ? ` · shown ${v()}×` : " · not drawn"}`}>
              <span class="ptag">{labelOf(o)}</span><small>{v() ? (v() > 1 ? `${v()}×` : "") : "not drawn"}</small>
            </div>;
          }}</For>
        </Show>
      </Show>
    </div>
  );
  return (
    <div class="plant">
      <div class="pltools">
        <input class="psearch" placeholder="Find an object (tag, class…)" value={q()} onInput={e => setQ(e.currentTarget.value)} />
        <button class="btn sm" title="Add a plant" onClick={() => addNode("plant", 0)}>+ Plant</button>
      </div>
      <div class="pbody"><For each={kids(0)}>{n => <Node n={n} depth={0} />}</For></div>
      <p class="muted small plhint">Drag an object onto a sheet to draw it there, or onto an area to move it. Right-click for more. <Help k="plant" /></p>
    </div>
  );
};
