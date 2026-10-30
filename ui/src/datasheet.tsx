// Datasheets: one per instrument, control valve or relief device, made by the engine from the
// object's class attributes and the process conditions of its line. The object's own values are
// typed here (they are the same data as the Inspector's); the line's values are shown with where
// they come from and are edited on the line. Print and Excel export cover every datasheet.
// A datasheet is issued with a revision and a status; values changed since the last issue are
// marked Δ, and the revision block lists every issue.
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { Datasheet, DsRow, DsLayout, ImportPlan, Val, parseNum, STATUSES } from "./api";
import { call, act, proj, openDialog, closeDialog, setBackstage, toast, opts, setOpts } from "./store";
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
        <Show when={d()!.logo || d()!.form}><p class="dsco"><Show when={d()!.logo}><img class="dslogo" src={d()!.logo} alt="" /></Show><span class="muted small">{d()!.form ? `Form ${d()!.form}` : ""}</span></p></Show>
        <p class="dsrev"><b>{revLine(d()!)}</b>{d()!.by ? <span class="muted"> · issued by {d()!.by}</span> : ""}</p>
        <p class="muted small">Type the instrument's own values here or in the Inspector; grey rows come from the line or the drawing and change there. Δ marks a value changed since the last issue (hover for what it was).</p>
        <DatasheetTable d={d()!} edit />
        <RevBlock d={d()!} />
        <Show when={d()!.footer}><p class="muted small">{d()!.footer}</p></Show>
      </Show>
    </Dialog>
  );
});

/** Import datasheet values from an Excel workbook: the engine's plan is shown first; only the
 *  ticked values are written, as one edit that Undo takes back. */
export const importDatasheets = async () => {
  const plan = await call<ImportPlan | null>("import_datasheets");
  if (!plan) return;
  openDialog(() => {
    const sets = plan.changes.filter(c => c.kind === "set");
    const [on, setOn] = createSignal<boolean[]>(sets.map(() => true));
    const n = createMemo(() => on().filter(Boolean).length);
    const others = plan.changes.filter(c => c.kind === "conflict" || c.kind === "bad");
    const same = plan.changes.filter(c => c.kind === "same").length;
    const ok = async () => {
      const ops = sets.filter((_, i) => on()[i]).map(c => ({ SetData: { ids: [c.obj], key: c.key, val: c.val } }));
      if (!ops.length) return true;
      const r = await act({ Batch: { ops } });
      if (r) toast(`Imported ${ops.length} ${ops.length === 1 ? "value" : "values"}. Undo takes ${ops.length === 1 ? "it" : "them all"} back.`, true);
      return !!r;
    };
    return (
      <Dialog title="Import datasheets from Excel" wide ok={ok} okText={`Import ${n()} ${n() === 1 ? "value" : "values"}`} help="import">
        <p class="imsum">
          <b>{sets.length}</b> {sets.length === 1 ? "value" : "values"} to import · {same} already the same · <span classList={{ warnline: others.length > 0 }}>{others.length} not imported</span> · {plan.used} of {plan.sheets} worksheets matched a datasheet
        </p>
        <Show when={sets.length}>
          <h4>To import <button class="btn sm" onClick={() => setOn(on().map(() => !on().every(Boolean)))}>{on().every(Boolean) ? "Untick all" : "Tick all"}</button></h4>
          <table class="tbl small imtbl"><thead><tr><th /><th>Tag</th><th>Value</th><th>Now</th><th>From the file</th></tr></thead><tbody>
            <For each={sets}>{(c, i) => <tr><td><input type="checkbox" checked={on()[i()]} onChange={e => setOn(on().map((x, j) => (j === i() ? e.currentTarget.checked : x)))} /></td><td class="mono">{c.tag}</td><td>{c.label}</td><td class="muted">{c.old || "—"}</td><td><b>{c.new}</b></td></tr>}</For>
          </tbody></table>
        </Show>
        <Show when={others.length}>
          <h4>Not imported</h4>
          <table class="tbl small imtbl"><thead><tr><th>Tag</th><th>Value</th><th>Now</th><th>The file says</th><th>Why</th></tr></thead><tbody>
            <For each={others}>{c => <tr class={c.kind}><td class="mono">{c.tag}</td><td>{c.label}</td><td>{c.old || "—"}</td><td>{c.new}</td><td class="muted">{c.kind === "conflict" ? c.note : c.note}</td></tr>}</For>
          </tbody></table>
        </Show>
        <Show when={plan.unmatched.length}><p class="small"><b>Tags with no datasheet in this project:</b> {plan.unmatched.join(", ")}</p></Show>
        <Show when={plan.unknown.length}><p class="small"><b>Columns or rows not recognised:</b> {plan.unknown.join(", ")} <span class="muted">(add them as attributes in Options › Classes, then import again)</span></p></Show>
        <p class="muted small">Values that come from the line, the vessel or the drawing are never imported: change them there. Blank cells change nothing.</p>
      </Dialog>
    );
  });
};

/** Options › Datasheets: the company layout (form number, logo, footer, rows shown, section order). */
export const LayoutSec = () => {
  const [rows] = createResource(() => proj(), () => call<[string, string[]][]>("layout_rows"));
  const l = (): DsLayout => proj()!.settings.ds_layout;
  const set = (x: Partial<DsLayout>) => act({ SetSettings: { settings: { ...proj()!.settings, ds_layout: { ...l(), ...x } } } });
  const key = (s: string, r: string) => `${s} › ${r}`;
  const order = () => {
    const names = (rows() ?? []).map(r => r[0]);
    return [...names].sort((a, b) => { const i = l().order.indexOf(a), j = l().order.indexOf(b); return (i < 0 ? 999 : i) - (j < 0 ? 999 : j); });
  };
  const move = (name: string, d: number) => { const o = order(); const i = o.indexOf(name); const j = i + d; if (j < 0 || j >= o.length) return; [o[i], o[j]] = [o[j], o[i]]; set({ order: o }); };
  const logo = (f: File | undefined) => {
    if (!f) return;
    if (!/^image\/(png|jpeg|svg\+xml)$/.test(f.type)) return toast("Choose a PNG, JPEG or SVG image.");
    if (f.size > 200 * 1024) return toast("The logo is larger than 200 KB: save a smaller copy and choose it again.");
    const r = new FileReader(); r.onload = () => set({ logo: String(r.result) }); r.readAsDataURL(f);
  };
  return (
    <div class="dslayout">
      <div class="form">
        <label>Form number</label><div><input value={l().form} placeholder="e.g. IDS-001 rev 2" onChange={e => set({ form: e.currentTarget.value })} /></div>
        <label>Logo</label><div class="inline"><Show when={l().logo}><img class="dslogo" src={l().logo} alt="" /></Show><label class="btn sm">Choose…<input type="file" accept="image/png,image/jpeg,image/svg+xml" hidden onChange={e => logo(e.currentTarget.files?.[0])} /></label><Show when={l().logo}><button class="btn sm" onClick={() => set({ logo: "" })}>Remove</button></Show></div>
        <label>Footer</label><div><input value={l().footer} placeholder="A line at the foot of every datasheet" onChange={e => set({ footer: e.currentTarget.value })} /></div>
      </div>
      <Show when={(rows() ?? []).length} fallback={<p class="muted small">Draw an instrument to choose the rows its datasheet shows.</p>}>
        <p class="muted small">Untick the rows your company's form leaves out; move sections up or down. Hidden rows keep their data and are still imported and issued.</p>
        <div class="dsrows"><For each={order()}>{name => {
          const labels = () => (rows() ?? []).find(r => r[0] === name)?.[1] ?? [];
          return <div class="dssec">
            <div class="dsech"><b>{name}</b><button class="btn sm" title="Move up" onClick={() => move(name, -1)}>↑</button><button class="btn sm" title="Move down" onClick={() => move(name, 1)}>↓</button></div>
            <For each={labels()}>{r => <label class="chk"><input type="checkbox" checked={!l().hidden.includes(key(name, r))} onChange={e => set({ hidden: e.currentTarget.checked ? l().hidden.filter(h => h !== key(name, r)) : [...l().hidden, key(name, r)] })} />{r}</label>}</For>
          </div>;
        }}</For></div>
      </Show>
      <p><button class="btn sm" onClick={() => { const o = { ...opts(), defaults: { ...(opts().defaults ?? proj()!.settings), ds_layout: l() } }; setOpts(o); call("set_options", { opts: o }).then(() => toast("New projects and the examples will use this datasheet layout.", true)); }}>Use this layout for new projects</button></p>
    </div>
  );
};
