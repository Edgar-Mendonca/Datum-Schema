// Options › Pipe specs: the project's pipe classes, their limits and the components each allows
// (engine: specs.rs, checks 21–23 and the take-off). The window edits; the engine keeps them sound.
import { For, Show, createMemo, createSignal } from "solid-js";
import { Implied, PipeSpec, SpecItem } from "./api";
import { act, call, lib, proj, toast } from "./store";
import { fromU, toU, unit } from "./units";
import { num, parseNum } from "./api";

const ENDS = ["flanged", "butt weld", "socket weld", "threaded", "none"];

/** The components a spec can list: valves, fittings, control and relief valves, in-line elements. */
const kinds = createMemo(() => (lib()?.symbols ?? []).filter(s => !s.key.startsWith("x.") && (["valve", "control", "fitting"].includes(s.cat) || (s.cat === "primary" && s.inline))).map(s => [s.key, s.name] as [string, string]));

export const SpecsSec = () => {
  const specs = () => proj()!.settings.specs;
  const [cur, setCur] = createSignal(0);
  const sp = () => specs()[Math.min(cur(), specs().length - 1)];
  const save = (v: PipeSpec[]) => act({ SetSettings: { settings: { ...proj()!.settings, specs: v } } });
  const set = (patch: Partial<PipeSpec>) => save(specs().map(s => (s === sp() ? { ...s, ...patch } : s)));
  const setImp = (j: number, patch: Partial<Implied>) => set({ implied: sp()!.implied.map((x, k) => (k === j ? { ...x, ...patch } : x)) });
  const setItem = (j: number, patch: Partial<SpecItem>) => set({ items: sp()!.items.map((x, k) => (k === j ? { ...x, ...patch } : x)) });
  const add = async (starter: boolean) => {
    const used = new Set(specs().map(s => s.name.toUpperCase()));
    let n = 1; while (used.has(`A${n}A`)) n++;
    const name = `A${n}A`;
    const s: PipeSpec | null | undefined = starter ? await call<PipeSpec>("spec_starter", { name }) : { name, desc: "", rating: "", material: "", corrosion: 0, max_p: 0, max_t: 0, min: "", max: "", sched: "", items: [], implied: [] };
    if (s && (await save([...specs(), s]))) setCur(specs().length - 1);
  };
  const csv = (f: File | undefined) => {
    if (!f) return;
    const r = new FileReader();
    r.onload = async () => {
      const out = await call<[PipeSpec[], string[]]>("specs_csv", { specs: specs(), text: String(r.result) });
      if (out && (await save(out[0]))) toast(`Read ${out[0].reduce((n, s) => n + s.items.length, 0)} components in ${out[0].length} classes.${out[1].length ? " " + out[1].slice(0, 3).join(" ") : ""}`, true);
    };
    r.readAsText(f);
  };
  const load = (f: File | undefined) => {
    if (!f) return;
    const r = new FileReader();
    r.onload = () => {
      try {
        const j = JSON.parse(String(r.result));
        const v = (j?.specs ?? j) as PipeSpec[];
        if (!Array.isArray(v)) return toast("That is not a Datum Schema pipe spec file.");
        const names = new Set(v.map(s => String(s.name).toUpperCase()));
        save([...specs().filter(s => !names.has(s.name.toUpperCase())), ...v]).then(o => o && toast(`Loaded ${v.length} pipe classes.`, true));
      } catch { toast("That is not a Datum Schema pipe spec file."); }
    };
    r.readAsText(f);
  };
  const exportIt = async () => {
    const f = await call<string | null>("save_file", { name: `${proj()!.name || "Project"} pipe specs.dspecs.json`, data: JSON.stringify({ datum_schema_pipe_specs: 1, specs: specs() }, null, 1) });
    if (f) toast(`Saved ${f}.`, true);
  };
  const n = (v: string) => { const x = parseNum(v); return x === null ? 0 : x; };
  return (
    <div class="specsec">
      <Show when={specs().length} fallback={<p class="muted small">No pipe specs yet: lines can name any class and checks 21–23 stay quiet. Start from a typical carbon steel class, read your specs from a CSV table, or load a file.</p>}>
        <div class="spectabs"><For each={specs()}>{(s, i) => <button classList={{ on: sp() === s }} onClick={() => setCur(i())}>{s.name}</button>}</For></div>
        <Show when={sp()}>{s => <>
          <div class="form specform">
            <label>Class</label><div class="inline"><input class="mono" style={{ width: "90px" }} value={s().name} onChange={e => set({ name: e.currentTarget.value })} /><input placeholder="Description" style={{ flex: 1 }} value={s().desc} onChange={e => set({ desc: e.currentTarget.value })} /></div>
            <label>Rating and material</label><div class="inline"><input style={{ width: "90px" }} placeholder="150#" value={s().rating} onChange={e => set({ rating: e.currentTarget.value })} /><input placeholder="ASTM A106 Gr. B" style={{ flex: 1 }} value={s().material} onChange={e => set({ material: e.currentTarget.value })} /></div>
            <label>Sizes</label><div class="inline"><input class="mono" style={{ width: "70px" }} placeholder="1/2" value={s().min} onChange={e => set({ min: e.currentTarget.value })} /> to <input class="mono" style={{ width: "70px" }} placeholder="24" value={s().max} onChange={e => set({ max: e.currentTarget.value })} /> NPS, schedule <input class="mono" style={{ width: "60px" }} value={s().sched} onChange={e => set({ sched: e.currentTarget.value })} /></div>
            <label>Limits</label><div class="inline">
              <input class="mono" style={{ width: "80px" }} value={s().max_p ? num(toU("press", s().max_p), 4) : ""} onChange={e => set({ max_p: e.currentTarget.value.trim() ? fromU("press", n(e.currentTarget.value)) : 0 })} /> {unit("press")} at most,
              <input class="mono" style={{ width: "80px" }} value={s().max_t ? num(toU("temp", s().max_t), 4) : ""} onChange={e => set({ max_t: e.currentTarget.value.trim() ? fromU("temp", n(e.currentTarget.value)) : 0 })} /> {unit("temp")} at most; corrosion
              <input class="mono" style={{ width: "60px" }} value={num(s().corrosion * 1000, 3)} onChange={e => set({ corrosion: n(e.currentTarget.value) / 1000 })} /> mm</div>
          </div>
          <table class="tbl specitems">
            <thead><tr><th>Component</th><th>From</th><th>To</th><th>Description</th><th>Commodity code</th><th>Ends</th><th /></tr></thead>
            <tbody><For each={s().items}>{(it, j) => <tr>
              <td><select value={it.kind} onChange={e => setItem(j(), { kind: e.currentTarget.value })}><For each={kinds()}>{([k, l]) => <option value={k}>{l}</option>}</For></select></td>
              <td><input class="mono num" value={it.min} onChange={e => setItem(j(), { min: e.currentTarget.value })} /></td>
              <td><input class="mono num" value={it.max} onChange={e => setItem(j(), { max: e.currentTarget.value })} /></td>
              <td><input value={it.desc} onChange={e => setItem(j(), { desc: e.currentTarget.value })} /></td>
              <td><input class="mono" value={it.code} onChange={e => setItem(j(), { code: e.currentTarget.value })} /></td>
              <td><select value={it.ends} onChange={e => setItem(j(), { ends: e.currentTarget.value })}><option value="">from the description</option><For each={ENDS}>{x => <option value={x}>{x}</option>}</For></select></td>
              <td><button class="x" title="Remove" onClick={() => set({ items: s().items.filter((_, k) => k !== j()) })}>×</button></td>
            </tr>}</For></tbody>
          </table>
          <h4 class="imph">Implied at each end <span class="muted small">(for the take-off)</span></h4>
          <table class="tbl specitems implied">
            <thead><tr><th>Item</th><th>At ends</th><th>Per end</th><th>From</th><th>To</th><th>Description</th><th>Commodity code</th><th /></tr></thead>
            <tbody><For each={s().implied}>{(r, j) => <tr>
              <td><input value={r.what} onChange={e => setImp(j(), { what: e.currentTarget.value })} /></td>
              <td><select value={r.ends} onChange={e => setImp(j(), { ends: e.currentTarget.value })}><For each={ENDS}>{x => <option value={x}>{x}</option>}</For></select></td>
              <td><input class="mono num" value={r.per} onChange={e => setImp(j(), { per: n(e.currentTarget.value) })} /></td>
              <td><input class="mono num" value={r.min} onChange={e => setImp(j(), { min: e.currentTarget.value })} /></td>
              <td><input class="mono num" value={r.max} onChange={e => setImp(j(), { max: e.currentTarget.value })} /></td>
              <td><input value={r.desc} onChange={e => setImp(j(), { desc: e.currentTarget.value })} /></td>
              <td><input class="mono" value={r.code} onChange={e => setImp(j(), { code: e.currentTarget.value })} /></td>
              <td><button class="x" title="Remove" onClick={() => set({ implied: s().implied.filter((_, k) => k !== j()) })}>×</button></td>
            </tr>}</For></tbody>
          </table>
          <p class="specbtns"><button class="btn sm" onClick={() => set({ items: [...s().items, { kind: kinds()[0]?.[0] ?? "gate", min: "", max: "", desc: "", code: "", ends: "" }] })}>Add a component</button>
            <button class="btn sm" onClick={() => set({ implied: [...s().implied, { what: "Gasket", ends: "flanged", per: 1, min: "", max: "", desc: "", code: "" }] })}>Add an implied item</button>
            <button class="btn sm" onClick={() => { save(specs().filter(x => x !== s())); setCur(0); }}>Delete class {s().name}</button></p>
        </>}</Show>
      </Show>
      <p class="specbtns">
        <button class="btn sm" onClick={() => add(true)}>Add a typical carbon steel class</button>
        <button class="btn sm" onClick={() => add(false)}>Add an empty class</button>
        <label class="btn sm">Read from CSV…<input type="file" accept=".csv,.txt,text/csv" hidden onChange={e => csv(e.currentTarget.files?.[0])} /></label>
        <label class="btn sm">Load a spec file…<input type="file" accept=".json,application/json" hidden onChange={e => load(e.currentTarget.files?.[0])} /></label>
        <Show when={specs().length}><button class="btn sm" onClick={exportIt}>Save a copy…</button></Show>
      </p>
      <p class="muted small">CSV: one row per component with the columns Class, Component (a symbol's name, such as “Gate valve”), From, To, Description, Code and, if you like, Ends. Sizes in NPS. A typical class is a starting point: check it against your company's specification.</p>
    </div>
  );
};
