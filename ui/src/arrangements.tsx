// Standard arrangements (engine: arrange.rs): what goes round each kind of equipment, placed in
// one step. The ribbon applies them; Options › Arrangements edits them.
import { For, Show, createMemo } from "solid-js";
import { Arrangement, ArrangementPart } from "./api";
import { act, lib, proj, sel, sheet, setSheetId, toast } from "./store";

const KINDS: [string, string][] = [["inline", "In the line"], ["branch", "Tapped on"], ["controller", "Controller beside it"], ["bypass", "Bypass round it"]];

/** Places the arrangement `name` (empty: each one's own class's) round the selected equipment. */
export const applyArrangement = async (name = "") => {
  const ids = sel();
  const before = proj()?.items.length ?? 0;
  const r = await act({ Arrange: { ids, name } });
  if (!r) return;
  const n = (proj()?.items.length ?? 0) - before;
  toast(n > 0 ? `Placed ${n} symbols. Tidy the draft as you like; Undo takes it all back.` : "Nothing new: the selection already has its arrangement, or none fits it (Options › Arrangements).", n > 0);
};

/** The arrangements the ribbon offers for the selection. */
export const arrangementNames = () => (proj()?.settings.arrangements ?? []).map(a => a.name);

export const ArrangementsSec = () => {
  const arr = () => proj()!.settings.arrangements;
  const save = (v: Arrangement[]) => act({ SetSettings: { settings: { ...proj()!.settings, arrangements: v } } });
  const set = (i: number, x: Partial<Arrangement>) => save(arr().map((a, j) => (j === i ? { ...a, ...x } : a)));
  const setPart = (i: number, k: number, x: Partial<ArrangementPart>) => set(i, { parts: arr()[i].parts.map((p, j) => (j === k ? { ...p, ...x } : p)) });
  const classes = createMemo(() => (lib()?.classes ?? []).map(c => [c.name, c.label] as [string, string]));
  const syms = createMemo(() => (lib()?.symbols ?? []).filter(s => ["valve", "control", "fitting", "instrument", "primary"].includes(s.cat)).map(s => [s.key, s.name] as [string, string]));
  return (
    <div class="arrsec">
      <For each={arr()} fallback={<p class="muted small">No arrangements.</p>}>{(a, i) => <details class="arrbox">
        <summary><b>{a.name}</b> <span class="muted">· {classes().find(c => c[0] === a.class)?.[1] ?? a.class} · {a.parts.length} parts</span></summary>
        <div class="arrhead">
          <label>Name <input value={a.name} onChange={e => set(i(), { name: e.currentTarget.value })} /></label>
          <label>For every <select value={a.class} onChange={e => set(i(), { class: e.currentTarget.value })}><For each={classes()}>{([k, l]) => <option value={k} selected={k === a.class}>{l}</option>}</For></select></label>
          <button class="btn sm" onClick={() => save(arr().filter((_, j) => j !== i()))}>Delete</button>
        </div>
        <table class="tbl small arrtbl"><thead><tr><th>Nozzle (alternatives with |)</th><th>How</th><th>Symbol</th><th>Tag letters</th><th /></tr></thead><tbody>
          <For each={a.parts}>{(p, k) => <tr>
            <td><input value={p.nozzle} disabled={p.kind === "bypass"} onChange={e => setPart(i(), k(), { nozzle: e.currentTarget.value })} /></td>
            <td><select value={p.kind} onChange={e => setPart(i(), k(), { kind: e.currentTarget.value })}><For each={KINDS}>{([v, l]) => <option value={v} selected={v === p.kind}>{l}</option>}</For></select></td>
            <td><select value={p.sym} onChange={e => setPart(i(), k(), { sym: e.currentTarget.value })}><For each={syms()}>{([v, l]) => <option value={v} selected={v === p.sym}>{l}</option>}</For></select></td>
            <td><input class="mono" style={{ width: "60px" }} value={p.tag} placeholder="auto" onChange={e => setPart(i(), k(), { tag: e.currentTarget.value })} /></td>
            <td><button class="x" title="Remove" onClick={() => set(i(), { parts: a.parts.filter((_, j) => j !== k()) })}>×</button></td>
          </tr>}</For>
        </tbody></table>
        <p><button class="btn sm" onClick={() => set(i(), { parts: [...a.parts, { nozzle: "discharge", kind: "inline", sym: "gate", tag: "" }] })}>Add a part</button></p>
      </details>}</For>
      <p class="ibtns">
        <button class="btn sm" onClick={() => save([...arr(), { name: `Arrangement ${arr().length + 1}`, class: "Pump", parts: [] }])}>Add an arrangement</button>
        <Show when={!arr().some(a => a.name === "Centrifugal pump")}><button class="btn sm" onClick={() => import("./store").then(s => s.call<Arrangement[]>("arrangement_starter").then(v => v && save([...arr().filter(a => !v.some(x => x.name === a.name)), ...v])))}>Add the starter set</button></Show>
      </p>
    </div>
  );
};

/** Makes a PFD sheet from every P&ID sheet (engine: makepfd.rs) and opens it. */
export const makePfd = async () => {
  const o = await act({ MakePfd: { sheets: [] } });
  if (!o?.created.length) return;
  const sid = o.created[0];
  const n = o.p.lines.filter(l => l.sheet === sid).length;
  setSheetId(sid);
  toast(`Made ${o.p.sheets.find(x => x.id === sid)?.number} from the P&IDs: ${n} streams between the main equipment, each linked to its lines. Undo takes it back.`, true);
};

/** Develops the current PFD sheet into a new P&ID sheet (engine: develop.rs) and opens it. */
export const developPid = async () => {
  const s = sheet();
  if (!s?.pfd) return toast("Open a process flow diagram sheet first (Sheets › PFD Sheet).");
  const o = await act({ Develop: { sheet: s.id } });
  if (!o?.created.length) return;
  const sid = o.created[0];
  const n = o.p.items.filter(i => i.sheet === sid).length;
  setSheetId(sid);
  toast(`Developed ${o.p.sheets.find(x => x.id === sid)?.number}: ${n} symbols, each stream a line number with its data. A draft to tidy; Undo takes it back.`, true);
};

/** Options › Process cases: their names (the values of a removed case go with it). */
export const CasesSec = () => {
  const cs = () => proj()!.settings.cases;
  const save = (v: string[]) => act({ SetCases: { cases: v } });
  return (
    <div class="casesec">
      <For each={cs()}>{(c, i) => <div class="iprow"><input value={c} onChange={e => save(cs().map((x, j) => (j === i() ? e.currentTarget.value : x)))} />{c === proj()!.settings.case ? <span class="muted small">active</span> : <button class="x" title="Remove this case and its values" onClick={() => save(cs().filter((_, j) => j !== i()))}>×</button>}</div>}</For>
      <p><button class="btn sm" onClick={() => save([...cs(), ["Design", "Start-up", "Turndown", "Winter", "Summer"].find(x => !cs().includes(x)) ?? `Case ${cs().length + 1}`])}>Add a case</button></p>
    </div>
  );
};

/** Places a loop (F, P, T or L) on the selected line or vessel (engine: loopplace.rs). */
export const placeLoop = async (v: string, control: boolean) => {
  const ids = sel();
  if (ids.length !== 1) return toast(v === "L" ? "Select one vessel first." : "Select one process line first.");
  const before = proj()?.items.length ?? 0;
  const o = await act({ PlaceLoop: { target: ids[0], var: v, control } });
  if (o) toast(`Placed ${(o.p.items.length - before)} symbols with one loop number. Undo takes them back.`, true);
};
