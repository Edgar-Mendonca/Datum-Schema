// Old P&IDs turned into live sheets (engine: convert.rs): read a DXF or PDF, review which symbol
// each block or recognised shape becomes, then convert into a new sheet with the old drawing behind it.
import { For, createMemo, createSignal } from "solid-js";
import { call, lib, openDialog, setOut, setSheetIdRaw, toast, setPanel, setPanelOpen } from "./store";
import { Out } from "./api";
import { Dialog } from "./ui";

interface BlockUse { block: string; count: number; sym: string; how: string; sample: string }
interface Scan { blocks: BlockUse[]; lines: number; texts: number; warnings: string[] }

export const convertDrawing = async () => {
  const r = await call<[string, Scan] | null>("convert_scan", { path: null });
  if (!r) return;
  const [name, scan] = r;
  openDialog(() => {
    const [map, setMap] = createSignal<Record<string, string>>(Object.fromEntries(scan.blocks.map(b => [b.block, b.sym])));
    const syms = createMemo(() => (lib()?.symbols ?? []).filter(s => !s.key.startsWith("x.") && s.key !== "unknown").sort((a, b) => a.cat_name.localeCompare(b.cat_name) || a.name.localeCompare(b.name)));
    const mapped = () => scan.blocks.filter(b => map()[b.block]).reduce((n, b) => n + b.count, 0);
    const ok = async () => {
      const o = await call<[Out, { sheet: number; symbols: number; tagged: number; lines: number; open_ends: number; split: number; line_numbers: number; notes: string[] }] | null>("convert_apply", { map: map() });
      if (!o) return false;
      const [out, rep] = o;
      setOut(out); setSheetIdRaw(rep.sheet);
      toast(`Converted: ${rep.symbols} symbols (${rep.tagged} tagged), ${rep.lines} lines (${rep.open_ends} ends left open), ${rep.line_numbers} line numbers, ${rep.split} valves set into their lines. The old drawing is behind the sheet to compare; the Checks panel lists what to finish.${rep.notes.length ? " " + rep.notes.join(" ") : ""}`, true);
      setPanel("checks"); setPanelOpen(true);
      return true;
    };
    return (
      <Dialog title={`Convert ${name}`} wide ok={ok} okText="Convert to a new sheet" help="convert">
        <p><b>{scan.blocks.length}</b> kinds of block ({scan.blocks.reduce((n, b) => n + b.count, 0)} placed, <b>{mapped()}</b> to become symbols), <b>{scan.lines}</b> lines, <b>{scan.texts}</b> texts.</p>
        <For each={scan.warnings}>{w => <p class="small warn">{w}</p>}</For>
        <div class="hzscroll"><table class="tbl small cvtbl"><thead><tr><th>Block</th><th>Placed</th><th>A tag on one</th><th>Becomes</th><th>Chosen by</th></tr></thead><tbody>
          <For each={scan.blocks}>{b => <tr classList={{ warn: !map()[b.block] }}>
            <td class="mono">{b.block.startsWith("~") ? <i>a drawn shape</i> : b.block}</td><td class="num">{b.count}</td><td class="mono">{b.sample || "—"}</td>
            <td><select value={map()[b.block] ?? ""} onChange={e => setMap({ ...map(), [b.block]: e.currentTarget.value })}><option value="">Leave out</option><For each={syms()}>{s => <option value={s.key} selected={s.key === map()[b.block]}>{s.cat_name} · {s.name}</option>}</For></select></td>
            <td class="muted">{map()[b.block] !== b.sym ? "you" : b.how === "library" ? "a symbol library" : b.how === "name" ? "the block's name" : b.how === "shape" ? "its shape" : "—"}</td>
          </tr>}</For>
        </tbody></table></div>
        <p class="muted small">The new sheet gets the blocks (and the instrument bubbles, bow-tie valves and pumps drawn as loose lines, recognised by their shape) as symbols where they were, with their tags (from the blocks' attributes or the nearest tag-like text), the drawing's lines joined to the symbols' nozzles, valves drawn over a line set into it, and line numbers read from the text beside the lines. The old drawing stays faintly behind the sheet to compare; hide it in the Inspector when done. Undo takes the whole conversion back.</p>
      </Dialog>
    );
  });
};
