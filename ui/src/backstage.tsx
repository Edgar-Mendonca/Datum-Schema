// File (backstage): home with examples and recent projects, export, print, options (including the
// check rules and hydraulic limits), roadmap, updates. From Datum Plan by way of Datum Pipe.
import { LayoutSec } from "./datasheet";
import { For, Show, createSignal, createResource, onMount, JSX } from "solid-js";
import { PrintPage } from "./print";
import { RoadmapPage } from "./roadmap";
import { Options, Out, Example, Settings, Limit, Field, UserAttr, listen, invoke, num, parseNum } from "./api";
import { backstage, setBackstage, call, out, setOut, opts, setOpts, toast, setThemeRaw, applyTheme, proj, act, lib } from "./store";
import { C } from "./cmds";
import * as D from "./dialogs";
import { Icon, Text, Pick, Check } from "./ui";
import { toU, fromU, unit } from "./units";
import { RULES } from "./help";

type Info = { current: string; latest: string | null; beta: boolean; available: boolean; notes: string; published: string; url: string | null; sums: string | null; size: number; page: string };
export const [update, setUpdate] = createSignal<Info | null>(null);
export const [checking, setChecking] = createSignal(false), [checkedAt, setCheckedAt] = createSignal("");
export const checkUpdate = async (quiet: boolean) => {
  setChecking(true);
  try { setUpdate(await invoke<Info>("check_update")); setCheckedAt(new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })); if (!quiet) toast(update()?.available ? `Version ${update()!.latest} is available.` : "You have the latest version.", true); }
  catch (e) { if (!quiet) toast(`Could not check for updates: ${e}`); }
  finally { setChecking(false); }
};
const GH = "https://github.com/Edgar-Mendonca/Datum-Schema";
export const GitHubLinks = () => (
  <div class="ghlinks">
    <For each={[["", "GitHub", "The project and its code"], ["/releases", "Releases", "Every version and its change log"], ["/issues/new", "Report an issue", "Tell us about a bug or an idea"]]}>{([path, l, t]) =>
      <button class="btn" title={t} onClick={() => call("open_url", { url: GH + path })}><svg class="ic"><use href={path === "/issues/new" ? "#i-bug" : path ? "#i-tag" : "#i-github"} /></svg>{l}</button>}</For>
  </div>
);
const md = (s: string) => s.split("\n").map(l => {
  const e = l.replace(/[&<>]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]!).replace(/\*\*(.+?)\*\*/g, "<b>$1</b>").replace(/`(.+?)`/g, "<code>$1</code>");
  return l.startsWith("## ") ? `<h3>${e.slice(3)}</h3>` : l.startsWith("- ") ? `<li>${e.slice(2)}</li>` : e.trim() ? `<p>${e}</p>` : "";
}).join("");

/** A small drawing for each example tile. */
const ExampleArt = (p: { id: string }) => {
  const s = { fill: "none", stroke: "var(--blue)", "stroke-width": 2, "stroke-linecap": "round" as const, "stroke-linejoin": "round" as const };
  const pics: Record<string, string> = {
    blank: "M14 10h72v40H14zM62 42h24M62 42v8",
    pump: "M10 14h18v32H10zM28 40h14M48 40a6 6 0 1 0 0.1 0M54 34h16v-14h10M80 12h10v28H80z",
    faults: "M10 14h18v32H10zM28 40h14M48 40a6 6 0 1 0 0.1 0M54 34h16v-14h10M80 12h10v28H80zM70 46l8 8M78 46l-8 8",
    compressor: "M10 10h14v36H10zM24 40h16M40 34l14-4v20l-14-4zM54 40h14v-24h-50M72 8h16v10H72z",
    exchanger: "M20 24h50v14H20zM10 31h10M70 31h10M34 10v14M56 38v14M44 6h4",
  };
  return <svg viewBox="0 0 100 60" class="exart"><path d={pics[p.id] ?? pics.blank} {...s} /></svg>;
};

const Home = () => {
  const [ex] = createResource(() => call<Example[]>("examples_list"));
  const [recent] = createResource(() => call<{ path: string; name: string; modified: string }[]>("recent"));
  return (
    <div class="home">
      <div>
        <h1>Good {new Date().getHours() < 12 ? "morning" : new Date().getHours() < 18 ? "afternoon" : "evening"}</h1>
        <div class="tiles">
          <button class="tile" onClick={() => C.newProject(null)}><div class="pic"><ExampleArt id="blank" /></div><b>Blank project</b><span>One A3 sheet with its border and title block.</span></button>
          <For each={ex() ?? []}>{t => <button class="tile" onClick={() => C.newProject(t.id)}><div class="pic"><ExampleArt id={t.id} /></div><b>{t.name}</b><span>{t.what}</span></button>}</For>
          <button class="tile" onClick={() => C.importDexpi()}><div class="pic"><svg class="ic"><use href="#i-import" /></svg></div><b>From a DEXPI file</b><span>Open a P&ID saved in the DEXPI exchange format (XML) by Datum Schema or another program.</span></button>
          <button class="tile" onClick={() => setBackstage("roadmap")}><div class="pic"><svg class="ic"><use href="#i-list" /></svg></div><b>Roadmap</b><span>Everything Datum Schema will do, and when.</span></button>
        </div>
        <h2>Recent</h2>
        <table class="recent"><tbody>
          <For each={recent() ?? []} fallback={<tr><td class="muted">Projects you open or save appear here.</td></tr>}>{r => <tr class="f" onClick={() => C.openPath(r.path)}><td><b>{r.name}</b><div class="p">{r.path}</div></td><td class="p">{r.modified}</td></tr>}</For>
        </tbody></table>
      </div>
      <UpdateCard />
    </div>
  );
};

const UpdateCard = () => {
  const [pct, setPct] = createSignal<number | null>(null);
  const [ver] = createResource(() => call<string>("version"));
  onMount(() => { listen("update-progress", (p: number) => setPct(p)); if (!update()) checkUpdate(true); });
  const install = async () => { const u = update(); if (!u?.url) return; setPct(0); await call("install_update", { url: u.url, sums: u.sums }); setPct(null); };
  return (
    <div class="card">
      <div class="brand"><img src="logo.svg" alt="" /><div><b>Datum Schema</b><span class="muted">Version {ver()}{opts().beta ? " · beta channel" : ""}</span></div></div>
      <Show when={update()} fallback={<><span class="pill">{checking() ? "Checking for updates…" : "Not checked yet"}</span><button class="btn wide" disabled={checking()} onClick={() => checkUpdate(false)}>{checking() ? <><i class="spin" /> Checking…</> : "Check for updates"}</button><GitHubLinks /></>}>{u => <>
        <Show when={u().available} fallback={<span class="pill ok">You have the latest version</span>}><span class="pill warn">Version {u().latest}{u().beta ? " (beta)" : ""} is ready</span></Show>
        <Show when={u().available && u().url}><button class="btn pri wide" disabled={pct() !== null} onClick={install}>{pct() === null ? `Update now (${Math.round(u().size / 1048576)} MB)` : `Downloading… ${pct()}%`}</button></Show>
        <Show when={pct() !== null}><div class="prog"><i style={{ width: `${pct()}%` }} /></div></Show>
        <button class="btn wide" disabled={checking()} onClick={() => checkUpdate(false)}>{checking() ? <><i class="spin" /> Checking…</> : "Check again"}</button>
        <Show when={checkedAt() && !checking()}><span class="muted small">Checked at {checkedAt()}</span></Show>
        <GitHubLinks />
        <div class="notes"><h4>What's new{u().latest ? ` in ${u().latest}` : ""}</h4><div innerHTML={md(u().notes || "Release notes will appear here.")} /></div>
      </>}</Show>
    </div>
  );
};

const Export = () => (
  <div><h1>Export</h1>
    <div class="tiles">
      <button class="tile" onClick={() => setBackstage("print")}><div class="pic"><svg class="ic"><use href="#i-print" /></svg></div><b>Print or save as PDF</b><span>The sheets at their size or fitted to A3, the lists and the legend.</span></button>
      <button class="tile" onClick={() => C.exportDxf()}><div class="pic"><svg class="ic"><use href="#i-dxf" /></svg></div><b>DXF of this sheet</b><span>R12 DXF: a layer per line type and symbol category, symbols as blocks with their tags. Opens in any CAD program.</span></button>
      <button class="tile" onClick={() => C.exportDexpi()}><div class="pic"><svg class="ic"><use href="#i-export" /></svg></div><b>DEXPI of this sheet</b><span>The DEXPI P&ID exchange format (XML): equipment, line numbers, valves, instruments, signal lines and their data, for other P&ID programs.</span></button>
      <button class="tile" onClick={() => C.exportSvg()}><div class="pic"><svg class="ic"><use href="#i-svg" /></svg></div><b>SVG of this sheet</b><span>The sheet as a vector picture, for documents and the web.</span></button>
      <For each={[["lines", "Line list"], ["equipment", "Equipment list"], ["instruments", "Instrument index"], ["valves", "Valve list"], ["datasheets", "Datasheet index"]]}>{([k, l]) =>
        <button class="tile" onClick={() => C.exportList(k, "xlsx")}><div class="pic"><svg class="ic"><use href="#i-xlsx" /></svg></div><b>{l} to Excel</b><span>Or to CSV: <a href="#" onClick={e => { e.preventDefault(); e.stopPropagation(); C.exportList(k, "csv"); }}>save as CSV</a>.</span></button>}</For>
    </div>
  </div>
);

const Opt = (p: { l: string; hint?: string; children: JSX.Element }) => <div class="opt"><div><b>{p.l}</b>{p.hint && <small>{p.hint}</small>}</div><div class="ctl">{p.children}</div></div>;
const Sec = (p: { t: string; about: string; children: JSX.Element }) => <section class="osec"><h3>{p.t}</h3><p class="muted">{p.about}</p>{p.children}</section>;

/** Options › Classes: every class with its attributes; the project adds its own (and marks any as
 *  required: check 6 then reports objects where it is empty). */
const ClassesSec = () => {
  const [cls, setCls] = createSignal("Pump");
  const [label, setLabel] = createSignal(""), [q, setQ] = createSignal(""), [choices, setChoices] = createSignal(""), [req, setReq] = createSignal(false);
  const info = () => lib()?.classes.find(c => c.name === cls());
  const own = () => (proj()?.settings.user_attrs ?? []).map((u, i) => [u, i] as const).filter(([u]) => u.class === cls());
  const set = (user_attrs: UserAttr[]) => act({ SetSettings: { settings: { ...proj()!.settings, user_attrs } } });
  const add = () => {
    const l = label().trim(); if (!l) return toast("Give the attribute a name.");
    const key = l.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "") || "attr";
    const ch = choices().split(",").map(x => x.trim()).filter(Boolean);
    const attr: Field = { key, label: l, q: q(), type: ch.length ? "choice" : q() ? "number" : "text", choices: ch, dexpi: "", required: req(), from: cls() };
    set([...(proj()!.settings.user_attrs ?? []), { class: cls(), attr }]).then(o => { if (o) { setLabel(""); setChoices(""); setReq(false); toast(`${l} added to ${info()?.label ?? cls()} and every class under it.`, true); } });
  };
  return <Sec t="Classes" about="Every kind of object is a class with attributes, inherited from the classes above it (a centrifugal pump has a pump's and an equipment item's attributes). Add your company's attributes to any class; they reach every class under it and are saved in the project. Mark one required and check 6 reports where it is empty.">
    <div class="classes">
      <select size="12" onChange={e => setCls(e.currentTarget.value)}><For each={lib()?.classes ?? []}>{c => <option value={c.name} selected={c.name === cls()}>{c.label}{c.symbols.length ? "" : " (group)"}</option>}</For></select>
      <div>
        <p class="muted small">{info()?.name}{info()?.parent ? ` · under ${lib()?.classes.find(c => c.name === info()!.parent)?.label}` : ""}</p>
        <table class="tbl small"><thead><tr><th>Attribute</th><th>Unit</th><th>From</th><th></th></tr></thead><tbody>
          <For each={info()?.attrs ?? []}>{a => <tr><td>{a.label}{a.required ? " *" : ""}</td><td class="muted">{a.q ? unit(a.q) : a.choices.length ? "choice" : "text"}</td><td class="muted">{lib()?.classes.find(c => c.name === a.from)?.label ?? a.from}</td><td /></tr>}</For>
          <For each={own()}>{([u, i]) => <tr><td><b>{u.attr.label}{u.attr.required ? " *" : ""}</b></td><td class="muted">{u.attr.q ? unit(u.attr.q) : u.attr.choices.length ? "choice" : "text"}</td><td class="muted">This project</td><td><button class="x" title="Remove" onClick={() => set(proj()!.settings.user_attrs.filter((_, j) => j !== i))}>×</button></td></tr>}</For>
        </tbody></table>
        <div class="addattr">
          <input placeholder="New attribute, e.g. Seal plan" value={label()} onInput={e => setLabel(e.currentTarget.value)} />
          <Pick value={q()} set={setQ} options={[["", "Text"], ["press", "Pressure"], ["temp", "Temperature"], ["flow", "Volume flow"], ["mflow", "Mass flow"], ["density", "Density"], ["visc", "Viscosity"], ["length", "Length"], ["thk", "Thickness"], ["power", "Power"], ["head", "Head"], ["volume", "Volume"], ["area", "Area"], ["velocity", "Velocity"]]} />
          <input placeholder="Choices, comma-separated (optional)" value={choices()} onInput={e => setChoices(e.currentTarget.value)} />
          <Check label="Required" value={req()} set={setReq} />
          <button class="btn sm pri" onClick={add}>Add attribute</button>
        </div>
      </div>
    </div>
  </Sec>;
};

/** The open project's check rules, line number format and hydraulic limits. */
const ProjectSettings = () => {
  const s = () => proj()!.settings;
  const set = (x: Partial<Settings>) => act({ SetSettings: { settings: { ...s(), ...x } } });
  const setLim = (i: number, k: keyof Limit, v: string) => {
    const x = parseNum(v); if (x === null) return toast("Type a number.");
    const si = k === "dp" ? fromU("dp100", x) : fromU("velocity", x);
    set({ limits: s().limits.map((l, j) => (j === i ? { ...l, [k]: si } : l)) });
  };
  return <Show when={proj()}>
    <Sec t="Checks" about="The rules the Checks panel runs on this project. Switch off any your company does not follow; the choice is saved in the project file.">
      <div class="rules"><For each={RULES}>{([n, t, lvl]) => <label class="chk rule"><input type="checkbox" checked={!s().off.includes(n)} onChange={e => set({ off: e.currentTarget.checked ? s().off.filter(x => x !== n) : [...s().off, n] })} /><span class="mono">{n}</span><span>{t}</span><span class={`lvl ${lvl}`}>{lvl}</span></label>}</For></div>
    </Sec>
    <Sec t="Line numbers" about="How a line number is built from its data. Tokens: <size>, <fluid>, <seq>, <class>, <unit>.">
      <Opt l="Format" hint={`Default <size>-<fluid>-<seq>-<class>, e.g. 3"-P-1001-A1A.`}><Text value={s().line_format} set={v => set({ line_format: v })} /></Opt>
      <Opt l="Pipe roughness" hint="For the friction factor; 0.045 mm is commercial steel."><span class="inline"><input type="number" step="0.005" style={{ width: "90px" }} value={num(s().roughness * 1000, 4)} onChange={e => { const x = parseNum(e.currentTarget.value); if (x !== null) set({ roughness: x / 1000 }); }} /> mm</span></Opt>
      <Opt l="Snap grid" hint="Symbols, bends and ends snap to it."><span class="inline"><input type="number" step="0.5" style={{ width: "90px" }} value={s().grid} onChange={e => set({ grid: Math.max(0.5, +e.currentTarget.value || 2.5) })} /> mm</span></Opt>
    </Sec>
    <ClassesSec />
    <Sec t="Datasheets" about="Your company's datasheet form: its number, your logo, a footer, the rows it shows and the order of its sections. Saved with the project; use it for new projects too.">
      <LayoutSec />
    </Sec>
    <Sec t="Hydraulic limits" about="Typical limits by service, used for the badges on the lines and check 13. Change them to your company's practice; a line takes the limits of its service (set on the line, or worked out from a pump's suction or discharge and the phase).">
      <table class="tbl limits"><thead><tr><th>Service</th><th class="r">Min velocity <span class="unit">{unit("velocity")}</span></th><th class="r">Max velocity <span class="unit">{unit("velocity")}</span></th><th class="r">Max pressure drop <span class="unit">{unit("dp100")}</span></th></tr></thead>
        <tbody><For each={s().limits}>{(l, i) => <tr><td>{l.name}</td>
          <td class="r"><input class="mono" value={num(toU("velocity", l.vmin), 3)} onChange={e => setLim(i(), "vmin", e.currentTarget.value)} /></td>
          <td class="r"><input class="mono" value={num(toU("velocity", l.vmax), 3)} onChange={e => setLim(i(), "vmax", e.currentTarget.value)} /></td>
          <td class="r"><input class="mono" value={num(toU("dp100", l.dp), 3)} onChange={e => setLim(i(), "dp", e.currentTarget.value)} /></td></tr>}</For></tbody></table>
      <p><button class="btn sm" onClick={() => { const o = { ...opts(), defaults: s() }; setOpts(o); call("set_options", { opts: o }).then(() => toast("New projects will start with these rules, format and limits.", true)); }}>Use these for new projects</button></p>
    </Sec>
  </Show>;
};

const OptionsPage = () => {
  const [o, setO] = createSignal<Options>({ ...opts() });
  const save = async () => { const r = await call<Out>("set_options", { opts: o() }); if (r) { setOpts(o()); setOut(r); toast("Options saved.", true); } };
  const put = <K extends keyof Options>(k: K, v: Options[K]) => setO({ ...o(), [k]: v });
  return (
    <div class="options"><h1>Options</h1>
      <p class="muted">Settings for Datum Schema on this computer, and the open project's checks and limits.</p>
      <Sec t="General" about="How the window looks.">
        <Opt l="Theme" hint="Auto follows the Windows light or dark setting."><Pick value={o().theme || "auto"} set={v => { put("theme", v); setThemeRaw(v); applyTheme(); }} options={[["auto", "Auto (follow Windows)"], ["light", "Light"], ["dark", "Dark"]]} /></Opt>
        <Opt l="Grid" hint="Show the snap grid behind the sheet."><Check label="Show the grid" value={o().grid} set={v => put("grid", v)} /></Opt>
      </Sec>
      <Sec t="Start-up and saving" about="What happens when Datum Schema starts, and how your work is protected.">
        <Opt l="When Datum Schema starts" hint="Show the File page, or reopen the project you used last."><Pick value={o().openLast ? "last" : "home"} set={v => put("openLast", v === "last")} options={[["home", "Show the File page"], ["last", "Reopen the last project"]]} /></Opt>
        <Opt l="Recovery copy every" hint="A copy is kept while you work, so a crash or power cut loses little. 0 turns it off."><span class="inline"><input type="number" min="0" style={{ width: "70px" }} value={o().autosave} onChange={e => put("autosave", +e.currentTarget.value)} /> minutes</span></Opt>
      </Sec>
      <Sec t="New projects" about="Defaults given to every new project. Each project keeps its own settings afterwards.">
        <Opt l="Units" hint="How values are shown and typed. Files always keep SI units."><Pick value={o().units} set={v => put("units", v as any)} options={[["SI", "SI: bar, °C, m³/h, kg/m³, cP, m"], ["US", "US: psi, °F, gpm, lb/ft³, cP, ft"]]} /></Opt>
        <Opt l="Company" hint="Written in the title block of new projects."><Text value={o().company} set={v => put("company", v)} /></Opt>
        <Opt l="Your name" hint="Written in the history of every change you make. Empty: the computer's user name."><Text value={o().user} set={v => put("user", v)} /></Opt>
      </Sec>
      <Sec t="Updates" about="Datum Schema checks GitHub for new versions when it starts. Updates install from File › Updates.">
        <Opt l="Beta versions" hint="Get new features early. They may have bugs."><Check label="Get beta versions" value={o().beta} set={v => put("beta", v)} /></Opt>
      </Sec>
      <p class="osave"><button class="btn pri" onClick={save}>Save options</button><span class="muted">Datum Schema keeps everything on this computer. Its only connection is the update check to GitHub.</span></p>
      <h2>This project</h2>
      <ProjectSettings />
    </div>
  );
};

export const Backstage = () => (
  <div class="bs">
    <aside>
      <button class="back" onClick={() => out() && setBackstage(null)} title="Back">←</button>
      <For each={[["home", "home", "Home"], ["new", "new", "New"], ["open", "open", "Open"], ["save", "save", "Save"], ["saveas", "save", "Save As"], ["export", "export", "Export"], ["print", "print", "Print"], ["roadmap", "list", "Roadmap"]]}>{([k, i, l]) =>
        <button classList={{ on: backstage() === k }} onClick={() => {
          if (k === "open") C.open(); else if (k === "save") C.save(); else if (k === "saveas") C.save(true);
          else setBackstage(k === "new" ? "home" : k);
        }}><Icon n={i} size={22} />{l}</button>}</For>
      <span class="grow" />
      <button classList={{ on: backstage() === "options" }} onClick={() => setBackstage("options")}><Icon n="opts" size={22} />Options</button>
      <button classList={{ on: backstage() === "about" }} onClick={() => setBackstage("about")}><Icon n="upd" size={22} />Updates</button>
    </aside>
    <section class="bsmain">
      <Show when={backstage() === "home"}><Home /></Show>
      <Show when={backstage() === "export"}><Export /></Show>
      <Show when={backstage() === "options"}><OptionsPage /></Show>
      <Show when={backstage() === "print"}><PrintPage /></Show>
      <Show when={backstage() === "roadmap"}><RoadmapPage /></Show>
      <Show when={backstage() === "about"}><div class="about"><h1>Updates</h1><UpdateCard /><p><button class="btn" onClick={() => D.shortcuts()}>Keyboard shortcuts</button></p></div></Show>
    </section>
  </div>
);
