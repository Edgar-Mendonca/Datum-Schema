// The handover package (engine: handover.rs): what it will hold and what is worth fixing first,
// then saved as one zip file.
import { For, Show, createResource } from "solid-js";
import { call, openDialog, toast } from "./store";
import { Dialog } from "./ui";

interface Summary { tags: number; drawn: number; not_drawn: number; properties: number; documents: number; files: string[]; gaps: string[] }

export const openHandover = () => openDialog(() => {
  const [s] = createResource(() => call<Summary>("handover_summary"));
  const save = async () => {
    const f = await call<string | null>("export", { kind: "handover", what: new Date().toISOString().slice(0, 10), sheet: 0 });
    if (f) toast(`Saved ${f}: the handover package, with a README that says what each file is.`, true);
    return !!f;
  };
  return (
    <Dialog title="Handover package" wide help="handover" ok={save} okText="Save the package…">
      <Show when={s()} fallback={<p class="muted">Reading the project…</p>}>{x => <>
        <p class="hosum"><b>{x().tags}</b> tags ({x().drawn} drawn, {x().not_drawn} registered or not drawn) · <b>{x().properties}</b> property values · <b>{x().documents}</b> {x().documents === 1 ? "document" : "documents"}</p>
        <p class="muted small">Tables in the shape of the CFIHOS handover specification (CSV), each P&ID in DEXPI, the plant as one web page, the lists and datasheets in Excel, the open checks and a README, in one zip file. Values are in the project's units, with the unit in its own column.</p>
        <Show when={x().gaps.length} fallback={<p class="ok">Nothing left to fix before handing over.</p>}>
          <h4>Worth fixing before handing over</h4>
          <ul class="isoprob hogaps"><For each={x().gaps}>{g => <li class="bad">{g}</li>}</For></ul>
          <p class="muted small">The package can be saved as it is; the README lists these too.</p>
        </Show>
        <h4>In the package</h4>
        <ul class="hofiles small mono"><For each={x().files}>{f => <li>{f}</li>}</For></ul>
      </>}</Show>
    </Dialog>
  );
});
