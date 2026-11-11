// The sheet manager (engine: sheetman.rs): every sheet with its kind, revision, status, contents
// and problems; reorder, print a chosen set, and bring back deleted sheets from the bin.
import { For, Show, createResource, createSignal } from "solid-js";
import { act, call, openDialog, proj, setBackstage, setSheetId, closeDialog, toast } from "./store";
import { Dialog } from "./ui";
import { setPrintSheets } from "./print";

interface Row { id: number; number: string; title: string; size: string; rev: string; date: string; status: string; kind: string; symbols: number; lines: number; errors: number; warnings: number; issued: string }
interface BinRow { number: string; title: string; symbols: number; lines: number }
interface Manager { sheets: Row[]; bin: BinRow[]; statuses: string[] }

export const openSheetManager = () => openDialog(() => {
  const [m, { refetch }] = createResource(() => call<Manager>("sheet_manager"));
  const [chosen, setChosen] = createSignal<Set<number>>(new Set());
  const run = async (op: object | string, msg?: string) => { const o = await act(op); if (o) { msg && toast(msg, true); refetch(); } return o; };
  const setStatus = (id: number, status: string) => { const s = proj()?.sheets.find(x => x.id === id); if (s) run({ SetSheet: { sheet: { ...s, status } } }); };
  const move = (id: number, by: number) => { const i = proj()?.sheets.findIndex(x => x.id === id) ?? -1; if (i >= 0) run({ MoveSheet: { id, to: Math.max(0, i + by) } }); };
  const toggle = (id: number, on: boolean) => { const s = new Set(chosen()); on ? s.add(id) : s.delete(id); setChosen(s); };
  const print = () => {
    const ids = (m()?.sheets ?? []).filter(r => chosen().has(r.id)).map(r => r.id);
    if (!ids.length) return toast("Tick the sheets to print first.");
    setPrintSheets(ids); closeDialog(); setBackstage("print");
  };
  return (
    <Dialog title="Sheet manager" wide help="sheetman" extra={<button class="btn" onClick={print}>Print the ticked sheets…</button>}>
      <table class="tbl small shtbl"><thead><tr><th><input type="checkbox" title="All" checked={!!m()?.sheets.length && chosen().size === m()!.sheets.length} onChange={e => setChosen(new Set(e.currentTarget.checked ? (m()?.sheets ?? []).map(r => r.id) : []))} /></th><th>Sheet</th><th>Title</th><th>Kind</th><th>Size</th><th>Rev</th><th>Issued</th><th>Status</th><th>Symbols</th><th>Lines</th><th>Problems</th><th /></tr></thead><tbody>
        <For each={m()?.sheets ?? []}>{r => <tr>
          <td><input type="checkbox" checked={chosen().has(r.id)} onChange={e => toggle(r.id, e.currentTarget.checked)} /></td>
          <td class="mono"><a href="#" onClick={e => { e.preventDefault(); setSheetId(r.id); closeDialog(); }}>{r.number}</a></td><td>{r.title}</td><td>{r.kind}</td><td>{r.size}</td><td>{r.rev}</td><td>{r.issued || "—"}</td>
          <td><select class="shstatus" value={r.status} onChange={e => setStatus(r.id, e.currentTarget.value)}><For each={m()?.statuses ?? []}>{s => <option value={s} selected={s === r.status}>{s}</option>}</For></select></td>
          <td class="num">{r.symbols}</td><td class="num">{r.lines}</td>
          <td classList={{ bad: r.errors > 0 }}>{r.errors || r.warnings ? `${r.errors} errors, ${r.warnings} warnings` : "none"}</td>
          <td class="nowrap"><button class="btn sm" title="Earlier" onClick={() => move(r.id, -1)}>↑</button><button class="btn sm" title="Later" onClick={() => move(r.id, 1)}>↓</button></td>
        </tr>}</For>
      </tbody></table>
      <h4>Deleted sheets</h4>
      <Show when={m()?.bin.length} fallback={<p class="muted small">None. A deleted sheet is kept here with its symbols, lines, notes and the objects only it held, until the bin is emptied (the last 20 are kept, in the project file).</p>}>
        <table class="tbl small shbin"><tbody><For each={m()?.bin ?? []}>{(b, i) => <tr><td class="mono">{b.number}</td><td>{b.title}</td><td class="muted">{b.symbols} symbols, {b.lines} lines</td><td><button class="btn sm" onClick={() => run({ RestoreSheet: { at: i() } }, `${b.number} is back, with everything that was on it.`)}>Restore</button></td></tr>}</For></tbody></table>
        <p><button class="btn sm" onClick={() => run("EmptyBin", "The bin is empty.")}>Empty the bin</button></p>
      </Show>
    </Dialog>
  );
});
