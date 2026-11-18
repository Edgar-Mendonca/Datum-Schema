// Pressure test packs (engine: testpack.rs): line numbers grouped by pipe class and test pressure,
// each with its limits, what to open, remove or isolate inside, its vents and drains, and what
// stops the test. Each pack prints as its own sheet.
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { act, call, openDialog, closeDialog, proj, setSel, setSheetId, setBackstage, toast, setPanel, setPanelOpen } from "./store";
import { Dialog } from "./ui";
import { fmt } from "./units";
import { setPrintList } from "./print";

interface PLine { obj: number; number: string; class: string; test_p: number; source: string; length: number; volume: number }
interface Step { item: number; tag: string; what: string; action: string }
interface Detail { id: string; medium: string; lines: PLine[]; pressure: number; volume: number; limits: Step[]; inside: Step[]; vents: string[]; drains: string[]; problems: string[]; pieces: number[] }
interface Overview { packs: Detail[]; left_out: [string, string][] }
interface Pack { id: string; lines: number[]; medium: string; status: string; date: string; witness: string; note: string }
interface Packs { show: boolean; packs: Pack[] }

const packs = (): Packs => (proj()?.settings as unknown as { testpacks: Packs }).testpacks;
const save = (f: (s: Packs) => void) => {
  const p = proj(); if (!p) return Promise.resolve(null);
  const s: Packs = JSON.parse(JSON.stringify(packs()));
  f(s);
  return act({ SetSettings: { settings: { ...p.settings, testpacks: s } } });
};

export const openTestPacks = () => openDialog(() => {
  const [ov, { refetch }] = createResource(() => call<Overview>("test_packs"));
  const [cur, setCur] = createSignal(packs()?.packs[0]?.id ?? "");
  const pack = createMemo(() => packs()?.packs.find(k => k.id === cur()));
  const det = createMemo(() => ov()?.packs.find(d => d.id === cur()));
  const edit = (f: (k: Pack) => void) => save(s => { const k = s.packs.find(x => x.id === cur()); if (k) f(k); }).then(o => { if (o) refetch(); return o; });
  const make = async () => {
    const o = await act("MakeTestPacks");
    if (o) { await refetch(); const n = packs().packs.length; setCur(packs().packs[0]?.id ?? ""); toast(`${n} test packs, by pipe class and test pressure. Check each pack's limits and what is to be removed.`, true); }
  };
  const show = () => {
    const d = det(); const p = proj(); if (!d || !p) return;
    const first = p.lines.find(l => l.id === d.pieces[0]);
    if (first) setSheetId(first.sheet);
    setSel([...d.pieces]); closeDialog();
  };
  const print = () => { setPrintList(`testpack:${cur()}`); closeDialog(); setBackstage("print"); };
  return (
    <Dialog title="Pressure test packs" wide help="testpacks" extra={<><button class="btn" onClick={() => { closeDialog(); setPanel("testpacks"); setPanelOpen(true); }}>Test pack list</button></>}>
      <div class="hzhead form">
        <label /><div class="inline"><button class="btn sm pri" onClick={make}>Make test packs</button><label class="chk"><input type="checkbox" checked={!!packs()?.show} onChange={e => save(s => { s.show = e.currentTarget.checked; })} />Mark the packs on the sheets (and print them)</label></div>
      </div>
      <div class="hzbody">
        <div class="hznodes">
          <For each={ov()?.packs ?? []}>{d => <button classList={{ on: d.id === cur() }} onClick={() => setCur(d.id)}><b>{d.id}</b> {fmt("press", d.pressure)}<small>{d.lines.length} lines · {packs()?.packs.find(k => k.id === d.id)?.status}{d.problems.length ? ` · ${d.problems.length} to resolve` : ""}</small></button>}</For>
          <Show when={ov()?.left_out.length}><p class="muted small">Not in a pack: <For each={ov()!.left_out}>{([n, why]) => <span class="nowrap">{n} ({why}); </span>}</For></p></Show>
        </div>
        <Show when={pack() && det()} fallback={<p class="muted">No test pack yet. Give the line numbers a Test P (or a design pressure, tested at 1.5 times) in the line list, then Make test packs: line numbers of the same pipe class and test pressure that meet are one pack.</p>}>
          <div class="hzsheet tpsheet">
            <div class="form">
              <label>Test pack</label><div class="inline"><b class="mono">{pack()!.id}</b>
                <select value={pack()!.medium} onChange={e => edit(k => { k.medium = e.currentTarget.value; })}><option>Hydrostatic</option><option>Pneumatic</option></select>
                <select class="tpstatus" value={pack()!.status} onChange={e => edit(k => { k.status = e.currentTarget.value; })}><option>Planned</option><option>Ready</option><option>Tested</option><option>Accepted</option></select>
                <button class="btn sm" onClick={show}>Show on the sheet</button><button class="btn sm" onClick={print}>Print this pack</button><button class="btn sm" onClick={() => save(s => { s.packs = s.packs.filter(k => k.id !== cur()); }).then(() => { refetch(); setCur(packs().packs[0]?.id ?? ""); })}>Delete pack</button></div>
              <label>Test</label><span><b>{fmt("press", det()!.pressure)} gauge</b>, {det()!.medium.toLowerCase()}; fill volume {fmt("volume", det()!.volume, 3)}</span>
              <label>Date</label><div class="inline"><input value={pack()!.date} onChange={e => edit(k => { k.date = e.currentTarget.value; })} /><input value={pack()!.witness} placeholder="Witnessed by" onChange={e => edit(k => { k.witness = e.currentTarget.value; })} /></div>
              <label>Note</label><input value={pack()!.note} onChange={e => edit(k => { k.note = e.currentTarget.value; })} />
            </div>
            <div class="hzscroll">
              <h4>Line numbers</h4>
              <table class="tbl small"><thead><tr><th>Line number</th><th>Class</th><th>Test pressure</th><th>From</th><th>Length</th><th>Volume</th></tr></thead><tbody>
                <For each={det()!.lines}>{l => <tr><td class="mono">{l.number}</td><td>{l.class}</td><td>{fmt("press", l.test_p)}</td><td class="muted">{l.source}</td><td>{l.length ? fmt("length", l.length) : "—"}</td><td>{l.volume ? fmt("volume", l.volume, 3) : "—"}</td></tr>}</For>
              </tbody></table>
              <h4>Test limits: blind or close</h4>
              <table class="tbl small tplimits"><tbody><For each={det()!.limits}>{s => <tr><td class="mono">{s.tag}</td><td>{s.what}</td><td>{s.action}</td></tr>}</For></tbody></table>
              <h4>Inside: open, remove or isolate</h4>
              <table class="tbl small tpinside"><tbody><For each={det()!.inside}>{s => <tr><td class="mono">{s.tag}</td><td>{s.what}</td><td>{s.action}</td></tr>}</For></tbody></table>
              <p><b>Vents:</b> {det()!.vents.join(", ") || "none"} · <b>Drains:</b> {det()!.drains.join(", ") || "none"}</p>
              <Show when={det()!.problems.length}><h4>To resolve before the test</h4><ul class="isoprob tpprob"><For each={det()!.problems}>{x => <li class="bad">{x}</li>}</For></ul></Show>
            </div>
          </div>
        </Show>
      </div>
      <p class="muted small">A planning aid read from the drawing: the test pressure, medium and procedure are set by the engineer under the piping code.</p>
    </Dialog>
  );
});
