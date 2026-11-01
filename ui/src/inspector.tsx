// The inspector: the selected item's or line's data with units, its connections and (for lines)
// the hydraulics; with nothing selected, the sheet and the title block.
import { For, Show, JSX, createMemo, createResource } from "solid-js";
import { Field, Item, Line, Note, Rev, Val, Change, Underlay, num, parseNum } from "./api";
import { sel, proj, sheet, itemOf, lineOf, objOf, symOf, lib, act, derived, setSel, toast, call } from "./store";
import { toU, fromU, unit, fmt } from "./units";
import { Help } from "./help";
import { C } from "./cmds";
import { Check, Pick } from "./ui";
import { issueRevision, compareSheet } from "./revisions";
import { openDatasheet } from "./datasheet";
import { openLoops } from "./loops";

const Row = (p: { label: string; unit?: string; children: JSX.Element; hint?: string }) => (
  <div class="iprow"><label>{p.label}{p.unit && <span class="unit">{p.unit}</span>}</label>{p.children}{p.hint && <span class="hint">{p.hint}</span>}</div>
);

/** Shows a data value in display units and sends the typed one back in SI. */
const FieldInput = (p: { f: Field; v: Val | undefined; ids: number[]; placeholder?: string }) => {
  const shown = () => {
    if (p.v === undefined || p.v === "") return "";
    if (p.f.q && typeof p.v === "number") return num(toU(p.f.q, p.v), 6);
    return String(p.v);
  };
  const set = (t: string) => {
    let val: Val | null = t.trim() === "" ? null : t;
    if (p.f.q && val !== null) {
      const x = parseNum(t);
      if (x === null) { toast(`${p.f.label}: type a number.`); return; }
      val = fromU(p.f.q, x);
    }
    act({ SetData: { ids: p.ids, key: p.f.key, val } });
  };
  return (
    <Row label={p.f.required ? `${p.f.label} *` : p.f.label} unit={p.f.q ? unit(p.f.q) : undefined}>
      <Show when={p.f.choices.length} fallback={<input value={shown()} placeholder={p.placeholder} class={p.f.q ? "mono" : undefined} list={p.f.key === "class" && proj()?.settings.specs.length ? "specnames" : undefined} onChange={e => set(e.currentTarget.value)} />}>
        <Pick value={shown()} set={set} options={[["", "—"], ...p.f.choices.filter(c => c).map(c => [c, c] as [string, string])]} />
      </Show>
    </Row>
  );
};

/** The recorded changes of an object: who changed what, when, from and to. */
const History = (p: { id: number }) => {
  const [rows] = createResource(() => [p.id, proj()] as const, ([id]) => call<Change[]>("changes", { id }));
  const when = (t: number) => new Date(t * 1000).toLocaleString([], { dateStyle: "short", timeStyle: "short" });
  return <Show when={rows()?.length}>
    <h4>History <Help k="history" /></h4>
    <table class="vmini hist"><tbody><For each={rows()!.slice(0, 30)}>{c => <tr title={`${when(c.time)} · ${c.user}`}>
      <td class="muted">{when(c.time)}</td><td>{c.what === "set" || c.what === "clear" ? c.key : c.what}</td><td class="r">{c.what === "add" ? "" : `${c.old || "—"} → ${c.new || "—"}`}</td>
    </tr>}</For></tbody></table>
  </Show>;
};

const ItemPanel = (p: { it: Item }) => {
  const s = () => symOf(p.it.sym);
  const o = () => objOf(p.it.obj);
  const v = () => derived()?.view?.items.find(i => i.id === p.it.id);
  const conns = createMemo(() => {
    const pr = proj(); if (!pr) return [];
    return (s()?.ports ?? []).map((port, k) => {
      const ls = pr.lines.filter(l => (l.a.at === "port" && l.a.item === p.it.id && l.a.port === k) || (l.b.at === "port" && l.b.item === p.it.id && l.b.port === k));
      return { port: port.name, lines: ls.map(l => derived()?.numbers[l.id] || lib()?.kinds.find(x => x[0] === l.kind)?.[1] || "line"), ids: ls.map(l => l.id) };
    });
  });
  const partner = () => proj()?.items.find(i => i.id === p.it.link);
  return <>
    <div class="iwhat"><b>{s()?.name ?? p.it.sym}</b><span class="muted small"> · {lib()?.classes.find(c => c.name === o()?.class)?.label ?? o()?.class} · {s()?.cat_name}</span><Help k={`cat:${s()?.cat ?? "mark"}`} /></div>
    <Show when={s()?.cat !== "mark" || s()?.prefix}>
      <Row label="Tag" hint={s()?.cat === "instrument" || s()?.cat === "primary" ? "Letters and loop number, e.g. LIC-101 (ISA-5.1)." : undefined}>
        <input class="mono" value={o()?.tag ?? ""} onChange={e => act({ SetTag: { id: p.it.obj, tag: e.currentTarget.value } })} />
      </Row>
    </Show>
    <div class="ibtns"><button class="btn sm" onClick={C.rotate} title="Turn a quarter clockwise (R)">Rotate</button><button class="btn sm" onClick={C.flip} title="Mirror left to right (M)">Mirror</button>
      <label class="inline small">Scale <select onChange={e => act({ SetItem: { item: { ...p.it, scale: +e.currentTarget.value } } })}><For each={[0.5, 0.75, 1, 1.25, 1.5, 2]}>{x => <option value={x} selected={Math.abs(p.it.scale - x) < 1e-9}>{x}×</option>}</For></select></label></div>
    <Show when={s()?.cat === "connector"}>
      <h4>Off-page partner <Help k="offpage" /></h4>
      <p class="small">{partner() ? <>Pairs with the connector on <b>{proj()?.sheets.find(x => x.id === partner()!.sheet)?.number}</b>. <button class="btn sm" onClick={() => C.goPartner(p.it.id)}>Go there</button></> : <span class="warnline">No partner yet: pair it with a connector on another sheet.</span>}</p>
      <button class="btn sm" onClick={() => C.pairDialog(p.it.id)}>{partner() ? "Change partner…" : "Pair with…"}</button>
    </Show>
    <Show when={o()}>{ob => <Row label="Area" hint={`Shown ${proj()!.items.filter(i => i.obj === ob().id).length}× · ${[...new Set(proj()!.items.filter(i => i.obj === ob().id).map(i => proj()!.sheets.find(s => s.id === i.sheet)?.number))].join(", ")}`}>
      <Pick value={String(ob().node)} set={v => act({ SetNode: { ids: [ob().id], node: +v } })} options={(proj()?.nodes ?? []).filter(n => n.level === "area").map(n => [String(n.id), proj()!.nodes.find(u => u.id === n.parent)?.name ? `${proj()!.nodes.find(u => u.id === n.parent)!.name} › ${n.name}` : n.name] as [string, string])} />
    </Row>}</Show>
    <Show when={["instrument", "primary", "control"].includes(s()?.cat ?? "")}>
      <div class="ibtns"><button class="btn sm" title="The datasheet: this object's data with the process conditions of its line, to print or export" onClick={() => openDatasheet(p.it.obj)}>Datasheet…</button><button class="btn sm" title="The loop diagram of this instrument's loop, with its wiring" onClick={() => openLoops(String(p.it.obj))}>Loop diagram…</button><Help k="datasheets" /></div>
    </Show>
    <h4>Data</h4>
    <For each={derived()?.attrs[o()?.class ?? ""] ?? s()?.fields ?? []}>{f => <FieldInput f={f} v={o()?.data[f.key]} ids={[p.it.obj]} placeholder={f.key === "loop" ? (o()?.tag.match(/\d+/)?.[0] ?? "") : undefined} />}</For>
    <Show when={conns().length}>
      <h4>Connections</h4>
      <table class="vmini"><tbody><For each={conns()}>{c => <tr classList={{ f: c.ids.length > 0 }} onClick={() => c.ids.length && setSel(c.ids)}><td>{c.port}</td><td class="r">{c.lines.length ? c.lines.join(", ") : <span class="muted">open</span>}</td></tr>}</For></tbody></table>
    </Show>
    <Show when={v() && derived()?.issues.some(i => i.ids.includes(p.it.id))}><Issues id={p.it.id} /></Show>
    <Calcs obj={p.it.obj} />
    <History id={p.it.obj} />
  </>;
};

const Issues = (p: { id: number }) => (
  <><h4>Checks</h4><For each={derived()?.issues.filter(i => i.ids.includes(p.id)) ?? []}>{i => <div class={`issue ${i.level}`}><b>{i.level === "error" ? "✗" : i.level === "warning" ? "!" : "i"}</b> {i.msg}</div>}</For></>
);

const LinePanel = (p: { l: Line }) => {
  const run = () => { const v = derived()?.view?.lines.find(x => x.id === p.l.id); return v?.run ?? null; };
  const ids = () => { const r = run(); return r !== null ? derived()?.runs[r] ?? [p.l.id] : [p.l.id]; };
  const hyd = () => { const r = run(); return r !== null ? derived()?.hyd[r] ?? null : null; };
  const number = () => derived()?.numbers[p.l.id] ?? "";
  const process = () => ["major", "minor", "utility"].includes(p.l.kind);
  const stream = () => objOf(p.l.obj)?.class === "Stream";
  const fields = () => stream() ? derived()?.attrs["Stream"] ?? lib()?.classes.find(c => c.name === "Stream")?.attrs ?? [] : derived()?.attrs["PipingNetworkSystem"] ?? lib()?.line_fields ?? [];
  return <>
    <div class="iwhat"><b>{process() ? stream() ? `Stream ${number()}` : number() || "Line without a number" : lib()?.kinds.find(k => k[0] === p.l.kind)?.[1]}</b><Help k={stream() ? "pfd" : "lines"} /></div>
    <Row label="Line type"><Pick value={p.l.kind} set={k => act({ SetLine: { line: { ...p.l, kind: k } } })} options={(lib()?.kinds ?? []).map(k => [k[0], k[1]] as [string, string])} /></Row>
    <div class="ibtns"><button class="btn sm" onClick={() => C.reverse(p.l.id)} title="Flip the flow arrows">Reverse flow</button><Show when={p.l.pts.length}><button class="btn sm" onClick={() => act({ SetPts: { id: p.l.id, pts: [] } })}>Straighten</button></Show>
      <Show when={ids().length > 1}><button class="btn sm" onClick={() => setSel(ids())}>Select all {ids().length} pieces</button></Show></div>
    <Show when={process()}>
      <Show when={ids().length > 1 && !stream()}><p class="muted small">This line number runs through {ids().length} drawn pieces; data typed here goes to all of them.</p></Show>
      <Show when={hyd()}>{h => <>
        <h4>Hydraulics <Help k="hydraulics" /></h4>
        <div class={["okline", "warnline", "badline"][h().status]}>{["✓ Within the limits", "! Close to or just outside the limits", "✗ Outside the limits"][h().status]} for {h().service_name.toLowerCase()}{h().reason ? `: ${h().reason}` : ""}</div>
        <table class="vmini"><tbody>
          <tr><td>Inside diameter</td><td class="r">{fmt("thk", h().id)}</td></tr>
          <tr><td>Velocity</td><td class="r">{fmt("velocity", h().v)}</td></tr>
          <tr><td>Reynolds number</td><td class="r">{num(h().re, 4)}</td></tr>
          <tr><td>Friction factor (Darcy)</td><td class="r">{num(h().f, 4)}</td></tr>
          <tr><td>Pressure drop</td><td class="r">{fmt("dp100", h().dp100)}</td></tr>
          <tr><td>Over {fmt("length", h().length)}</td><td class="r">{fmt("dp", h().dp)}</td></tr>
          <tr><td>Limits</td><td class="r">{fmt("velocity", h().vmin, 2)} – {fmt("velocity", h().vmax, 2)}, {fmt("dp100", h().dpmax, 3)}</td></tr>
        </tbody></table>
        <p class="muted small">A sizing aid: check it as the engineer.</p>
      </>}</Show>
      <h4>Data</h4>
      <For each={fields()}>{f => <FieldInput f={f} v={objOf(p.l.obj)?.data[f.key]} ids={[p.l.obj]} />}</For>
      <Show when={stream()}><p class="muted small">Enthalpy flow is the stream's heat content from your datum; the heat balance compares what goes in and out of each item with its duty.</p></Show>
      <div class="ibtns" style={{ display: stream() ? "none" : undefined }}><button class="btn sm" title="Give this piece a line number of its own (a copy of this line's data), as after a reducer" onClick={() => act({ NewLineObject: { line: p.l.id } })}>New line number from here</button></div>
      <Calcs obj={p.l.obj} />
      <History id={p.l.obj} />
    </Show>
    <Issues id={p.l.id} />
  </>;
};

const NotePanel = (p: { n: Note }) => (
  <>
    <div class="iwhat"><b>{p.n.kind === "hold" ? "Hold cloud" : p.n.kind === "leader" ? "Leader note" : "Note"}</b><Help k="notes" /></div>
    <Row label="Text"><textarea rows="4" value={p.n.text} onChange={e => act({ SetNote: { note: { ...p.n, text: e.currentTarget.value } } })} /></Row>
    <Show when={p.n.kind === "hold"}><div class="two">
      <Row label="Width" unit="mm"><input class="mono" value={p.n.w} onChange={e => act({ SetNote: { note: { ...p.n, w: Math.max(20, +e.currentTarget.value || 0) } } })} /></Row>
      <Row label="Height" unit="mm"><input class="mono" value={p.n.h} onChange={e => act({ SetNote: { note: { ...p.n, h: Math.max(12, +e.currentTarget.value || 0) } } })} /></Row>
    </div></Show>
  </>
);

const SIZES = ["A3", "A1", "ANSI B", "ANSI D"];

/** Nothing selected: the project, the sheet and the title block. */
const SheetPanel = () => {
  const s = () => sheet()!;
  const p = () => proj()!;
  const setS = (k: string, v: unknown) => act({ SetSheet: { sheet: { ...s(), [k]: v } } });
  const setT = (k: string, v: string) => act({ SetTitle: { title: { ...p().title, [k]: v } } });
  const setRev = (i: number, k: keyof Rev, v: string) => setS("revs", s().revs.map((r, j) => (j === i ? { ...r, [k]: v } : r)));
  return <>
    <div class="iwhat"><b>{s().number}</b><span class="muted small"> · sheet {p().sheets.findIndex(x => x.id === s().id) + 1} of {p().sheets.length}</span><Help k="sheets" /></div>
    <h4>Sheet</h4>
    <div class="two">
      <Row label="Drawing number"><input value={s().number} onChange={e => setS("number", e.currentTarget.value)} /></Row>
      <Row label="Revision"><input value={s().rev} onChange={e => setS("rev", e.currentTarget.value)} /></Row>
    </div>
    <Row label="Title"><input value={s().title} onChange={e => setS("title", e.currentTarget.value)} /></Row>
    <div class="two">
      <Row label="Size"><Pick value={s().size} set={v => setS("size", v)} options={SIZES} /></Row>
      <Row label="Date"><input value={s().date} placeholder="29-Oct-26" onChange={e => setS("date", e.currentTarget.value)} /></Row>
    </div>
    <Row label="Scale"><input value={s().scale} onChange={e => setS("scale", e.currentTarget.value)} /></Row>
    <Show when={!s().legend}><Check label="Process flow diagram (lines are streams, with a stream table)" value={s().pfd} set={v => setS("pfd", v)} /></Show>
    <h4>Old drawing behind <Help k="underlay" /></h4>
    <Show when={s().underlay} fallback={<div class="ibtns"><button class="btn sm" onClick={C.underlay}>Put a DXF behind this sheet…</button></div>}>{u => {
      const setU = (patch: Partial<Underlay>) => setS("underlay", { ...u(), ...patch });
      const mm = (v: string, d: number) => (Number.isFinite(parseFloat(v)) ? parseFloat(v) : d);
      return <div class="ubox">
        <div class="small mono uname" title={u().name}>{u().name}</div>
        <div class="two"><Check label="Show" value={u().show} set={v => setU({ show: v })} /><Check label="Print and export" value={u().print} set={v => setU({ print: v })} /></div>
        <div class="three">
          <Row label="Size" unit="%"><input class="mono" value={+(u().k * 100).toFixed(2)} onChange={e => setU({ k: Math.max(0.1, mm(e.currentTarget.value, u().k * 100)) / 100 })} /></Row>
          <Row label="Left" unit="mm"><input class="mono" value={+u().x.toFixed(1)} onChange={e => setU({ x: mm(e.currentTarget.value, u().x) })} /></Row>
          <Row label="Top" unit="mm"><input class="mono" value={+u().y.toFixed(1)} onChange={e => setU({ y: mm(e.currentTarget.value, u().y) })} /></Row>
        </div>
        <div class="ibtns"><button class="btn sm" onClick={C.underlay}>Replace…</button><button class="btn sm" onClick={() => setS("underlay", null)}>Remove</button></div>
      </div>;
    }}</Show>
    <h4>Revisions <button class="btn sm" title="Issue this sheet at a new revision: a row in the table, and a copy kept to compare later changes with" onClick={() => issueRevision(s())}>Issue…</button><button class="btn sm" title="A row in the revision table only" onClick={() => setS("revs", [...s().revs, { rev: String.fromCharCode(65 + s().revs.length), date: new Date().toLocaleDateString(), desc: "", by: p().title.by, checked: "", approved: "" }])}>Add row</button><Help k="revisions" /></h4>
    <For each={s().revs}>{(r, i) => <div class="revrow">
      <input class="mono" value={r.rev} onChange={e => setRev(i(), "rev", e.currentTarget.value)} style={{ width: "34px" }} />
      <input value={r.date} onChange={e => setRev(i(), "date", e.currentTarget.value)} style={{ width: "80px" }} />
      <input value={r.desc} placeholder="Description" onChange={e => setRev(i(), "desc", e.currentTarget.value)} />
      <button class="x" title="Remove" onClick={() => setS("revs", s().revs.filter((_, j) => j !== i()))}>×</button>
    </div>}</For>
    <Show when={s().snaps.length}>
      <Row label="Revision clouds" hint="Clouds round everything changed since the chosen issue, with the next revision in a triangle. Printed and exported.">
        <Pick value={s().clouds} set={v => setS("clouds", v)} options={[["", "None"], ...s().snaps.map(x => [x.rev, `Changes since revision ${x.rev}`] as [string, string])]} />
      </Row>
      <div class="ibtns"><For each={s().snaps}>{x => <button class="btn sm" onClick={() => compareSheet(s().id, x.rev)}>Compare with {x.rev}…</button>}</For></div>
    </Show>
    <h4>Title block (every sheet)</h4>
    <For each={[["company", "Company"], ["project", "Project"], ["client", "Client"], ["unit", "Unit or area"], ["by", "Drawn by"], ["checked", "Checked by"], ["approved", "Approved by"]] as [string, string][]}>{([k, l]) =>
      <Row label={l}><input value={(p().title as any)[k]} onChange={e => setT(k, e.currentTarget.value)} /></Row>}</For>
    <For each={p().settings.title_block.on ? [...new Set(p().settings.title_block.cells.filter(c => c.field.startsWith("f:")).map(c => c.field.slice(2)))] : []}>{k =>
      <Row label={k}><input value={p().title.extra?.[k] ?? ""} onChange={e => act({ SetTitle: { title: { ...p().title, extra: { ...(p().title.extra ?? {}), [k]: e.currentTarget.value } } } })} /></Row>}</For>
    <h4>Project</h4>
    <Row label="Name"><input value={p().name} onChange={e => act({ SetProject: { name: e.currentTarget.value, units: p().units } })} /></Row>
    <Row label="Units" hint="How values are shown and typed. The file keeps SI units."><Pick value={p().units} set={u => act({ SetProject: { name: p().name, units: u } })} options={[["SI", "SI: bar, °C, m³/h, kg/m³, cP, m"], ["US", "US: psi, °F, gpm, lb/ft³, cP, ft"]]} /></Row>
  </>;
};

/** Calculation sheets in Datum Calc for an object (pump power, NPSH, line pressure drop…). */
const Calcs = (p: { obj: number }) => {
  const [list] = createResource(() => [p.obj, proj()] as const, ([o]) => call<{ id: string; name: string }[]>("calc_sheets", { obj: o }));
  return (
    <Show when={list()?.length}>
      <h4>Calculations <Help k="calcs" /></h4>
      <div class="ibtns"><For each={list()}>{c => <button class="btn sm" title="Save it as a Datum Calc sheet, filled from this object's data" onClick={() => C.send("calc", `${p.obj}:${c.id}`)}>{c.name}…</button>}</For></div>
    </Show>
  );
};

/** The project's pipe classes, offered on a line's Pipe class field. */
const SpecNames = () => <datalist id="specnames"><For each={proj()?.settings.specs ?? []}>{s => <option value={s.name}>{s.desc}</option>}</For></datalist>;

export const Inspector = () => {
  const one = createMemo(() => (sel().length === 1 ? sel()[0] : null));
  const it = () => (one() !== null ? itemOf(one()!) : undefined);
  const ln = () => (one() !== null ? lineOf(one()!) : undefined);
  const nt = () => (one() !== null ? proj()?.notes.find(n => n.id === one()) : undefined);
  return (
    <aside class="insp">
      <SpecNames />
      <header>Inspector<Help k="inspector" /></header>
      <div class="ibody">
        <Show when={proj() && sheet()}>
          <Show when={sel().length > 1}>
            <div class="iwhat"><b>{sel().length} selected</b></div>
            <div class="ibtns"><button class="btn sm" onClick={C.rotate}>Rotate</button><button class="btn sm" onClick={C.flip}>Mirror</button><button class="btn sm" onClick={C.duplicate}>Duplicate</button><button class="btn sm" onClick={C.del}>Delete</button></div>
            <p class="muted small">Drag any of them to move them all; arrow keys nudge them.</p>
          </Show>
          <Show when={it()}>{i => <ItemPanel it={i()} />}</Show>
          <Show when={ln()}>{l => <LinePanel l={l()} />}</Show>
          <Show when={nt()}>{n => <NotePanel n={n()} />}</Show>
          <Show when={!sel().length}><SheetPanel /></Show>
        </Show>
      </div>
    </aside>
  );
};
