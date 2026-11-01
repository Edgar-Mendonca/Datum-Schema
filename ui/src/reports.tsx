// Reports you design: a source (a list, or every object of a class), columns in order, filters,
// sort and grouping, saved in the project by name. The engine builds them (reports.rs); a saved
// report is a tab of the bottom panel and prints and exports like any list.
import { For, Show, createResource, createSignal } from "solid-js";
import { List, Report, ReportFilter } from "./api";
import { act, call, openDialog, proj, setPanel, setPanelOpen, toast } from "./store";
import { Dialog } from "./ui";

const TESTS = ["is", "is not", "contains", "does not contain", "is empty", "is not empty", "at least", "at most"];

/** Open the designer: a new report from a source, or a saved one to change. */
export const designReport = (start: Partial<Report> & { source: string }, editing?: string) => {
  openDialog(() => {
    const [r, setR] = createSignal<Report>({ name: "", cols: [], filters: [], group: "", sort: "", desc: false, ...start });
    const set = (x: Partial<Report>) => setR({ ...r(), ...x });
    const [sources] = createResource(() => call<[string, string][]>("report_sources"));
    const [pv] = createResource(r, x => call<[List, List]>("report_preview", { report: { ...x, name: x.name || "Preview" } }).catch(() => null));
    const src = () => pv()?.[0]?.cols ?? [];
    const out = () => pv()?.[1];
    const shown = () => (r().cols.length ? r().cols : src().map(c => c.key));
    const label = (k: string) => src().find(c => c.key === k)?.label ?? k;
    const toggle = (k: string, on: boolean) => set({ cols: on ? [...shown(), k] : shown().filter(x => x !== k) });
    const move = (i: number, d: number) => { const v = [...shown()]; const j = i + d; if (j < 0 || j >= v.length) return; [v[i], v[j]] = [v[j], v[i]]; set({ cols: v }); };
    const setF = (i: number, x: Partial<ReportFilter>) => set({ filters: r().filters.map((f, j) => (j === i ? { ...f, ...x } : f)) });
    const save = async () => {
      const name = r().name.trim();
      if (!name) { toast("Give the report a name."); return false; }
      const others = (proj()?.settings.reports ?? []).filter(x => x.name.toLowerCase() !== (editing ?? "").toLowerCase());
      if (others.some(x => x.name.toLowerCase() === name.toLowerCase())) { toast(`There is already a report called ${name}.`); return false; }
      const ok = await act({ SetSettings: { settings: { ...proj()!.settings, reports: [...others, { ...r(), name }] } } });
      if (ok) { setPanel(`report:${name}`); setPanelOpen(true); }
      return !!ok;
    };
    return (
      <Dialog title={editing ? `Report: ${editing}` : "New report"} wide ok={save} okText="Save the report" help="reports">
        <div class="repgrid">
          <div>
            <label class="iprow"><span>Name</span><input class="repname" value={r().name} placeholder="Valves to buy" onInput={e => set({ name: e.currentTarget.value })} /></label>
            <label class="iprow"><span>From</span><select class="repsrc" value={r().source} onChange={e => set({ source: e.currentTarget.value, cols: [], filters: [], group: "", sort: "" })}><For each={sources() ?? []}>{([k, l]) => <option value={k} selected={k === r().source}>{l}</option>}</For></select></label>
            <h4>Columns</h4>
            <div class="repcols">
              <For each={shown()}>{(k, i) => <div class="repcol on"><input type="checkbox" checked onChange={() => toggle(k, false)} /><span>{label(k)}</span><button class="x" title="Earlier" onClick={() => move(i(), -1)}>↑</button><button class="x" title="Later" onClick={() => move(i(), 1)}>↓</button></div>}</For>
              <For each={src().filter(c => !shown().includes(c.key))}>{c => <div class="repcol"><input type="checkbox" onChange={() => toggle(c.key, true)} /><span class="muted">{c.label}</span></div>}</For>
            </div>
          </div>
          <div>
            <h4>Only the rows where</h4>
            <For each={r().filters}>{(f, i) => <div class="repf">
              <select value={f.col} onChange={e => setF(i(), { col: e.currentTarget.value })}><For each={src()}>{c => <option value={c.key} selected={c.key === f.col}>{c.label}</option>}</For></select>
              <select value={f.test} onChange={e => setF(i(), { test: e.currentTarget.value })}><For each={TESTS}>{t => <option value={t} selected={t === f.test}>{t}</option>}</For></select>
              <Show when={!f.test.startsWith("is empty") && f.test !== "is not empty"}><input value={f.value} placeholder={f.test === "is" ? "FC, FO" : ""} onChange={e => setF(i(), { value: e.currentTarget.value })} /></Show>
              <button class="x" title="Remove" onClick={() => set({ filters: r().filters.filter((_, j) => j !== i()) })}>×</button>
            </div>}</For>
            <p><button class="btn sm" onClick={() => set({ filters: [...r().filters, { col: src()[0]?.key ?? "", test: "is not empty", value: "" }] })}>Add a filter</button></p>
            <label class="iprow"><span>Group by</span><select class="repgroup" value={r().group} onChange={e => set({ group: e.currentTarget.value })}><option value="">—</option><For each={src()}>{c => <option value={c.key} selected={c.key === r().group}>{c.label}</option>}</For></select></label>
            <label class="iprow"><span>Sort by</span><span class="inline"><select value={r().sort} onChange={e => set({ sort: e.currentTarget.value })}><option value="">—</option><For each={src()}>{c => <option value={c.key} selected={c.key === r().sort}>{c.label}</option>}</For></select><label class="inline small"><input type="checkbox" checked={r().desc} onChange={e => set({ desc: e.currentTarget.checked })} /> largest first</label></span></label>
            <h4>Preview</h4>
            <Show when={out()} fallback={<p class="muted small">Choose a source and its columns.</p>}>{l => <>
              <p class="muted small reprows">{l().rows.length - (l().heads?.length ?? 0)} rows{l().heads?.length ? ` in ${l().heads!.length} groups` : ""}</p>
              <div class="repprev"><table class="tbl small"><thead><tr><For each={l().cols}>{c => <th>{c.label}</th>}</For></tr></thead><tbody>
                <For each={l().rows.slice(0, 12)}>{(row, i) => <tr classList={{ ghead: !!l().heads?.includes(i()) }}><For each={row.cells}>{c => <td>{c}</td>}</For></tr>}</For>
              </tbody></table></div>
            </>}</Show>
          </div>
        </div>
      </Dialog>
    );
  });
};

/** Remove a saved report. */
export const deleteReport = (name: string) => {
  act({ SetSettings: { settings: { ...proj()!.settings, reports: (proj()?.settings.reports ?? []).filter(x => x.name !== name) } } });
  setPanel("checks");
};
