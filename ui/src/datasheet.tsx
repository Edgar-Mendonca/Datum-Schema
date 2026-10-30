// Datasheets: one per instrument, control valve or relief device, made by the engine from the
// object's class attributes and the process conditions of its line. The object's own values are
// typed here (they are the same data as the Inspector's); the line's values are shown with where
// they come from and are edited on the line. Print and Excel export cover every datasheet.
// A datasheet is issued with a revision and a status; values changed since the last issue are
// marked Δ, and the revision block lists every issue.
import { For, Show, createResource, createSignal } from "solid-js";
import { Datasheet, DsRow, Val, parseNum, STATUSES } from "./api";
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
      <tr class="dsh"><th colspan="5">{s.title}</th></tr>
      <For each={s.rows}>{r => <tr classList={{ derived: !!r.source, chg: r.changed }}>
        <td>{r.label}{r.required ? " *" : ""}</td>
        <td>{p.edit ? <Cell r={r} obj={p.d.obj} /> : r.value}</td>
        <td class="u">{r.unit}</td>
        <td class="src">{r.source ? `from ${r.source}` : ""}</td>
        <td class="dlt" title={r.changed ? `Rev ${p.d.rev}: ${r.was}` : undefined}>{r.changed ? "Δ" : ""}</td>
      </tr>}</For>
    </tbody>}</For>
  </table>
);

/** The revision block: every issue, newest first. */
export const RevBlock = (p: { d: Datasheet }) => (
  <Show when={p.d.issues.length}>
    <table class="ds revs">
      <thead><tr class="dsh"><th>Rev</th><th>Status</th><th>Date</th><th>By</th><th>Note</th></tr></thead>
      <tbody><For each={[...p.d.issues].reverse()}>{i => <tr><td class="mono">{i.rev}</td><td>{i.status}</td><td>{i.date}</td><td>{i.by}</td><td>{i.note}</td></tr>}</For></tbody>
    </table>
  </Show>
);

/** The revision state in words: "Rev A · Preliminary · 2026-10-30 · Δ 2 changed since". */
export const revLine = (d: Datasheet) => d.rev ? `Rev ${d.rev} · ${d.status} · ${d.date}${d.changed ? ` · Δ ${d.changed} changed since` : ""}` : "Not issued yet";

/** Today as the person reads it (YYYY-MM-DD, local time). */
const today = () => { const t = new Date(); return `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, "0")}-${String(t.getDate()).padStart(2, "0")}`; };

/**
 * Issue datasheets: one (`d` given: its revision can be typed) or several (each takes its next
 * revision for the status).
 */
export const issueDialog = (ids: number[], d?: Datasheet) => openDialog(() => {
  const [status, setStatus] = createSignal(d?.status && d.status !== "As-built" ? STATUSES[Math.min(STATUSES.indexOf(d.status) + 1, 3)] : STATUSES[0]);
  const [typed, setTyped] = createSignal<string | null>(null);
  const [note, setNote] = createSignal("");
  const rev = () => typed() ?? d?.next_rev[status()] ?? "";
  const ok = async () => !!(await act({ IssueDatasheet: { ids, rev: d ? rev() : "", status: status(), date: today(), note: note() } }));
  return (
    <Dialog title={d ? `Issue the datasheet of ${d.tag}` : `Issue ${ids.length} datasheets`} ok={ok} okText="Issue" help="datasheets">
      <div class="form">
        <label>Status</label><div><Pick value={status()} set={v => { setStatus(v); setTyped(null); }} options={STATUSES} /></div>
        <Show when={d} fallback={<><label>Revision</label><div class="muted small">Each takes its next revision: letters (A, B…) before purchase, numbers (0, 1…) from purchase on.</div></>}>
          <label>Revision</label><div><input class="mono" value={rev()} onInput={e => setTyped(e.currentTarget.value)} /></div>
        </Show>
        <label>Note</label><div><input value={note()} placeholder="What this issue is for" onInput={e => setNote(e.currentTarget.value)} /></div>
        <label>Date</label><div class="muted">{today()}</div>
      </div>
      <p class="muted small">The issue keeps what the datasheet says now; later changes are marked Δ until the next issue.</p>
    </Dialog>
  );
});

export const openDatasheet = (id: number) => openDialog(() => {
  const [d] = createResource(() => [id, proj()] as const, ([i]) => call<Datasheet>("datasheet", { id: i }));
  return (
    <Dialog title={d() ? `${d()!.title} · ${d()!.tag}` : "Datasheet"} wide help="datasheets"
      extra={<>
        <button class="btn pri" onClick={() => { const x = d(); if (x) issueDialog([x.obj], x); }}>Issue…</button>
        <button class="btn" onClick={() => { const x = d(); if (x) { closeDialog(); C.goTo([x.item], x.sheet); } }}>Go to the symbol</button>
        <button class="btn" onClick={() => { setPrintOnly(id); closeDialog(); setBackstage("print"); }}>Print…</button>
        <button class="btn" onClick={C.exportDatasheets}>Excel (every datasheet)</button>
      </>}>
      <Show when={d()} fallback={<p class="muted">Making the datasheet…</p>}>
        <p class="dsrev"><b>{revLine(d()!)}</b>{d()!.by ? <span class="muted"> · issued by {d()!.by}</span> : ""}</p>
        <p class="muted small">Type the instrument's own values here or in the Inspector; grey rows come from the line or the drawing and change there. Δ marks a value changed since the last issue (hover for what it was).</p>
        <DatasheetTable d={d()!} edit />
        <RevBlock d={d()!} />
      </Show>
    </Dialog>
  );
});
