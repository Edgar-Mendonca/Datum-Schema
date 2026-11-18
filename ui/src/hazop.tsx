// HAZOP support (engine: hazop.rs): the study's nodes marked on the P&ID and their worksheets.
// Nodes are made from the selection; "Suggest from the drawing" fills the usual deviations with
// the causes and safeguards the drawing shows, marked suggested until the team confirms them.
import { For, Show, createMemo, createSignal } from "solid-js";
import { act, openDialog, closeDialog, proj, sel, setSel, setSheetId, toast, setPanel, setPanelOpen } from "./store";
import { Dialog } from "./ui";

interface HRow { param: string; guide: string; deviation: string; causes: string; consequences: string; safeguards: string; severity: string; likelihood: string; rec: string; by: string; status: string; suggested: boolean }
interface HNode { id: number; name: string; intent: string; ids: number[]; rows: HRow[] }
interface Study { title: string; date: string; team: string; show: boolean; nodes: HNode[] }

const study = (): Study => (proj()?.settings as unknown as { hazop: Study }).hazop;
/** Saves a changed study (one undoable edit). */
const save = (f: (s: Study) => void) => {
  const p = proj(); if (!p) return Promise.resolve(null);
  const s: Study = JSON.parse(JSON.stringify(study()));
  f(s);
  return act({ SetSettings: { settings: { ...p.settings, hazop: s } } });
};

/** A node from the selected symbols and lines. */
export const newNode = async () => {
  if (!sel().length) return toast("Select the symbols and lines of the node first.");
  const o = await act({ HazopNode: { ids: sel(), name: "" } });
  if (o) { toast(`Node N${o.created[0]} made from ${sel().length} symbols and lines. Name it and suggest its deviations in the HAZOP study.`, true); openHazop(o.created[0]); }
};

const EMPTY: HRow = { param: "", guide: "", deviation: "", causes: "", consequences: "", safeguards: "", severity: "", likelihood: "", rec: "", by: "", status: "", suggested: false };
const COLS: [keyof HRow, string, number][] = [["deviation", "Deviation", 120], ["causes", "Causes", 220], ["consequences", "Consequences", 200], ["safeguards", "Safeguards", 220], ["severity", "S", 34], ["likelihood", "L", 34], ["rec", "Recommendation", 200], ["by", "By", 70]];

export const openHazop = (start?: number) => openDialog(() => {
  const [cur, setCur] = createSignal<number>(start ?? study()?.nodes[0]?.id ?? 0);
  const node = createMemo(() => study()?.nodes.find(n => n.id === cur()));
  const ni = () => study()?.nodes.findIndex(n => n.id === cur()) ?? -1;
  const setNode = (f: (n: HNode) => void) => save(s => { const n = s.nodes.find(x => x.id === cur()); if (n) f(n); });
  const setRow = (i: number, k: keyof HRow, v: string) => setNode(n => { const r = n.rows[i] as unknown as Record<string, unknown>; if (r[k] !== v) { r[k] = v; n.rows[i].suggested = false; } });
  const suggest = async () => { const o = await act({ HazopSuggest: { node: cur() } }); if (o) toast("The usual deviations added, with the causes and safeguards the drawing shows. Each is marked suggested until the team confirms or changes it.", true); };
  const showOnSheet = () => {
    const n = node(); const p = proj(); if (!n || !p) return;
    const first = n.ids.map(i => p.items.find(x => x.id === i)?.sheet ?? p.lines.find(x => x.id === i)?.sheet).find(Boolean);
    if (first) setSheetId(first);
    setSel(n.ids); closeDialog();
  };
  return (
    <Dialog title="HAZOP study" wide help="hazop" extra={<><button class="btn" onClick={() => { closeDialog(); setPanel("hazop"); setPanelOpen(true); }}>Worksheet list</button><button class="btn" onClick={() => { closeDialog(); setPanel("hazop_recs"); setPanelOpen(true); }}>Recommendations</button></>}>
      <div class="hzhead form">
        <label>Study</label><input value={study()?.title ?? ""} placeholder="HAZOP of …" onChange={e => save(s => { s.title = e.currentTarget.value; })} />
        <label>Date</label><input value={study()?.date ?? ""} onChange={e => save(s => { s.date = e.currentTarget.value; })} />
        <label>Team</label><input value={study()?.team ?? ""} placeholder="Chair, scribe, process, operations, instruments…" onChange={e => save(s => { s.team = e.currentTarget.value; })} />
        <label /><label class="chk"><input type="checkbox" checked={!!study()?.show} onChange={e => save(s => { s.show = e.currentTarget.checked; })} />Show the nodes on the sheets (and print them)</label>
      </div>
      <div class="hzbody">
        <div class="hznodes">
          <For each={study()?.nodes ?? []}>{n => <button classList={{ on: n.id === cur() }} onClick={() => setCur(n.id)}><b>N{n.id}</b> {n.name}<small>{n.rows.length} rows{n.rows.some(r => r.suggested) ? ", some suggested" : ""}</small></button>}</For>
          <button class="btn sm" title="Select the node's symbols and lines on the sheet first" onClick={() => { closeDialog(); newNode(); }}>New node from the selection</button>
        </div>
        <Show when={node()} fallback={<p class="muted">No node yet. Select the symbols and lines of a part of the plant (a pump and its suction and discharge, a vessel and its nozzles' lines), then New node from the selection, or Check › New Node.</p>}>{n => <div class="hzsheet">
          <div class="form">
            <label>Node</label><div class="inline"><input value={n().name} onChange={e => setNode(x => { x.name = e.currentTarget.value; })} /><button class="btn sm" onClick={showOnSheet}>Show on the sheet</button><button class="btn sm" onClick={() => setNode(x => { x.ids = sel(); })} disabled={!sel().length} title="Its symbols and lines become the ones selected now">Use the selection</button><button class="btn sm" onClick={() => save(s => { s.nodes.splice(ni(), 1); }).then(() => setCur(study()?.nodes[0]?.id ?? 0))}>Delete node</button></div>
            <label>Design intent</label><input value={n().intent} placeholder="Transfer 30 m³/h of water from T-101 to V-101 at 4 bar" onChange={e => setNode(x => { x.intent = e.currentTarget.value; })} />
          </div>
          <p class="ibtns"><button class="btn sm pri" onClick={suggest}>Suggest from the drawing</button><button class="btn sm" onClick={() => setNode(x => { x.rows.push({ ...EMPTY }); })}>Add a row</button><span class="muted small">{n().ids.length} symbols and lines. Suggested text is in italics until the team confirms or edits it.</span></p>
          <div class="hzscroll"><table class="tbl small hztbl"><thead><tr><th>Parameter</th><For each={COLS}>{c => <th style={{ "min-width": `${c[2]}px` }}>{c[1]}</th>}</For><th>Status</th><th /></tr></thead><tbody>
            <For each={n().rows}>{(r, i) => <tr classList={{ sugg: r.suggested }}>
              <td><input class="hzp" value={r.param} placeholder="Flow" onChange={e => setRow(i(), "param", e.currentTarget.value)} /><input class="hzp" value={r.guide} placeholder="No" onChange={e => setRow(i(), "guide", e.currentTarget.value)} /></td>
              <For each={COLS}>{c => <td><textarea rows={c[2] > 60 ? 3 : 1} value={String(r[c[0]] ?? "")} onChange={e => setRow(i(), c[0], e.currentTarget.value)} /></td>}</For>
              <td><select value={r.status} disabled={!r.rec.trim()} onChange={e => setRow(i(), "status", e.currentTarget.value)}><option value="" /><option>Open</option><option>Closed</option></select></td>
              <td class="nowrap"><Show when={r.suggested}><button class="btn sm" title="The team agrees with this row" onClick={() => setNode(x => { x.rows[i()].suggested = false; })}>Confirm</button></Show><button class="btn sm" title="Remove the row" onClick={() => setNode(x => { x.rows.splice(i(), 1); })}>×</button></td>
            </tr>}</For>
          </tbody></table></div>
        </div>}</Show>
      </div>
      <p class="muted small">An aid to the study: the suggestions are read from the drawing; the HAZOP, its consequences, risk ranking and recommendations are the team's.</p>
    </Dialog>
  );
});
