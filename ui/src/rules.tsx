// Options › Company rules (check 24) and Options › Tags (how new tags are written). The engine
// checks and numbers; the window edits the settings.
import { For, Show, createMemo } from "solid-js";
import { TagFormat, UserRule } from "./api";
import { act, lib, proj } from "./store";
import { fromU, toU, unit } from "./units";
import { num, parseNum } from "./api";

const TESTS = ["required", "equals", "not equals", "one of", "contains", "at least", "at most"];

export const RulesSec = () => {
  const rules = () => proj()!.settings.rules;
  const save = (v: UserRule[]) => act({ SetSettings: { settings: { ...proj()!.settings, rules: v } } });
  const set = (i: number, patch: Partial<UserRule>) => save(rules().map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const classes = createMemo(() => (lib()?.classes ?? []).filter(c => c.attrs.length).map(c => [c.name, c.label] as [string, string]));
  const attrsOf = (cls: string) => lib()?.classes.find(c => c.name === cls)?.attrs ?? [];
  const qOf = (r: UserRule) => attrsOf(r.class).find(a => a.key === r.key)?.q ?? "";
  return (
    <div class="rulesec">
      <Show when={rules().length} fallback={<p class="muted small">No company rules yet.</p>}>
        <table class="tbl rulestbl">
          <thead><tr><th>On</th><th>Every</th><th>Attribute</th><th>Must be</th><th>Value</th><th>Level</th><th>Message (optional)</th><th /></tr></thead>
          <tbody><For each={rules()}>{(r, i) => <tr>
            <td class="c"><input type="checkbox" checked={r.on} onChange={e => set(i(), { on: e.currentTarget.checked })} /></td>
            <td><select value={r.class} onChange={e => set(i(), { class: e.currentTarget.value, key: attrsOf(e.currentTarget.value)[0]?.key ?? "" })}><For each={classes()}>{([k, l]) => <option value={k}>{l}</option>}</For></select></td>
            <td><select value={r.key} onChange={e => set(i(), { key: e.currentTarget.value })}><For each={attrsOf(r.class)}>{a => <option value={a.key}>{a.label}</option>}</For></select></td>
            <td><select value={r.test} onChange={e => set(i(), { test: e.currentTarget.value })}><For each={TESTS}>{t => <option value={t}>{t}</option>}</For></select></td>
            <td><Show when={r.test !== "required"}><Show when={r.test === "at least" || r.test === "at most"} fallback={<input value={r.value} placeholder={r.test === "one of" ? "FC, FO" : ""} onChange={e => set(i(), { value: e.currentTarget.value })} />}>
              <span class="inline"><input class="mono num" value={num(qOf(r) ? toU(qOf(r), r.num) : r.num, 6)} onChange={e => { const x = parseNum(e.currentTarget.value); if (x !== null) set(i(), { num: qOf(r) ? fromU(qOf(r), x) : x }); }} /> {qOf(r) ? unit(qOf(r)) : ""}</span>
            </Show></Show></td>
            <td><select value={r.level} onChange={e => set(i(), { level: e.currentTarget.value })}><For each={["error", "warning", "note"]}>{l => <option value={l}>{l}</option>}</For></select></td>
            <td><input value={r.message} placeholder="Said in the Checks panel" onChange={e => set(i(), { message: e.currentTarget.value })} /></td>
            <td><button class="x" title="Remove" onClick={() => save(rules().filter((_, j) => j !== i()))}>×</button></td>
          </tr>}</For></tbody>
        </table>
      </Show>
      <p><button class="btn sm" onClick={() => save([...rules(), { on: true, class: "Equipment", key: "material", test: "required", value: "", num: 0, level: "warning", message: "" }])}>Add a rule</button></p>
    </div>
  );
};

export const TagsSec = () => {
  const f = () => proj()!.settings.tag_format;
  const set = (patch: Partial<TagFormat>) => act({ SetSettings: { settings: { ...proj()!.settings, tag_format: { ...f(), ...patch } } } });
  const sample = (n: number) => `P${f().sep}${f().unit}${String(n).padStart(f().digits, "0")}`;
  return (
    <div class="form tagsec">
      <label>Separator</label><div><select value={f().sep} onChange={e => set({ sep: e.currentTarget.value })}><For each={[["-", "P-101 (hyphen)"], ["", "P101 (none)"], [" ", "P 101 (space)"], ["_", "P_101"], [".", "P.101"], ["/", "P/101"]]}>{([k, l]) => <option value={k}>{l}</option>}</For></select></div>
      <label>Unit or area code</label><div><input class="mono" style={{ width: "80px" }} value={f().unit} placeholder="none" onChange={e => set({ unit: e.currentTarget.value })} /> <span class="muted small">digits put before the number (10 → P-10101)</span></div>
      <label>Numbering</label><div><select value={f().numbering} onChange={e => set({ numbering: e.currentTarget.value })}><option value="sheet">From 101 on each sheet (201 on sheet 2…)</option><option value="project">From 1 through the project</option></select></div>
      <label>Digits</label><div><input class="mono" type="number" min="1" max="6" style={{ width: "60px" }} value={f().digits} onChange={e => set({ digits: +e.currentTarget.value || 3 })} /></div>
      <label>New tags</label><div class="mono">{f().numbering === "project" ? `${sample(1)}, ${sample(2)}, LT${f().sep}${f().unit}${String(1).padStart(f().digits, "0")}…` : `${sample(101)}, ${sample(102)}, ${sample(201)} on sheet 2…`}</div>
    </div>
  );
};
