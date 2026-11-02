// The tag register (engine: register.rs): tags added before they are drawn, typed or pasted from a
// spreadsheet or read from Excel or CSV, previewed first; drawn later as the same objects.
import { For, Show, createSignal } from "solid-js";
import { act, call, lib, openDialog, proj, setPanel, setPanelOpen, setTool, toast } from "./store";
import { Dialog } from "./ui";
import { symbolFor } from "./plant";

interface RegRow { tag: string; class: string; service: string; status: string; note: string }

/** Add tags: paste (tab, comma or semicolon separated: tag, class, service) or read a file. */
export const addTags = () => openDialog(() => {
  const [text, setText] = createSignal("");
  const [rows, setRows] = createSignal<RegRow[] | null>(null);
  const preview = async () => setRows((await call<RegRow[]>("register_read", { text: text() })) ?? null);
  const file = async () => { const r = await call<RegRow[] | null>("register_file"); if (r) setRows(r); };
  const label = (c: string) => lib()?.classes.find(x => x.name === c)?.label ?? c;
  const ok = async () => {
    const good = (rows() ?? []).filter(r => r.status !== "bad");
    if (!good.length) { toast("Nothing to add: preview the tags first."); return false; }
    const o = await act({ Register: { rows: good } });
    if (o) { toast(`${good.length} tags in the register. Draw them from the Tag register or the Plant panel.`, true); setPanel("register"); setPanelOpen(true); }
    return !!o;
  };
  return (
    <Dialog title="Add tags to the register" wide ok={ok} okText={`Add ${(rows() ?? []).filter(r => r.status !== "bad").length} tags`} help="register">
      <p class="muted small">One tag per line, optionally with its class (or symbol name) and service, separated by tabs, commas or semicolons; a header row with a Tag column is understood. Paste straight from a spreadsheet, or read an Excel or CSV file. The class is guessed from the tag's letters when left out (P-101 a pump, FT-101 a transmitter, FV-101 a control valve).</p>
      <textarea class="regtext" rows={8} placeholder={"P-201\t\tTransfer pump\nV-201\tVertical vessel\tSurge drum\nFT-201\t\tFeed flow"} value={text()} onInput={e => setText(e.currentTarget.value)} />
      <p class="ibtns"><button class="btn sm" onClick={preview}>Preview</button><button class="btn sm" onClick={file}>Read a file…</button></p>
      <Show when={rows()}>{r => <table class="tbl small regtbl"><thead><tr><th>Tag</th><th>Class</th><th>Service</th><th>What happens</th></tr></thead><tbody>
        <For each={r()}>{x => <tr class={x.status}><td class="mono">{x.tag}</td><td>{label(x.class)}</td><td>{x.service}</td><td class="muted">{x.status === "new" ? "added, not drawn" : x.status === "exists" ? "already in the project: marked as registered" : `left out: ${x.note}`}</td></tr>}</For>
      </tbody></table>}</Show>
    </Dialog>
  );
});

/** Starts placing a registered tag that is not drawn yet. */
export const placeRegistered = (obj: number) => {
  const o = proj()?.objects.find(x => x.id === obj);
  const s = o && symbolFor(o);
  if (!o || !s) return toast("Choose a symbol for it in the Plant panel (Draw it on this sheet).");
  setTool({ k: "place", sym: s, obj: o.id });
  toast(`Click on the sheet to draw ${o.tag}.`);
};
