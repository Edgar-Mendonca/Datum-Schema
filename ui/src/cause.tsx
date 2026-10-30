// The cause-and-effect matrix, read by the engine (cause.rs) from the interlocks on the P&ID:
// causes down (the instruments that trip), effects across (the valves and drives acted on, with
// what they do), and in each cell the interlock that links them. Print it or export it to Excel.
import { For, Show, createResource } from "solid-js";
import { CauseMatrix } from "./api";
import { call, proj, openDialog, closeDialog, setBackstage, toast } from "./store";
import { Dialog } from "./ui";
import { C } from "./cmds";
import { setPrintCause } from "./print";

/** The matrix as a table (on screen with links to the drawing, or on the printed page). */
export const MatrixTable = (p: { m: CauseMatrix; go?: (item: number, sheet: number) => void }) => (
  <table class="cem">
    <thead>
      <tr><th colspan="3" class="corner">Causes ↓ · Effects →</th><For each={p.m.effects}>{e => <th class="eff"><div><b onClick={() => p.go?.(e.item, e.sheet)}>{e.tag}</b><span>{e.what}</span></div></th>}</For></tr>
      <tr><th>Tag</th><th>Trip</th><th>Setting</th><For each={p.m.effects}>{e => <th class="act">{e.action}</th>}</For></tr>
    </thead>
    <tbody><For each={p.m.causes}>{(c, r) => <tr>
      <td class="mono"><b onClick={() => p.go?.(c.item, c.sheet)}>{c.tag}</b></td><td>{c.what}</td><td>{c.setting}</td>
      <For each={p.m.cells[r()]}>{v => <td class="x" classList={{ on: !!v }}>{v}</td>}</For>
    </tr>}</For></tbody>
  </table>
);

export const openCause = () => openDialog(() => {
  const [m] = createResource(() => proj(), () => call<CauseMatrix>("cause_effect"));
  const go = (item: number, sheet: number) => { closeDialog(); C.goTo([item], sheet); };
  const has = () => !!m() && m()!.causes.length > 0 && m()!.effects.length > 0;
  return (
    <Dialog title="Cause and effect" wide help="cause"
      extra={<>
        <button class="btn" disabled={!has()} onClick={() => { setPrintCause(true); closeDialog(); setBackstage("print"); }}>Print…</button>
        <button class="btn" disabled={!has()} onClick={async () => { const f = await call<string | null>("export", { kind: "cause", what: "", sheet: 0 }); if (f) toast(`Saved ${f}.`, true); }}>Excel</button>
      </>}>
      <Show when={m()} fallback={<p class="muted">Reading the interlocks…</p>}>
        <Show when={has()} fallback={<p class="muted">No interlock with a cause and an effect yet. Draw an interlock symbol (Instruments › Interlock logic) and join it with signal lines to the instruments that trip it and to the valves or drives it acts on; a switch (PSHH, LSLL…) wired straight to a valve counts too.</p>}>
          <div class="cemwrap"><MatrixTable m={m()!} go={go} /></div>
          <p class="muted small">Click a tag to find it on the drawing. A valve does what its fail position says (FC: close, FO: open); a drive stops. An X marks a switch wired straight to its effect.</p>
        </Show>
        <Show when={m()!.open.length}><p class="small warnline">{m()!.open.join(" ")}</p></Show>
      </Show>
    </Dialog>
  );
});
