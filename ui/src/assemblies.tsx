// Assemblies (a group saved and placed again as one) and data labels (engine: doc.rs
// SaveAssembly/PlaceAssembly, view.rs labels).
import { For, Show, createMemo, createSignal } from "solid-js";
import { Assembly, LabelRule } from "./api";
import { act, call, lib, openDialog, opts, proj, sel, setOpts, setTool, toast, tool } from "./store";
import { Dialog } from "./ui";

/** Save the selection as an assembly. */
export const saveAssembly = () => {
  const ids = sel();
  if (!ids.length) return toast("Select the symbols and lines to save as an assembly.");
  openDialog(() => {
    const [name, setName] = createSignal("");
    return (
      <Dialog title="Save as an assembly" okText="Save" help="assemblies" ok={async () => { const o = await act({ SaveAssembly: { name: name(), ids } }); if (!o) return false; toast(`Saved the assembly “${name().trim()}”: place it from the palette's Assemblies tab.`, true); }}>
        <div class="form"><label>Name</label><div><input placeholder="Control valve station" value={name()} onInput={e => setName(e.currentTarget.value)} /></div></div>
        <p class="muted small">The selected symbols, lines and notes, with their data. Placed again, they get the next free tags; lines to things left out become open ends.</p>
      </Dialog>
    );
  });
};

/** The palette's Assemblies tab. */
export const AssembliesPanel = () => {
  const list = () => proj()?.settings.assemblies ?? [];
  return (
    <div class="pbody">
      <For each={list()} fallback={<p class="muted small plhint">No assemblies yet: select a group on the sheet, then Format › Save as Assembly.</p>}>{a => (
        <button class="asmrow" classList={{ on: tool().k === "assembly" && (tool() as { name: string }).name === a.name }} onClick={() => setTool({ k: "assembly", name: a.name })} title="Click, then click the sheet where it goes">
          <b>{a.name}</b><span class="muted small">{a.items.length} symbols · {a.lines.length} lines</span>
        </button>
      )}</For>
      <p class="muted small plhint">Click an assembly, then click the sheet. Manage them in File › Options › Assemblies.</p>
    </div>
  );
};

/** Options › Assemblies: rename, delete, share, use for new projects. */
export const AssembliesSec = () => {
  const list = () => proj()!.settings.assemblies;
  const save = (v: Assembly[]) => act({ SetSettings: { settings: { ...proj()!.settings, assemblies: v } } });
  const exportIt = async () => {
    const f = await call<string | null>("save_file", { name: `${proj()!.name || "Project"} assemblies.dsasm.json`, data: JSON.stringify({ datum_schema_assemblies: 1, assemblies: list() }, null, 1) });
    if (f) toast(`Saved ${f}.`, true);
  };
  const load = (f: File | undefined) => {
    if (!f) return;
    const r = new FileReader();
    r.onload = () => {
      try {
        const v = (JSON.parse(String(r.result))?.assemblies ?? []) as Assembly[];
        if (!Array.isArray(v) || !v.length) return toast("That is not a Datum Schema assembly file.");
        const names = new Set(v.map(a => a.name.toLowerCase()));
        save([...list().filter(a => !names.has(a.name.toLowerCase())), ...v]).then(o => o && toast(`Loaded ${v.length} assemblies.`, true));
      } catch { toast("That is not a Datum Schema assembly file."); }
    };
    r.readAsText(f);
  };
  return (
    <div class="asmsec">
      <Show when={list().length} fallback={<p class="muted small">None yet: select a group on the sheet, then Format › Save as Assembly.</p>}>
        <table class="tbl"><tbody><For each={list()}>{(a, i) => <tr>
          <td><input value={a.name} onChange={e => save(list().map((x, j) => (j === i() ? { ...x, name: e.currentTarget.value } : x)))} /></td>
          <td class="muted small">{a.items.length} symbols, {a.lines.length} lines</td>
          <td><button class="x" title="Delete" onClick={() => save(list().filter((_, j) => j !== i()))}>×</button></td>
        </tr>}</For></tbody></table>
      </Show>
      <p class="specbtns">
        <Show when={list().length}><button class="btn sm" onClick={exportIt}>Save a copy…</button></Show>
        <label class="btn sm">Load assemblies…<input type="file" accept=".json,application/json" hidden onChange={e => load(e.currentTarget.files?.[0])} /></label>
        <Show when={list().length}><button class="btn sm" onClick={() => { const base = opts().defaults ?? proj()!.settings; const o = { ...opts(), defaults: { ...base, assemblies: list() } }; setOpts(o); call("set_options", { opts: o }).then(() => toast("New projects will start with these assemblies.", true)); }}>Use for new projects</button></Show>
      </p>
    </div>
  );
};

/** Options › Labels: the data shown under each kind of symbol. */
export const LabelsSec = () => {
  const rules = () => proj()!.settings.labels;
  const save = (v: LabelRule[]) => act({ SetSettings: { settings: { ...proj()!.settings, labels: v } } });
  const classes = createMemo(() => (lib()?.classes ?? []).filter(c => c.attrs.length).map(c => [c.name, c.label] as [string, string]));
  const attrsOf = (cls: string) => (lib()?.classes.find(c => c.name === cls)?.attrs ?? []).filter(a => !a.group && a.key !== "service");
  return (
    <div class="lblsec">
      <For each={rules()}>{(r, i) => (
        <div class="lblrule">
          <select value={r.class} onChange={e => save(rules().map((x, j) => (j === i() ? { class: e.currentTarget.value, keys: [] } : x)))}><For each={classes()}>{([k, l]) => <option value={k}>{l}</option>}</For></select>
          <div class="lblkeys"><For each={attrsOf(r.class)}>{a => (
            <label class="chk"><input type="checkbox" checked={r.keys.includes(a.key)} onChange={e => save(rules().map((x, j) => (j === i() ? { ...x, keys: e.currentTarget.checked ? [...x.keys, a.key].slice(0, 6) : x.keys.filter(k => k !== a.key) } : x)))} />{a.label}</label>
          )}</For></div>
          <button class="x" title="Remove" onClick={() => save(rules().filter((_, j) => j !== i()))}>×</button>
        </div>
      )}</For>
      <p><button class="btn sm" onClick={() => save([...rules(), { class: "Pump", keys: ["flow", "head"] }])}>Add a label rule</button></p>
    </div>
  );
};
