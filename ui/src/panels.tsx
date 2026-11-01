// The bottom panel: Checks · Line list · Equipment · Instruments · Valves. The lists come from the
// engine (lists.rs) in display units; sort, filter and group them, click a row to find it on the
// drawing, edit a cell and the drawing follows.
import { For, Show, createSignal, createMemo, createResource, createEffect, on } from "solid-js";
import { List, Col, Row, parseNum, Issue } from "./api";
import { panel, setPanel, panelOpen, setPanelOpen, derived, call, out, act, proj, toast } from "./store";
import { fromU } from "./units";
import { C } from "./cmds";
import { Help } from "./help";
import { openDatasheet, issueDialog, importDatasheets, importStreams } from "./datasheet";
import { CommentsPanel } from "./review";
import { ADD, WIRING_TABS, addWiring } from "./wiring";
import { TodoPanel, todos } from "./todo";
import { designReport, deleteReport } from "./reports";

const TABS: [string, string][] = [...[["checks", "Checks"]] as [string, string][], ["todo", "To do"], ["comments", "Comments"], ["lines", "Line list"], ["streams", "Streams"], ["equipment", "Equipment"], ["instruments", "Instruments"], ["io", "I/O summary"], ["valves", "Valves"], ["takeoff", "Take-off"], ...WIRING_TABS, ["datasheets", "Datasheets"]];

const level = (l: string) => (l === "error" ? 0 : l === "warning" ? 1 : 2);

const Checks = () => {
  const [show, setShow] = createSignal<Record<string, boolean>>({ error: true, warning: true, note: true });
  const issues = createMemo(() => [...(derived()?.issues ?? [])].sort((a, b) => level(a.level) - level(b.level) || a.rule - b.rule).filter(i => show()[i.level]));
  const sheetNo = (id: number) => proj()?.sheets.find(s => s.id === id)?.number ?? "";
  const go = (i: Issue) => C.goTo(i.ids, i.sheet);
  const n = (l: string) => (derived()?.issues ?? []).filter(i => i.level === l).length;
  return (
    <div class="checks">
      <div class="cfilter">
        <For each={[["error", "Errors"], ["warning", "Warnings"], ["note", "Notes"]]}>{([k, l]) => <label class={`chk ${k}`}><input type="checkbox" checked={show()[k]} onChange={e => setShow({ ...show(), [k]: e.currentTarget.checked })} />{n(k)} {l.toLowerCase()}</label>}</For>
        <span class="grow" /><span class="muted small">Click a row to go to it. Rules can be switched off in Options › Checks.</span>
      </div>
      <div class="cscroll">
        <Show when={issues().length} fallback={<div class="cempty"><b>✓ Nothing to report</b><span class="muted">{(derived()?.issues.length ?? 0) ? "Nothing at the levels shown." : "Every check passes: no duplicate tags, no open ends, sizes and classes change where they should, pumps have check valves, vessels have relief."}</span></div>}>
          <table class="tbl ctbl"><thead><tr><th style={{ width: "30px" }} /><th style={{ width: "42px" }}>Rule</th><th>What</th><th style={{ width: "90px" }}>Sheet</th></tr></thead><tbody>
            <For each={issues()}>{i => <tr class={`f ${i.level}`} onClick={() => go(i)}><td class="lvl">{i.level === "error" ? "✗" : i.level === "warning" ? "!" : "i"}</td><td class="mono">{i.rule}</td><td>{i.msg}</td><td class="mono">{sheetNo(i.sheet)}</td></tr>}</For>
          </tbody></table>
        </Show>
      </div>
    </div>
  );
};

/** One list as a grid. */
const ListView = (p: { kind: string }) => {
  // Reload whenever the project changes.
  const [list, { refetch }] = createResource(() => p.kind, k => call<List>("list", { kind: k }));
  createEffect(on(() => out(), () => refetch(), { defer: true }));
  const [sort, setSort] = createSignal<{ col: number; dir: 1 | -1 } | null>(null);
  const [q, setQ] = createSignal("");
  const [group, setGroup] = createSignal(-1);
  const heads = createMemo(() => new Set((list()?.heads ?? []).map(i => list()!.rows[i])));
  const rows = createMemo(() => {
    const l = list(); if (!l) return [] as Row[];
    const s = q().trim().toLowerCase();
    // A report's own groups stand while it is shown as designed.
    if (heads().size && !s && !sort() && group() < 0) return l.rows;
    let r = l.rows.filter(x => !heads().has(x) && (!s || x.cells.some(c => c.toLowerCase().includes(s))));
    const so = sort();
    if (so) r = [...r].sort((a, b) => { const x = a.cells[so.col], y = b.cells[so.col]; const nx = parseFloat(x), ny = parseFloat(y); const c = !isNaN(nx) && !isNaN(ny) && l.cols[so.col].num ? nx - ny : x.localeCompare(y, undefined, { numeric: true }); return c * so.dir; });
    if (group() >= 0) r = [...r].sort((a, b) => a.cells[group()].localeCompare(b.cells[group()], undefined, { numeric: true }));
    return r;
  });
  const edit = (r: Row, c: Col, v: string) => {
    if (c.key === "tag") return act({ SetTag: { id: r.ids[0], tag: v } });
    let val: string | number | null = v.trim() === "" ? null : v;
    if (c.q && val !== null) { const x = parseNum(v); if (x === null) return toast(`${c.label}: type a number.`); val = fromU(c.q, x); }
    act({ SetData: { ids: r.ids, key: c.key, val } });
  };
  return (
    <div class="lists">
      <div class="cfilter">
        <input class="lsearch" placeholder="Filter" value={q()} onInput={e => setQ(e.currentTarget.value)} />
        <label class="inline small">Group by <select onChange={e => setGroup(+e.currentTarget.value)}><option value="-1">—</option><For each={list()?.cols ?? []}>{(c, i) => <option value={i()}>{c.label}</option>}</For></select></label>
        <span class="grow" /><span class="muted small">{rows().filter(r => !heads().has(r)).length} rows</span>
        <Show when={p.kind.startsWith("report:")} fallback={<button class="btn sm" title="Design a report from this list: its columns, filters and grouping, saved in the project" onClick={() => designReport({ source: p.kind, name: "" })}>Save as report…</button>}>
          <button class="btn sm" onClick={() => { const r = proj()?.settings.reports.find(x => `report:${x.name}` === p.kind); if (r) designReport(r, r.name); }}>Edit report…</button>
          <button class="btn sm" onClick={() => deleteReport(p.kind.slice(7))}>Delete report</button>
        </Show>
        <Show when={p.kind === "datasheets"}><button class="btn sm" title="Import datasheet values from an Excel workbook, with a preview first" onClick={importDatasheets}>Import Excel…</button></Show>
        <Show when={p.kind === "streams"}><button class="btn sm" title="Import a simulator's stream results (Excel or CSV), with a preview first" onClick={importStreams}>Import results…</button></Show>
        <Show when={ADD[p.kind]}><button class="btn sm" onClick={() => addWiring(ADD[p.kind])}>Add {p.kind === "jbs" ? "a junction box" : p.kind === "cables" ? "a cable" : "an I/O card"}…</button></Show>
        <Show when={p.kind === "datasheets" && rows().length}><button class="btn sm" title="Issue every datasheet shown (use the filter to choose), each with its next revision" onClick={() => issueDialog(rows().map(r => r.ids[0]))}>Issue {rows().length} shown…</button></Show>
        <button class="btn sm" onClick={() => C.exportList(p.kind, "xlsx")}>Excel</button><button class="btn sm" onClick={() => C.exportList(p.kind, "csv")}>CSV</button><button class="btn sm" onClick={C.print}>Print</button>
      </div>
      <div class="lscroll">
        <Show when={list()} fallback={<div class="cempty"><span class="muted">{list.error ? String(list.error) : "…"}</span></div>}>{l => <table class="egrid">
          <thead><tr><For each={l().cols}>{(c, i) => <th classList={{ r: c.num }} onClick={() => setSort(sort()?.col === i() ? { col: i(), dir: (-sort()!.dir) as 1 | -1 } : { col: i(), dir: 1 })}>{c.label}{sort()?.col === i() ? (sort()!.dir > 0 ? " ▲" : " ▼") : ""}</th>}</For></tr></thead>
          <tbody>
            <For each={rows()} fallback={<tr><td colSpan={l().cols.length} class="muted" style={{ padding: "8px" }}>{l().kind === "takeoff" ? "Valves and fittings on the P&IDs appear here, counted by type, size, pipe class and rating." : l().kind === "io" ? "Transmitters, switches and control valves appear here, counted by control system (Inspector › Wiring › Control system) and I/O kind." : l().kind === "streams" ? "Streams appear here: draw process lines on a PFD sheet." : l().kind === "lines" ? "Lines with a number appear here: draw a process line and give it a size, fluid, sequence and class." : "Tagged items appear here as you place them."}</td></tr>}>{(r, ri) => <>
              <Show when={group() >= 0 && (ri() === 0 || rows()[ri() - 1].cells[group()] !== r.cells[group()])}><tr class="grow"><td colSpan={l().cols.length}><b>{l().cols[group()].label}: {r.cells[group()] || "—"}</b></td></tr></Show>
              <Show when={heads().has(r)} fallback={<tr class="f" classList={{ bad: r.status === 2, warn: r.status === 1 }} onClick={() => p.kind === "datasheets" ? openDatasheet(r.ids[0]) : C.goTo(r.ids, r.sheet)}>
                <For each={l().cols}>{(c, i) => <td classList={{ r: c.num }}>
                  <Show when={c.edit} fallback={<span>{r.cells[i()]}</span>}><input value={r.cells[i()]} classList={{ mono: c.num }} onClick={e => e.stopPropagation()} onChange={e => edit(r, c, e.currentTarget.value)} /></Show>
                </td>}</For>
              </tr>}><tr class="ghead"><For each={r.cells}>{(c, i) => <td classList={{ r: l().cols[i()].num }}><b>{c}</b></td>}</For></tr></Show>
            </>}</For>
          </tbody>
        </table>}</Show>
      </div>
    </div>
  );
};

export const Bottom = () => {
  const [h, setH] = createSignal(230);
  const resize = (e: MouseEvent) => {
    const y0 = e.clientY, h0 = h();
    const mv = (m: MouseEvent) => setH(Math.max(90, Math.min(innerHeight * 0.7, h0 + y0 - m.clientY)));
    const upf = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", upf); };
    addEventListener("mousemove", mv); addEventListener("mouseup", upf);
  };
  const badge = (k: string) => { if (k === "todo") { const n = todos()?.length ?? 0; return n ? ` (${n})` : ""; } if (k === "comments") { const n = (proj()?.comments ?? []).filter(c => !c.resolved).length; return n ? ` (${n})` : ""; } if (k !== "checks") return ""; const i = derived()?.issues ?? []; const e = i.filter(x => x.level === "error").length, w = i.filter(x => x.level === "warning").length; return e || w ? ` (${e} · ${w})` : ""; };
  return (
    <section class="bottom" style={{ height: panelOpen() ? `${h()}px` : "30px" }}>
      <Show when={panelOpen()}><div class="bhandle" onMouseDown={resize} /></Show>
      <nav class="btabs">
        <For each={[...TABS, ...(proj()?.settings.reports ?? []).map(r => [`report:${r.name}`, r.name] as [string, string])].filter(([k]) => (k !== "streams" || proj()?.sheets.some(s => s.pfd)) && (!ADD[k] || panel() === k || !!proj()?.objects.some(o => o.class === ADD[k])))}>{([k, l]) => <button classList={{ on: panelOpen() && panel() === k }} onClick={() => { if (panel() === k && panelOpen()) setPanelOpen(false); else { setPanel(k); setPanelOpen(true); } }}>{l}{badge(k)}</button>}</For>
        <Help k={panel() === "checks" ? "checks" : panel() === "comments" ? "review" : panel() === "todo" ? "todo" : panel().startsWith("report:") ? "reports" : "lists"} />
        <span class="grow" />
        <button class="bcollapse" title={panelOpen() ? "Hide the panel" : "Show the panel"} onClick={() => setPanelOpen(!panelOpen())}>{panelOpen() ? "▾" : "▴"}</button>
      </nav>
      <Show when={panelOpen()}>
        <Show when={panel() === "checks"} fallback={<Show when={panel() === "comments"} fallback={<Show when={panel() === "todo"} fallback={<ListView kind={panel()} />}><TodoPanel /></Show>}><CommentsPanel /></Show>}><Checks /></Show>
      </Show>
    </section>
  );
};
