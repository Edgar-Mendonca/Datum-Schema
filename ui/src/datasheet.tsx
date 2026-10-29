// Datasheets: one per instrument, control valve or relief device, made by the engine from the
// object's class attributes and the process conditions of its line. The object's own values are
// typed here (they are the same data as the Inspector's); the line's values are shown with where
// they come from and are edited on the line. Print and Excel export cover every datasheet.
import { For, Show, createResource } from "solid-js";
import { Datasheet, DsRow, Val, parseNum } from "./api";
import { call, act, proj, openDialog, closeDialog, setBackstage, toast } from "./store";
import { fromU } from "./units";
import { Dialog, Pick } from "./ui";
import { C } from "./cmds";
import { setPrintOnly } from "./print";

const Cell = (p: { r: DsRow; obj: number }) => {
  const set = (t: string) => {
    let val: Val | null = t.trim() === "" ? null : t;
    if (p.r.q && val !== null) {
      const x = parseNum(t);
      if (x === null) { toast(`${p.r.label}: type a number.`); return; }
      val = fromU(p.r.q, x);
    }
    act({ SetData: { ids: [p.obj], key: p.r.key, val } });
  };
  return (
    <Show when={p.r.key} fallback={<span>{p.r.value || <span class="muted">—</span>}</span>}>
      <Show when={p.r.choices.length} fallback={<input value={p.r.value} class={p.r.q ? "mono" : undefined} onChange={e => set(e.currentTarget.value)} />}>
        <Pick value={p.r.value} set={set} options={[["", "—"], ...p.r.choices.filter(c => c).map(c => [c, c] as [string, string])]} />
      </Show>
    </Show>
  );
};

/** A datasheet's sections as a table; `edit` makes the object's own values typable. */
export const DatasheetTable = (p: { d: Datasheet; edit?: boolean }) => (
  <table class="ds">
    <For each={p.d.sections}>{s => <tbody>
      <tr class="dsh"><th colspan="4">{s.title}</th></tr>
      <For each={s.rows}>{r => <tr classList={{ derived: !!r.source }}>
        <td>{r.label}{r.required ? " *" : ""}</td>
        <td>{p.edit ? <Cell r={r} obj={p.d.obj} /> : r.value}</td>
        <td class="u">{r.unit}</td>
        <td class="src">{r.source ? `from ${r.source}` : ""}</td>
      </tr>}</For>
    </tbody>}</For>
  </table>
);

export const openDatasheet = (id: number) => openDialog(() => {
  const [d] = createResource(() => [id, proj()] as const, ([i]) => call<Datasheet>("datasheet", { id: i }));
  return (
    <Dialog title={d() ? `${d()!.title} · ${d()!.tag}` : "Datasheet"} wide help="datasheets"
      extra={<>
        <button class="btn" onClick={() => { const x = d(); if (x) { closeDialog(); C.goTo([x.item], x.sheet); } }}>Go to the symbol</button>
        <button class="btn" onClick={() => { setPrintOnly(id); closeDialog(); setBackstage("print"); }}>Print…</button>
        <button class="btn" onClick={C.exportDatasheets}>Excel (every datasheet)</button>
      </>}>
      <Show when={d()} fallback={<p class="muted">Making the datasheet…</p>}>
        <p class="muted small">Type the instrument's own values here or in the Inspector; grey rows come from the line or the drawing and change there.</p>
        <DatasheetTable d={d()!} edit />
      </Show>
    </Dialog>
  );
});
