// Find anything (engine: query.rs): a plain query whose results open as a list in the bottom panel,
// so they can be clicked, printed and exported like any list.
import { For, Show, createSignal } from "solid-js";
import { act, closeDialog, openDialog, setPanel, setPanelOpen, toast } from "./store";
import { Dialog } from "./ui";
import { Issue } from "./api";

const EXAMPLES = [
  "control valves without a bypass",
  "pumps without a check valve",
  "lines with velocity above 3 m/s",
  "instruments without a range",
  "equipment with design pressure > 10 bar",
  "valves normally closed",
  "equipment with errors",
  "FT-1*",
];

export const [lastFind, setLastFind] = createSignal("");

/** Runs a query: its results open as a list. */
export const runFind = (q: string) => {
  const t = q.trim();
  if (!t) return toast("Type what to find.");
  setLastFind(t);
  setPanel(`find:${t}`);
  setPanelOpen(true);
};

export const openFind = () => openDialog(() => {
  const [q, setQ] = createSignal(lastFind());
  const go = () => { if (!q().trim()) { toast("Type what to find."); return false; } runFind(q()); return true; };
  return (
    <Dialog title="Find" ok={go} okText="Find" help="find">
      <input class="findq" value={q()} placeholder="control valves without a bypass" onInput={e => setQ(e.currentTarget.value)} />
      <p class="muted small">Name what to look at (pumps, control valves, gate valves, lines, instruments, a tag like FT-1*), then conditions joined by "and": missing data (without a range), comparisons with units (velocity above 3 m/s), the drawing (without a bypass, without isolation, without a check valve, not drawn, on PID-100, with errors, failing rule 8, fail open, normally closed), or "quoted text". What was understood heads the list.</p>
      <div class="findex"><For each={EXAMPLES}>{x => <button class="btn sm" onClick={() => { setQ(x); runFind(x); closeDialog(); }}>{x}</button>}</For></div>
    </Dialog>
  );
});

/** Accept a problem with a reason (kept for audit), or open it again. */
export const acceptProblem = (i: Issue) => openDialog(() => {
  const [why, setWhy] = createSignal("");
  const ok = async () => {
    if (why().trim().length < 3) { toast("Say why it is accepted (a few words at least)."); return false; }
    const o = await act({ Accept: { rule: i.rule, ids: i.ids, msg: i.msg, reason: why(), by: "", date: new Date().toISOString().slice(0, 10) } });
    if (o) toast("Accepted with your reason. It stays in the Accepted list; if the drawing changes what the check says, it is open again.", true);
    return !!o;
  };
  return (
    <Dialog title="Accept this problem" ok={ok} okText="Accept" help="accepted">
      <p><b>Rule {i.rule}:</b> {i.msg}</p>
      <textarea class="acceptwhy" rows={3} placeholder="Why it is acceptable here (for the record)" value={why()} onInput={e => setWhy(e.currentTarget.value)} />
      <p class="muted small">Your name (Options) and today's date are kept with the reason. The problem is listed as accepted, no longer counted, and comes back if the drawing changes what the check says.</p>
    </Dialog>
  );
});

export const unacceptProblem = (i: Issue) => act({ Unaccept: { rule: i.rule, ids: i.ids, msg: i.msg } }).then(o => o && toast("Open again.", true));

export const AcceptedNote = (p: { i: Issue; reason?: string }) => <Show when={p.reason}><div class="muted small">Accepted: {p.reason}</div></Show>;
