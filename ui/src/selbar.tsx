// What can be done with the selection (0.11.5): one list of commands, shown two ways. The ribbon's
// Selection tab (coloured, named for what is picked: Symbol, Line) holds them all; the quick bar by
// the selection (View › Quick bar, on by default) holds the common ones, so Rotate, Mirror, Reverse
// flow or the datasheet are one click away without looking for them.
import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { sel, itemOf, lineOf, symOf, tool, act, activeDoc, backstage, setSide, setPanel, setListOpen, dialogs, quickBar } from "./store";
import { C } from "./cmds";
import { Icon } from "./ui";
import { openDatasheet } from "./datasheet";
import { openLoops } from "./loops";

export type SelKind = "" | "symbol" | "symbols" | "line" | "lines" | "mixed";
export const selKind = (): SelKind => {
  const s = sel(); if (!s.length) return "";
  const its = s.every(id => itemOf(id)), lns = s.every(id => lineOf(id));
  if (s.length === 1) return its ? "symbol" : lns ? "line" : "mixed";
  return its ? "symbols" : lns ? "lines" : "mixed";
};
/** The Selection tab's name and caption, by what is picked. */
export const selTab = (): [string, string] => {
  const k = selKind();
  return k === "symbol" || k === "symbols" ? ["Symbol", "Symbol tools"] : k === "line" || k === "lines" ? ["Line", "Line tools"] : ["Arrange", "Selection"];
};

export type SelAct = { k: string; i: string; l: string; run: () => void; keys?: string; group: string; bar?: boolean; off?: boolean };
const LIST_OF: Record<string, string> = { vessel: "equipment", rotating: "equipment", heat: "equipment", valve: "valves", control: "valves", instrument: "instruments", primary: "instruments" };
const showIn = (k: string) => { setPanel(k); setListOpen(true); };

/** Every command for the selection, grouped as the ribbon shows them; `bar` marks the quick bar's. */
export const selActions = (): SelAct[] => {
  const s = sel(), kind = selKind(); if (!kind) return [];
  const it = kind === "symbol" ? itemOf(s[0]) : undefined, ln = kind === "line" ? lineOf(s[0]) : undefined;
  const sym = it ? symOf(it.sym) : undefined, cat = sym?.cat ?? "";
  const a: SelAct[] = [];
  if (kind === "symbol" || kind === "symbols" || kind === "mixed") {
    a.push({ k: "rotate", i: "rotate", l: "Rotate", keys: "R", run: C.rotate, group: "Arrange", bar: true });
    a.push({ k: "flip", i: "flip", l: "Mirror", keys: "M", run: C.flip, group: "Arrange", bar: true });
  }
  if (it && sym?.inline) a.push({ k: "turn", i: "flow", l: "Turn Round", run: () => act({ TurnRound: { id: it.id } }), group: "Arrange", bar: true });
  if (ln) {
    a.push({ k: "reverse", i: "flow", l: "Reverse Flow", run: () => C.reverse(ln.id), group: "Flow", bar: true });
    a.push({ k: "straight", i: "line", l: "Straighten", run: () => act({ SetPts: { id: ln.id, pts: [] } }), off: !ln.pts.length, group: "Flow", bar: true });
    a.push({ k: "run", i: "select", l: "Whole Line Number", run: () => C.selectRun(ln.id), group: "Flow", bar: true });
  }
  a.push({ k: "dup", i: "dup", l: "Duplicate", keys: "Ctrl+D", run: C.duplicate, group: "Edit", bar: kind !== "line" && kind !== "lines" });
  a.push({ k: "copy", i: "copy", l: "Copy", keys: "Ctrl+C", run: C.copy, group: "Edit" });
  a.push({ k: "del", i: "del", l: "Delete", keys: "Del", run: C.del, group: "Edit", bar: true });
  if (s.length === 1) a.push({ k: "props", i: "info", l: "Properties", run: () => setSide("inspector"), group: "Data", bar: true });
  if (it && ["instrument", "primary", "control", "vessel", "rotating", "heat"].includes(cat)) a.push({ k: "ds", i: "report", l: "Datasheet", run: () => openDatasheet(it.obj), group: "Data", bar: true });
  if (it && ["instrument", "primary", "control"].includes(cat)) a.push({ k: "loop", i: "sig", l: "Loop Diagram", run: () => openLoops(String(it.obj)), group: "Data" });
  if (it && LIST_OF[cat]) a.push({ k: "list", i: "table", l: "Show in the List", run: () => showIn(LIST_OF[cat]), group: "Data" });
  if (ln) a.push({ k: "list", i: "table", l: "Show in the Line List", run: () => showIn("lines"), group: "Data" });
  if (it && cat === "connector") {
    a.push({ k: "partner", i: "offpage", l: "Go to Partner", run: () => C.goPartner(it.id), group: "Off-page", bar: true });
    a.push({ k: "pair", i: "link", l: "Pair With…", run: () => C.pairDialog(it.id), group: "Off-page" });
  }
  return a;
};

/** The quick bar, by the selection on the sheet. It keeps out of the way: hidden while the mouse
 *  button is down (dragging, drawing), while another tool is active, and over a tab or a window. */
export const QuickBar = () => {
  const [at, setAt] = createSignal<{ x: number; y: number } | null>(null);
  const [down, setDown] = createSignal(false);
  let raf = 0, el: HTMLDivElement | undefined;
  const md = (e: PointerEvent) => { if (!(e.target as HTMLElement).closest(".qbar")) setDown(true); };
  const mu = () => setDown(false);
  const show = () => quickBar() && tool().k === "select" && !down() && !activeDoc() && !backstage() && !dialogs().length && sel().length > 0 && sel().length <= 40;
  const place = () => {
    raf = requestAnimationFrame(place);
    if (!show()) { if (at()) setAt(null); return; }
    const canvas = document.querySelector(".canvhold .canvas")?.getBoundingClientRect();
    const rs = [...document.querySelectorAll(".dsvg .it.sel, .dsvg .ln.sel .lstroke, .dsvg .note.sel")].map(e => e.getBoundingClientRect()).filter(r => r.width + r.height > 0);
    if (!canvas || !rs.length) { if (at()) setAt(null); return; }
    const top = Math.min(...rs.map(r => r.top)), bot = Math.max(...rs.map(r => r.bottom)), l = Math.min(...rs.map(r => r.left)), r = Math.max(...rs.map(r => r.right));
    const w = el?.offsetWidth ?? 200, h = el?.offsetHeight ?? 30;
    let y = top - h - 10; if (y < canvas.top + 4) y = bot + 10;
    if (y + h > canvas.bottom - 4) y = Math.max(canvas.top + 4, top - h - 10);
    const x = Math.min(Math.max((l + r) / 2 - w / 2, canvas.left + 4), canvas.right - w - 4);
    const cur = at(); if (!cur || Math.abs(cur.x - x) > 0.5 || Math.abs(cur.y - y) > 0.5) setAt({ x, y });
  };
  onMount(() => { document.addEventListener("pointerdown", md, true); document.addEventListener("pointerup", mu, true); raf = requestAnimationFrame(place); });
  onCleanup(() => { document.removeEventListener("pointerdown", md, true); document.removeEventListener("pointerup", mu, true); cancelAnimationFrame(raf); });
  return (
    <Show when={at()}>{p => (
      <div class="qbar" ref={el} style={{ left: `${p().x}px`, top: `${p().y}px` }} onMouseDown={e => e.stopPropagation()}>
        <For each={selActions().filter(x => x.bar)}>{x => <button disabled={x.off} title={`${x.l}${x.keys ? ` (${x.keys})` : ""}`} data-k={x.k} classList={{ danger: x.k === "del" }} onClick={() => { x.run(); setTimeout(() => import("./ribbon").then(m => m.backToSheet()), 0); }}><Icon n={x.i} /></button>}</For>
        <button class="qmore" title="Everything for the selection: the Selection tab on the ribbon" onClick={() => import("./ribbon").then(m => m.pickTab("format"))}>⋯</button>
      </div>
    )}</Show>
  );
};
