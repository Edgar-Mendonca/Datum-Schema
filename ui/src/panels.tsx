// The bottom panel: Checks · Line list · Equipment · Instruments · Valves. The lists come from the
// engine (lists.rs) in display units; sort, filter and group them, click a row to find it on the
// drawing, edit a cell and the drawing follows.
import { For, Show, JSX, createSignal, createMemo, createResource, createEffect, on } from "solid-js";
import { List, Col, Row, parseNum, Issue, Change } from "./api";
import { sel, setPanel, derived, call, out, act, proj, toast, openMenu, openDoc, docTabs, BOTTOM_KEYS, listKey, listOpen, setListOpen, bottomKey, bottomOpen, setBottomOpen, messages } from "./store";
import { fromU } from "./units";
import { C } from "./cmds";
import { Help } from "./help";
import { openDatasheet, issueDialog, importDatasheets, importStreams, importList } from "./datasheet";
import { CommentsPanel } from "./review";
import { ADD, WIRING_TABS, addWiring } from "./wiring";
import { TodoPanel, todos } from "./todo";
import { designReport, deleteReport } from "./reports";
import { addTags, placeRegistered } from "./register";
import { acceptProblem, unacceptProblem } from "./find";

const TABS: [string, string][] = [...[["checks", "Checks"]] as [string, string][], ["todo", "To do"], ["comments", "Comments"], ["lines", "Line list"], ["streams", "Streams"], ["equipment", "Equipment"], ["instruments", "Instruments"], ["io", "I/O summary"], ["valves", "Valves"], ["takeoff", "Take-off"], ...WIRING_TABS, ["datasheets", "Datasheets"], ["register", "Tag register"], ["safety", "Safety"], ["loads", "Loads"], ["hazop", "HAZOP"], ["hazop_recs", "Actions"], ["testpacks", "Packs"], ["accepted", "Accepted"]];

const level = (l: string) => (l === "error" ? 0 : l === "warning" ? 1 : l === "note" ? 2 : 3);
/** The reason a problem was accepted with, if it was. */
const acceptedOf = (i: Issue) => ((proj()?.settings as unknown as { accepted?: { rule: number; ids: number[]; msg: string; reason: string; by: string; date: string }[] }).accepted ?? []).find(a => a.rule === i.rule && a.msg === i.msg && [...a.ids].sort().join() === [...i.ids].sort().join());

/** The fixes the engine offers for a problem, as a menu; the one chosen is one undoable edit. */
const offerFixes = async (e: MouseEvent, i: Issue) => {
  const labels = (await call<string[]>("fixes", { rule: i.rule, ids: i.ids })) ?? [];
  const items = labels.length ? labels.map((l, k) => ({ label: l, run: () => act({ Fix: { rule: i.rule, ids: i.ids, choice: k } }).then(o => o && toast(`Fixed: ${l}. Undo takes it back.`, true)) })) : [{ label: "No fix to offer: go to it and put it right by hand", run: () => C.goTo(i.ids, i.sheet) }];
  items.push(i.level === "accepted" ? { label: "Open it again (no longer accepted)", run: () => unacceptProblem(i) } : { label: "Accept it, with a reason…", run: () => acceptProblem(i) });
  openMenu(e, items);
};

const Checks = () => {
  const [show, setShow] = createSignal<Record<string, boolean>>({ error: true, warning: true, note: true, accepted: false });
  const issues = createMemo(() => [...(derived()?.issues ?? [])].sort((a, b) => level(a.level) - level(b.level) || a.rule - b.rule).filter(i => show()[i.level]));
  const sheetNo = (id: number) => proj()?.sheets.find(s => s.id === id)?.number ?? "";
  const go = (i: Issue) => C.goTo(i.ids, i.sheet);
  const n = (l: string) => (derived()?.issues ?? []).filter(i => i.level === l).length;
  return (
    <div class="checks">
      <div class="cfilter">
        <For each={[["error", "Errors"], ["warning", "Warnings"], ["note", "Notes"], ["accepted", "Accepted"]]}>{([k, l]) => <label class={`chk ${k}`}><input type="checkbox" checked={show()[k]} onChange={e => setShow({ ...show(), [k]: e.currentTarget.checked })} />{n(k)} {l.toLowerCase()}</label>}</For>
        <span class="grow" /><span class="muted small">Click a row to go to it. Rules can be switched off in Options › Checks.</span>
      </div>
      <div class="cscroll">
        <Show when={issues().length} fallback={<div class="cempty"><b>✓ Nothing to report</b><span class="muted">{(derived()?.issues.length ?? 0) ? "Nothing at the levels shown." : "Every check passes: no duplicate tags, no open ends, sizes and classes change where they should, pumps have check valves, vessels have relief."}</span></div>}>
          <table class="tbl ctbl"><thead><tr><th style={{ width: "30px" }} /><th style={{ width: "42px" }}>Rule</th><th>What</th><th style={{ width: "90px" }}>Sheet</th><th style={{ width: "60px" }} /></tr></thead><tbody>
            <For each={issues()}>{i => <tr class={`f ${i.level}`} onClick={() => go(i)}><td class="lvl">{i.level === "error" ? "✗" : i.level === "warning" ? "!" : i.level === "accepted" ? "✓" : "i"}</td><td class="mono">{i.rule}</td><td>{i.msg}<Show when={i.level === "accepted" && acceptedOf(i)}>{a => <div class="muted small acc">Accepted by {a().by || "—"}, {a().date || "—"}: {a().reason}</div>}</Show></td><td class="mono">{sheetNo(i.sheet)}</td><td class="fixc"><button class="btn sm fixb" title="The fixes for this problem" onClick={e => { e.stopPropagation(); offerFixes(e, i); }}>Fix ▾</button></td></tr>}</For>
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
      <Show when={p.kind.startsWith("find:") || p.kind === "accepted"}><div class="ltitle"><b>{list()?.title}</b><Show when={p.kind.startsWith("find:")}><button class="btn sm" onClick={() => import("./find").then(m => m.openFind())}>Change the query…</button></Show></div></Show>
      <div class="cfilter">
        <input class="lsearch" placeholder="Filter" value={q()} onInput={e => setQ(e.currentTarget.value)} />
        <label class="inline small">Group by <select onChange={e => setGroup(+e.currentTarget.value)}><option value="-1">—</option><For each={list()?.cols ?? []}>{(c, i) => <option value={i()}>{c.label}</option>}</For></select></label>
        <span class="grow" /><span class="muted small">{rows().filter(r => !heads().has(r)).length} rows</span>
        <Show when={p.kind.startsWith("report:")} fallback={<Show when={!p.kind.startsWith("find:") && p.kind !== "accepted"}><button class="btn sm" title="Design a report from this list: its columns, filters and grouping, saved in the project" onClick={() => designReport({ source: p.kind, name: "" })}>Save as report…</button></Show>}>
          <button class="btn sm" onClick={() => { const r = proj()?.settings.reports.find(x => `report:${x.name}` === p.kind); if (r) designReport(r, r.name); }}>Edit report…</button>
          <button class="btn sm" onClick={() => deleteReport(p.kind.slice(7))}>Delete report</button>
        </Show>
        <Show when={p.kind === "datasheets"}><button class="btn sm" title="Import datasheet values from an Excel workbook, with a preview first" onClick={importDatasheets}>Import Excel…</button></Show>
        <Show when={p.kind === "register"}><button class="btn sm" title="Add tags before they are drawn: typed, pasted from a spreadsheet, or read from Excel or CSV" onClick={addTags}>Add tags…</button></Show>
        <Show when={p.kind === "streams"}><button class="btn sm" title="Import a simulator's stream results (Excel or CSV), with a preview first" onClick={importStreams}>Import results…</button></Show>
        <Show when={ADD[p.kind]}><button class="btn sm" onClick={() => addWiring(ADD[p.kind])}>Add {p.kind === "jbs" ? "a junction box" : p.kind === "cables" ? "a cable" : "an I/O card"}…</button></Show>
        <Show when={p.kind === "datasheets" && rows().length}><button class="btn sm" title="Issue every datasheet shown (use the filter to choose), each with its next revision" onClick={() => issueDialog(rows().map(r => r.ids[0]))}>Issue {rows().length} shown…</button></Show>
        <button class="btn sm" onClick={() => C.exportList(p.kind, "xlsx")}>Excel</button><Show when={!p.kind.startsWith("find:") && p.kind !== "accepted"}><button class="btn sm" title="Read this list back after editing it in Excel (export it first): the changes are shown before anything changes" onClick={() => importList({ kind: p.kind })}>Read back…</button></Show><button class="btn sm" onClick={() => C.exportList(p.kind, "csv")}>CSV</button><button class="btn sm" onClick={C.print}>Print</button>
      </div>
      <div class="lscroll">
        <Show when={list()} fallback={<div class="cempty"><span class="muted">{list.error ? String(list.error) : "…"}</span></div>}>{l => <table class="egrid" ref={el => createEffect(on(sel, () => queueMicrotask(() => el.querySelector("tr.f.sel")?.scrollIntoView({ block: "nearest" })), { defer: true }))}>
          <thead><tr><For each={l().cols}>{(c, i) => <th classList={{ r: c.num }} onClick={() => setSort(sort()?.col === i() ? { col: i(), dir: (-sort()!.dir) as 1 | -1 } : { col: i(), dir: 1 })}>{c.label}{sort()?.col === i() ? (sort()!.dir > 0 ? " ▲" : " ▼") : ""}</th>}</For></tr></thead>
          <tbody>
            <For each={rows()} fallback={<tr><td colSpan={l().cols.length} class="muted" style={{ padding: "8px" }}>{l().kind === "takeoff" ? "Valves and fittings on the P&IDs appear here, counted by type, size, pipe class and rating." : l().kind === "io" ? "Transmitters, switches and control valves appear here, counted by control system (Inspector › Wiring › Control system) and I/O kind." : l().kind === "streams" ? "Streams appear here: draw process lines on a PFD sheet." : l().kind === "lines" ? "Lines with a number appear here: draw a process line and give it a size, fluid, sequence and class." : "Tagged items appear here as you place them."}</td></tr>}>{(r, ri) => <>
              <Show when={group() >= 0 && (ri() === 0 || rows()[ri() - 1].cells[group()] !== r.cells[group()])}><tr class="grow"><td colSpan={l().cols.length}><b>{l().cols[group()].label}: {r.cells[group()] || "—"}</b></td></tr></Show>
              <Show when={heads().has(r)} fallback={<tr class="f" classList={{ bad: r.status === 2, warn: r.status === 1, sel: r.ids.some(i => sel().includes(i)) }} onClick={() => p.kind === "datasheets" ? openDatasheet(r.ids[0]) : p.kind === "register" && r.sheet === 0 ? placeRegistered(r.ids[0]) : C.goTo(r.ids, r.sheet)}>
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

/** What a list key shows: the Checks, the to-do list, the comments, or a list. Shown in the panel below
 *  the sheet, or full size in a tab beside the sheets. */
export const PanelBody = (p: { k: string }) => (
  <Show when={p.k === "checks"} fallback={<Show when={p.k === "comments"} fallback={<Show when={p.k === "todo"} fallback={<ListView kind={p.k} />}><TodoPanel /></Show>}><CommentsPanel /></Show>}><Checks /></Show>
);
/** A list's title (its tab's name). */
export const tabTitle = (k: string) => TABS.find(t => t[0] === k)?.[1] ?? (k.startsWith("report:") ? k.slice(7) : k);
/** Opens a list as a tab beside the sheets. */
export const openListTab = (k: string) => openDoc({ id: `list:${k}`, title: tabTitle(k), list: k });

/** The app's messages, newest first. */
const MessagesPanel = () => {
  const when = (t: number) => new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  return <div class="cscroll"><table class="tbl ctbl msgtbl"><tbody>
    <For each={[...messages()].reverse()} fallback={<tr><td class="muted" style={{ padding: "8px" }}>What Datum Schema tells you appears here, newest first: saved files, imports, conversions, and anything that went wrong.</td></tr>}>{m => <tr classList={{ warning: !m.ok }}><td class="muted mono">{when(m.time)}</td><td>{m.text}</td></tr>}</For>
  </tbody></table></div>;
};

/** Every recorded change of the project, newest first, with who and when; click one to go to it. */
const LogPanel = () => {
  const [rows] = createResource(() => proj(), () => call<Change[]>("changes", { id: 0 }));
  const when = (t: number) => new Date(t * 1000).toLocaleString([], { dateStyle: "short", timeStyle: "medium" });
  const tagOf = (id: number) => proj()?.objects.find(o => o.id === id)?.tag || "";
  const go = (id: number) => { const it = proj()?.items.find(i => i.obj === id) ?? proj()?.lines.find(l => l.obj === id); if (it) C.goTo([it.id], it.sheet); };
  return <div class="cscroll"><table class="tbl ctbl logtbl"><thead><tr><th>When</th><th>Who</th><th>Object</th><th>What</th><th>Was</th><th>Now</th></tr></thead><tbody>
    <For each={(rows() ?? []).slice(0, 500)} fallback={<tr><td colSpan={6} class="muted" style={{ padding: "8px" }}>Every change to the project is recorded here as you work, with who made it and when.</td></tr>}>{c => <tr class="f" onClick={() => go(c.obj)}>
      <td class="muted mono">{when(c.time)}</td><td>{c.user}</td><td class="mono">{tagOf(c.obj) || `#${c.obj}`}</td><td>{c.what === "set" || c.what === "clear" ? c.key : c.what}</td><td class="muted">{c.old}</td><td>{c.new}</td>
    </tr>}</For>
  </tbody></table></div>;
};

/** A row of tabs over a table area: tabs that do not fit go under More. */
const TabRow = (p: { tabs: [string, string][]; cur: string; open: boolean; pick: (k: string) => void; badge: (k: string) => string; children?: JSX.Element }) => {
  const [hid, setHid] = createSignal<string[]>([]);
  return (
    <nav class="btabs" ref={el => { const fit = () => { const rest = [...el.querySelectorAll<HTMLElement>(":scope > :not(.bt):not(.grow):not(.bmore)")].reduce((w, x) => w + x.getBoundingClientRect().width, 0); const r = el.getBoundingClientRect().right - rest - 96; const bs = [...el.querySelectorAll<HTMLElement>(":scope > button.bt")]; bs.forEach(b => (b.style.display = "")); const off = bs.filter(b => b.getBoundingClientRect().right > r).map(b => b.dataset.k!); bs.forEach(b => { if (off.includes(b.dataset.k!)) b.style.display = "none"; }); setHid(off); }; new ResizeObserver(fit).observe(el); createEffect(on(() => [p.tabs, p.cur], () => queueMicrotask(fit))); }}>
      <For each={p.tabs}>{([k, l]) => <button class="bt" data-k={k} classList={{ on: p.open && p.cur === k }} onClick={() => p.pick(k)}>{l}{p.badge(k)}</button>}</For>
      <Show when={hid().length}><button class="bmore" title="The ones that do not fit" onClick={e => openMenu(e, p.tabs.filter(([k]) => hid().includes(k)).map(([k, l]) => ({ label: `${l}${p.badge(k)}`, on: p.open && p.cur === k, run: () => p.pick(k) })))}>More ▾</button></Show>
      <span class="grow" />
      {p.children}
    </nav>
  );
};

/** A resizable table area's height, kept. */
const useHeight = (key: string, d: number) => {
  const [h, setH] = createSignal(Math.max(90, +(localStorage.getItem(key) ?? d) || d));
  const resize = (e: MouseEvent) => {
    const y0 = e.clientY, h0 = h();
    const mv = (m: MouseEvent) => setH(Math.max(90, Math.min(innerHeight * 0.75, h0 + y0 - m.clientY)));
    const upf = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", upf); try { localStorage.setItem(key, String(h())); } catch { /* private window */ } window.dispatchEvent(new Event("resize")); };
    addEventListener("mousemove", mv); addEventListener("mouseup", upf);
  };
  return [h, resize] as const;
};

const listBadge = () => "";
/** The lists under the drawing, in the drawing's own tab (0.11.3): for the review, the drawing and
 *  its lists side by side, linked both ways. A list moved to its own tab is not shown here. */
/** Every list this project has to show (reports included), in order. */
export const LIST_TABS = (): [string, string][] => [...TABS.filter(([k]) => !BOTTOM_KEYS.includes(k)), ...(proj()?.settings.reports ?? []).map(r => [`report:${r.name}`, r.name] as [string, string])].filter(([k]) => (k !== "streams" || proj()?.sheets.some(s => s.pfd)) && (!k.startsWith("hazop") || listKey() === k || !!(proj()?.settings.hazop as { nodes?: unknown[] } | undefined)?.nodes?.length) && (k !== "accepted" || listKey() === k || !!(proj()?.settings as { accepted?: unknown[] } | undefined)?.accepted?.length) && (k !== "testpacks" || listKey() === k || !!(proj()?.settings.testpacks as { packs?: unknown[] } | undefined)?.packs?.length) && (!ADD[k] || listKey() === k || !!proj()?.objects.some(o => o.class === ADD[k])));
export const ReviewLists = () => {
  const [h, resize] = useHeight("ds.lists.h", 260);
  const tabs = createMemo(() => LIST_TABS().filter(([k]) => !docTabs().some(t => t.list === k) && (k !== "streams" || proj()?.sheets.some(s => s.pfd)) && (!k.startsWith("hazop") || listKey() === k || !!(proj()?.settings.hazop as { nodes?: unknown[] } | undefined)?.nodes?.length) && (k !== "accepted" || listKey() === k || !!(proj()?.settings as { accepted?: unknown[] } | undefined)?.accepted?.length) && (k !== "testpacks" || listKey() === k || !!(proj()?.settings.testpacks as { packs?: unknown[] } | undefined)?.packs?.length) && (!ADD[k] || listKey() === k || !!proj()?.objects.some(o => o.class === ADD[k]))));
  const pick = (k: string) => { setPanel(k); setListOpen(true); };
  const shown = () => listOpen() && !docTabs().some(t => t.list === listKey());
  return (
    <section class="bottom lowerlists" style={{ height: shown() ? `${h()}px` : "30px" }}>
      <Show when={shown()}><div class="bhandle" onMouseDown={resize} /></Show>
      <TabRow tabs={tabs()} cur={listKey()} open={shown()} pick={pick} badge={listBadge}>
        <span class="lcap muted">Lists</span>
        <button class="bcollapse bdoc" title="Move this list to its own tab, full size (for working through it); Show under the drawing brings it back" onClick={() => { openListTab(listKey()); }}>⧉ Own tab</button>
        <Help k={listKey().startsWith("report:") ? "reports" : "lists"} />
        <button class="bcollapse" title={shown() ? "Fold the lists away" : "Show the lists under the drawing"} onClick={() => setListOpen(!listOpen())}>{shown() ? "▾" : "▴"}</button>
      </TabRow>
      <Show when={shown()}><PanelBody k={listKey()} /></Show>
    </section>
  );
};

/** The slim bottom pane (0.11.3): checks, the to-do list, comments, messages and the log. */
export const Bottom = () => {
  const [h, resize] = useHeight("ds.bottom.h", 170);
  const badge = (k: string) => { if (k === "todo") { const n = todos()?.length ?? 0; return n ? ` (${n})` : ""; } if (k === "comments") { const n = (proj()?.comments ?? []).filter(c => !c.resolved).length; return n ? ` (${n})` : ""; } if (k !== "checks") return ""; const i = derived()?.issues ?? []; const e = i.filter(x => x.level === "error").length, w = i.filter(x => x.level === "warning").length; return e || w ? ` (${e} · ${w})` : ""; };
  const tabs: [string, string][] = [["checks", "Checks"], ["todo", "To do"], ["comments", "Comments"], ["messages", "Messages"], ["log", "Log"]];
  const pick = (k: string) => { setPanel(k); setBottomOpen(true); };
  return (
    <section class="bottom slim" style={{ height: bottomOpen() ? `${h()}px` : "30px" }}>
      <Show when={bottomOpen()}><div class="bhandle" onMouseDown={resize} /></Show>
      <TabRow tabs={tabs} cur={bottomKey()} open={bottomOpen()} pick={pick} badge={badge}>
        <Help k={bottomKey() === "checks" ? "checks" : bottomKey() === "comments" ? "review" : bottomKey() === "todo" ? "todo" : "workspace"} />
        <button class="bcollapse" title={bottomOpen() ? "Fold the pane away" : "Show the pane"} onClick={() => setBottomOpen(!bottomOpen())}>{bottomOpen() ? "▾" : "▴"}</button>
      </TabRow>
      <Show when={bottomOpen()}>
        <Show when={bottomKey() === "messages"} fallback={<Show when={bottomKey() === "log"} fallback={<PanelBody k={bottomKey()} />}><LogPanel /></Show>}><MessagesPanel /></Show>
      </Show>
    </section>
  );
};
