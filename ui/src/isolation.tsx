// The isolation planner (engine: isolation.rs): for the selected equipment, valve or line, the
// valves and blinds that isolate it, the vents and drains inside, and what stops a safe isolation.
import { For, Show, createResource } from "solid-js";
import { call, openDialog, closeDialog, sel, setSel, proj, toast } from "./store";
import { Dialog } from "./ui";

interface Point { item: number; tag: string; what: string; action: string; normal: string; positive: boolean; note: string; role: string }
interface Plan { target: string; points: Point[]; lines: string[]; inside: number[]; problems: string[]; notes: string[]; steps: string[] }

const ROLE: Record<string, string> = { boundary: "Boundary", vent: "Vent or drain", relief: "Relief" };

export const planIsolation = (ids = sel()) => {
  if (!ids.length) return toast("Select the equipment, valve or line to isolate first.");
  openDialog(() => {
    const [plan] = createResource(() => call<Plan>("isolation", { ids }));
    const show = () => { const p = plan(); if (!p) return; setSel([...p.inside, ...p.points.map(x => x.item)].filter(i => proj()?.items.some(x => x.id === i) || proj()?.lines.some(x => x.id === i))); closeDialog(); };
    const copy = () => {
      const p = plan(); if (!p) return;
      const rows = [`Isolation of ${p.target}`, "", ["Tag", "What", "Role", "Action", "Normal", "Note"].join("\t"), ...p.points.map(x => [x.tag, x.what, ROLE[x.role], x.action, x.normal, x.note].join("\t")), "", ...p.steps, "", ...p.problems.map(x => `Problem: ${x}`)];
      navigator.clipboard?.writeText(rows.join("\n")).then(() => toast("Copied: paste it into a spreadsheet or a permit.", true), () => toast("Could not copy."));
    };
    return (
      <Dialog title="Isolation plan" wide help="isolation" extra={<><button class="btn" onClick={show}>Show on the sheet</button><button class="btn" onClick={copy}>Copy</button></>}>
        <Show when={plan()} fallback={<p class="muted">Planning…</p>}>{p => <>
          <p><b>Isolating {p().target}</b><span class="muted small"> · lines inside: {p().lines.join(", ") || "none"}</span></p>
          <table class="tbl small isotbl"><thead><tr><th>Tag</th><th>What</th><th>Role</th><th>Action</th><th>Normal</th><th>Note</th></tr></thead><tbody>
            <For each={p().points}>{x => <tr classList={{ warn: !!x.note && x.role === "boundary" && !x.note.startsWith("Normally") }}><td class="mono">{x.tag}</td><td>{x.what}</td><td>{ROLE[x.role]}</td><td><b>{x.action}</b>{x.positive ? " (positive)" : ""}</td><td>{x.normal || "—"}</td><td class="muted">{x.note}</td></tr>}</For>
          </tbody></table>
          <h4>Order of work</h4>
          <ol class="isosteps"><For each={p().steps}>{s => <li>{s}</li>}</For></ol>
          <Show when={p().problems.length}><h4>To decide before work</h4><ul class="isoprob"><For each={p().problems}>{s => <li class="bad">{s}</li>}</For></ul></Show>
          <Show when={p().notes.length}><ul class="muted small"><For each={p().notes}>{s => <li>{s}</li>}</For></ul></Show>
          <p class="muted small">A planning aid read from the drawing: the isolation, its method and its sign-off are the site's procedure.</p>
        </>}</Show>
      </Dialog>
    );
  });
};
