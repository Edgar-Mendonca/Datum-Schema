// Help › User guide: every feature in chapters, built from the same help topics as the "?" cards
// (so the guide and the cards never disagree), with the check rules, the shortcuts and the
// glossary. Search it, jump to a chapter, or print it on A4.
import { For, Show, createMemo, createSignal } from "solid-js";
import { Portal } from "solid-js/web";
import { HELP, RULES } from "./help";
import { GLOSSARY, GettingStarted, SHORTCUTS } from "./dialogs";
import { call } from "./store";

type Chapter = { id: string; t: string; intro: string; keys: string[] };

/** The chapters and the help topics in each; topics not named here go into "More". */
const CHAPTERS: Chapter[] = [
  { id: "drawing", t: "Drawing the plant", intro: "Place symbols, draw lines that route themselves, carry lines between sheets, and mark up the drawing.", keys: ["palette", "draw", "edit", "select", "lines", "signals", "offpage", "notes", "tags", "isa", "sheets", "view", "layers"] },
  { id: "data", t: "Data and objects", intro: "Every symbol and line is a view of a plant object with its data, in a plant › unit › area hierarchy, with every change recorded.", keys: ["inspector", "plant", "history", "units"] },
  { id: "checks", t: "Checks and hydraulics", intro: "The drawing is checked on every change; every line with a flow is sized. Hydraulic results are a sizing aid, checked by the engineer.", keys: ["checks", "rules", "hydraulics"] },
  { id: "lists", t: "Lists and reports", intro: "Lists written from the drawing, always in step with it: edit them, export them to Excel or CSV, print them.", keys: ["lists", "io", "takeoff", "export"] },
  { id: "instruments", t: "Instruments", intro: "Datasheets with their process conditions from the P&ID, loop diagrams, and the cause-and-effect matrix.", keys: ["datasheets", "import", "loops", "cause"] },
  { id: "pfd", t: "Process flow diagrams", intro: "Streams with their flows, conditions and compositions, a stream table on the sheet, and heat, mass and component balances.", keys: ["pfd"] },
  { id: "standards", t: "Symbols, standards and title blocks", intro: "A client's own symbols, old drawings to trace over, and your company's title block.", keys: ["libraries", "underlay", "titleblock"] },
  { id: "review", t: "Revisions and review", intro: "Issue sheets at revisions, compare and cloud what changed, and review with comments.", keys: ["revisions", "review"] },
  { id: "family", t: "Working with other programs", intro: "Send lines and loops to the other Datum apps; exchange drawings as DXF and DEXPI.", keys: ["send"] },
  { id: "examples", t: "Examples", intro: "Worked projects to learn from and to show.", keys: ["examples"] },
];

const placed = new Set(CHAPTERS.flatMap(c => c.keys));
const MORE: Chapter = { id: "more", t: "More", intro: "Symbol categories and other tools.", keys: Object.keys(HELP).filter(k => !placed.has(k)) };

/** Whether a topic matches the search. */
const hit = (k: string, q: string) => {
  if (!q) return true;
  const h = HELP[k];
  return !!h && [h.t, h.what, ...(h.how ?? []), h.tip ?? ""].join(" ").toLowerCase().includes(q);
};

const Topic = (p: { k: string }) => {
  const h = HELP[p.k];
  return (
    <Show when={h}>
      <section class="gtopic" id={`g-${p.k}`}>
        <h3>{h.t}</h3>
        <p>{h.what}</p>
        <Show when={h.how?.length}><ol><For each={h.how}>{x => <li>{x}</li>}</For></ol></Show>
        <Show when={h.tip}><p class="gtip">{h.tip}</p></Show>
      </section>
    </Show>
  );
};

/** The whole guide, for the page and for print. */
const Body = (p: { q: string }) => {
  const chapters = createMemo(() => [...CHAPTERS, MORE].map(c => ({ ...c, keys: c.keys.filter(k => HELP[k] && hit(k, p.q)) })).filter(c => c.keys.length));
  const rules = createMemo(() => RULES.filter(r => !p.q || r[1].toLowerCase().includes(p.q)));
  return (
    <div class="gbody">
      <Show when={!p.q}>
        <section class="gchap" id="g-start"><h2>Getting started</h2><GettingStarted /></section>
      </Show>
      <For each={chapters()}>{(c, i) => (
        <section class="gchap" id={`g-ch-${c.id}`}>
          <h2>{i() + 1}. {c.t}</h2>
          <p class="gintro">{c.intro}</p>
          <For each={c.keys}>{k => <Topic k={k} />}</For>
          <Show when={c.id === "checks" && rules().length}>
            <h3>The check rules</h3>
            <table class="tbl grules"><tbody><For each={rules()}>{([n, t, l]) => <tr><td class="mono">{n}</td><td>{t}</td><td class={`lvl ${l}`}>{l}</td></tr>}</For></tbody></table>
          </Show>
        </section>
      )}</For>
      <Show when={!p.q}>
        <section class="gchap" id="g-keys"><h2>Keyboard shortcuts</h2>
          <table class="tbl keys"><tbody><For each={SHORTCUTS}>{([a, b]) => <tr><td>{a}</td><td><kbd>{b}</kbd></td></tr>}</For></tbody></table>
        </section>
      </Show>
      <section class="gchap" id="g-gloss"><h2>Glossary</h2>
        <dl class="gloss"><For each={GLOSSARY.filter(g => !p.q || (g[0] + g[1]).toLowerCase().includes(p.q))}>{([t, d]) => <><dt>{t}</dt><dd>{d}</dd></>}</For></dl>
      </section>
      <p class="muted small gfoot">Hydraulic results, balances and the models sent to Datum Pipe and Datum Sim are a sizing aid and a starting point, checked by the engineer.</p>
    </div>
  );
};

/** File › User guide. */
export const GuidePage = () => {
  const [q, setQ] = createSignal("");
  const [printing, setPrinting] = createSignal(false);
  const query = () => q().trim().toLowerCase();
  const go = (id: string) => document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" });
  const print = () => {
    setPrinting(true);
    const css = document.createElement("style");
    css.textContent = "@page { size: A4 portrait; margin: 16mm 14mm }";
    document.head.append(css);
    document.documentElement.classList.add("printing");
    setTimeout(() => { window.print(); document.documentElement.classList.remove("printing"); css.remove(); setPrinting(false); }, 300);
  };
  const [ver, setVer] = createSignal("");
  call<string>("version").then(v => v && setVer(v));
  return (
    <div class="guide">
      <nav class="gtoc">
        <input class="gsearch" placeholder="Search the guide" value={q()} onInput={e => setQ(e.currentTarget.value)} />
        <Show when={!query()}>
          <a onClick={() => go("g-start")}>Getting started</a>
          <For each={[...CHAPTERS, MORE].filter(c => c.keys.some(k => HELP[k]))}>{(c, i) => <a onClick={() => go(`g-ch-${c.id}`)}>{i() + 1}. {c.t}</a>}</For>
          <a onClick={() => go("g-keys")}>Keyboard shortcuts</a>
          <a onClick={() => go("g-gloss")}>Glossary</a>
        </Show>
        <button class="btn pri" onClick={print}><svg class="ic"><use href="#i-print" /></svg> Print / Save as PDF</button>
      </nav>
      <div class="gmain">
        <h1>Datum Schema user guide <span class="muted small">{ver() ? `version ${ver()}` : ""}</span></h1>
        <Body q={query()} />
      </div>
      <Show when={printing()}><Portal mount={document.body}><div id="printroot" class="gprint"><h1>Datum Schema user guide</h1><p class="muted">Version {ver()}</p><Body q="" /></div></Portal></Show>
    </div>
  );
};
