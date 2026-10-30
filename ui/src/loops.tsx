// Loop diagrams: every instrument loop, drawn by the engine (loops.rs) from the P&ID on an A3
// sheet. The wiring of each field device (junction box, terminals, cable, control system, I/O
// channel) is typed here, or in the Inspector; the diagram follows. Print one loop or all.
import { For, Show, createResource, createSignal } from "solid-js";
import { LoopInfo, LoopView, LOOP_WIRING } from "./api";
import { call, act, proj, openDialog, closeDialog, setBackstage } from "./store";
import { Dialog } from "./ui";
import { C } from "./cmds";
import { primsSvg, setPrintLoop } from "./print";

export const openLoops = (start?: string) => openDialog(() => {
  const [list] = createResource(() => proj(), () => call<LoopInfo[]>("loops"));
  const [key, setKey] = createSignal(start ?? "");
  const cur = () => key() || list()?.[0]?.key || "";
  const [view] = createResource(() => [cur(), proj()] as const, ([k]) => (k ? call<LoopView>("loop_diagram", { key: k }) : undefined));
  const set = (obj: number, k: string, v: string) => act({ SetData: { ids: [obj], key: k, val: v.trim() ? v.trim() : null } });
  return (
    <Dialog title={view() ? `${view()!.title}` : "Loop diagrams"} wide help="loops"
      extra={<>
        <button class="btn" disabled={!view()} onClick={() => { setPrintLoop(view()!.key); closeDialog(); setBackstage("print"); }}>Print…</button>
      </>}>
      <Show when={(list() ?? []).length} fallback={<p class="muted">{list() ? "There are no loops yet: a loop is two or more instruments sharing a first letter and a loop number (LT-101, LIC-101, LV-101)." : "…"}</p>}>
        <div class="loops">
          <div class="looplist"><For each={list()}>{l => <button classList={{ on: l.key === cur() }} onClick={() => setKey(l.key)}><b>{l.key}</b><small>{l.tags.join(" · ")}</small></button>}</For></div>
          <div class="loopmain">
            <Show when={view()} fallback={<p class="muted">Drawing the loop…</p>}>{v => <>
              <div class="loopsvg" innerHTML={primsSvg(v().prims, v().w, v().h, { width: v().w * 0.62, height: v().h * 0.62 })} />
              <h4>Wiring</h4>
              <table class="tbl small loopw"><thead><tr><th>Tag</th><For each={LOOP_WIRING}>{([, l]) => <th>{l}</th>}</For><th /></tr></thead><tbody>
                <For each={v().members}>{m => <tr>
                  <td class="mono" title={m.what}>{m.tag}</td>
                  <For each={LOOP_WIRING}>{([k], i) => <td><input value={m.wiring[i()]} placeholder={m.place === "system" && k !== "system" && k !== "io" ? "" : "—"} onChange={e => set(m.obj, k, e.currentTarget.value)} /></td>}</For>
                  <td><button class="btn sm" onClick={() => { closeDialog(); C.goTo([m.item], m.sheet); }}>Go to</button></td>
                </tr>}</For>
              </tbody></table>
              <Show when={v().missing.length}><p class="small warnline">To complete: {v().missing.join(" · ")}</p></Show>
            </>}</Show>
          </div>
        </div>
      </Show>
    </Dialog>
  );
});
