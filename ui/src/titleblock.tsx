// Options › Title block: the standard block or the company's template (engine: frame.rs). The
// window edits the cells and shows the engine's drawing of the block; the engine keeps it sound.
import { For, Show, createResource } from "solid-js";
import { Prim, TbCell, TitleBlock } from "./api";
import { act, call, opts, proj, setOpts, sheet, toast } from "./store";
import { PrimEl } from "./canvas";
import { Check, Pick } from "./ui";

/** What a cell can show (frame::FIELDS), then fixed text and the template's own fields. */
const FIELDS: [string, string][] = [
  ["company", "Company"], ["project", "Project"], ["client", "Client"], ["unit", "Unit or area"], ["title", "Sheet title"], ["number", "Drawing number"], ["rev", "Revision"],
  ["scale", "Scale"], ["sheet", "Sheet n of m"], ["date", "Date"], ["by", "Drawn by"], ["checked", "Checked by"], ["approved", "Approved by"], ["logo", "Company logo"],
  ["text:", "Fixed text…"], ["f:", "A field of our own…"],
];
const kindOf = (f: string) => (f.startsWith("text:") ? "text:" : f.startsWith("f:") ? "f:" : f);

type Preview = { w: number; h: number; prims: Prim[]; logo: [number, number, number, number] | null };

/** The block drawn by the engine, with the logo where it goes. */
export const BlockPreview = (p: { block: TitleBlock }) => {
  const [pv] = createResource(() => ({ block: p.block, sheet: sheet()?.id ?? 0 }), a => call<Preview>("title_preview", a));
  const logo = () => proj()?.settings.ds_layout.logo ?? "";
  return (
    <Show when={pv()}>{v => (
      <svg class="tbprev" viewBox={`-2 -2 ${v().w + 4} ${v().h + 4}`} style={{ "aspect-ratio": `${v().w + 4} / ${v().h + 4}` }}>
        <g class="frame"><For each={v().prims}>{x => <PrimEl p={x} />}</For></g>
        <Show when={v().logo}>{l => <Show when={logo()} fallback={<text x={l()[0] + l()[2] / 2} y={l()[1] + l()[3] / 2} font-size="2.2" text-anchor="middle" class="muted">logo</text>}>
          <image href={logo()} x={l()[0]} y={l()[1]} width={l()[2]} height={l()[3]} preserveAspectRatio="xMidYMid meet" />
        </Show>}</Show>
      </svg>
    )}</Show>
  );
};

export const TitleBlockSec = () => {
  const tb = () => proj()!.settings.title_block;
  const save = (b: TitleBlock) => act({ SetSettings: { settings: { ...proj()!.settings, title_block: b } } });
  const setCell = (i: number, c: Partial<TbCell>) => save({ ...tb(), cells: tb().cells.map((x, j) => (j === i ? { ...x, ...c } : x)) });
  const turnOn = async (on: boolean) => {
    if (on && !tb().cells.length) {
      const s = await call<TitleBlock>("title_standard");
      if (s) return save({ ...s, on: true, name: proj()!.title.company ? `${proj()!.title.company} title block` : "Company title block" });
    }
    save({ ...tb(), on });
  };
  const fromStandard = async () => { const s = await call<TitleBlock>("title_standard"); if (s) save({ ...s, on: true, name: tb().name || "Company title block" }); };
  const exportIt = async () => {
    const f = await call<string | null>("save_file", { name: `${tb().name || "Title block"}.dstitle.json`, data: JSON.stringify({ datum_schema_title_block: 1, block: tb() }, null, 1) });
    if (f) toast(`Saved ${f}: Load it in another project, or send it to a colleague.`, true);
  };
  const load = (f: File | undefined) => {
    if (!f) return;
    if (f.size > 200 * 1024) return toast("That file is too large to be a title block.");
    const r = new FileReader();
    r.onload = () => {
      try {
        const j = JSON.parse(String(r.result));
        const b = (j && j.block) as TitleBlock | undefined;
        if (!b || !Array.isArray(b.cells)) return toast("That is not a Datum Schema title block.");
        save({ ...b, on: true }).then(o => o && toast(`Loaded the title block “${b.name || f.name}”.`, true));
      } catch { toast("That is not a Datum Schema title block."); }
    };
    r.readAsText(f);
  };
  const add = () => save({ ...tb(), cells: [...tb().cells, { x: 0, y: 0, w: 40, h: 10, label: "LABEL", field: "f:Field", size: 2.5, bold: false }] });
  const n = (v: string, d: number) => { const x = parseFloat(v); return Number.isFinite(x) ? x : d; };
  return (
    <div class="tbsec">
      <Check label="Use a company title block on every sheet" value={tb().on} set={turnOn} />
      <Show when={tb().on} fallback={<p class="muted small">The standard block: company, project, title, drawing number, revision, scale, sheet, unit, drawn, checked, approved and date.</p>}>
        <div class="form tbhead">
          <label>Name</label><div><input value={tb().name} onChange={e => save({ ...tb(), name: e.currentTarget.value })} /></div>
          <label>Size</label><div class="inline"><input class="mono" style={{ width: "64px" }} value={tb().w} onChange={e => save({ ...tb(), w: n(e.currentTarget.value, tb().w) })} /> × <input class="mono" style={{ width: "64px" }} value={tb().h} onChange={e => save({ ...tb(), h: n(e.currentTarget.value, tb().h) })} /> mm</div>
        </div>
        <BlockPreview block={tb()} />
        <table class="tbl tbcells">
          <thead><tr><th>Caption</th><th>Shows</th><th /><th class="r">Left</th><th class="r">Top</th><th class="r">Width</th><th class="r">Height</th><th class="r">Text</th><th>Bold</th><th /></tr></thead>
          <tbody><For each={tb().cells}>{(c, i) => <tr>
            <td><input value={c.label} onChange={e => setCell(i(), { label: e.currentTarget.value })} /></td>
            <td><Pick value={kindOf(c.field)} set={v => setCell(i(), { field: v === "text:" ? "text:Text" : v === "f:" ? "f:Field" : v })} options={FIELDS} /></td>
            <td><Show when={kindOf(c.field).endsWith(":")}><input value={c.field.slice(kindOf(c.field).length)} placeholder={kindOf(c.field) === "f:" ? "Field name" : "Text"} onChange={e => setCell(i(), { field: kindOf(c.field) + e.currentTarget.value })} /></Show></td>
            <For each={["x", "y", "w", "h", "size"] as const}>{k => <td class="r"><input class="mono num" value={c[k]} onChange={e => setCell(i(), { [k]: n(e.currentTarget.value, c[k]) })} /></td>}</For>
            <td class="c"><input type="checkbox" checked={c.bold} onChange={e => setCell(i(), { bold: e.currentTarget.checked })} /></td>
            <td><button class="x" title="Remove this cell" onClick={() => save({ ...tb(), cells: tb().cells.filter((_, j) => j !== i()) })}>×</button></td>
          </tr>}</For></tbody>
        </table>
        <p class="muted small">Positions and sizes in millimetres from the block's top left corner. Fields of our own are filled in for each project in the Inspector (nothing selected › Title block). The logo is the one chosen under Datasheets.</p>
      </Show>
      <p class="tbbtns">
        <Show when={tb().on}><button class="btn sm" onClick={add}>Add a cell</button><button class="btn sm" onClick={fromStandard}>Start again from the standard block</button><button class="btn sm" onClick={exportIt}>Save a copy…</button></Show>
        <label class="btn sm">Load a title block…<input type="file" accept=".json,application/json" hidden onChange={e => load(e.currentTarget.files?.[0])} /></label>
        <Show when={tb().on}><button class="btn sm" onClick={() => { const o = { ...opts(), defaults: { ...(opts().defaults ?? proj()!.settings), title_block: tb(), ds_layout: (opts().defaults ?? proj()!.settings).ds_layout } }; setOpts(o); call("set_options", { opts: o }).then(() => toast("New projects will use this title block.", true)); }}>Use for new projects</button></Show>
      </p>
    </div>
  );
};
