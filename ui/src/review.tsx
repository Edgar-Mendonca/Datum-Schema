// Review comments (engine: review.rs): pins on the sheet, the Comments panel (reply, resolve,
// reopen, edit, delete, go to), and taking in a reviewer's copy.
import { For, Show, createMemo, createSignal } from "solid-js";
import { Comment, Out, Pt } from "./api";
import { act, call, openDialog, proj, sel, setSel, setSheetId, sheet, setOut, toast, setPanel, setPanelOpen } from "./store";
import { Dialog } from "./ui";

/** The comment the panel shows open (a pin was clicked). */
export const [focus, setFocus] = createSignal<string | null>(null);

const when = (t: number) => (t ? new Date(t * 1000).toLocaleString(undefined, { day: "2-digit", month: "short", year: "2-digit", hour: "2-digit", minute: "2-digit" }) : "");

/** A new comment at a point on the sheet, about whatever is selected. */
export const newComment = (at: Pt) => openDialog(() => {
  const [t, setT] = createSignal("");
  const on = sel();
  return (
    <Dialog title="Review comment" ok={async () => { const s = sheet(); if (!s) return; const o = await act({ Comment: { sheet: s.id, x: at[0], y: at[1], text: t(), on } }); if (!o) return false; setPanel("comments"); setPanelOpen(true); }} okText="Add comment" help="review">
      <textarea class="cmtext" rows="4" placeholder="What should be checked or changed?" value={t()} onInput={e => setT(e.currentTarget.value)} />
      <p class="muted small">{on.length ? `About the ${on.length} selected thing${on.length > 1 ? "s" : ""}; ` : ""}signed with your name from File › Options.</p>
    </Dialog>
  );
});

/** A reviewer's copy of the project: its new comments, replies and resolutions come in. */
export const importComments = async () => {
  const r = await call<[Out, { comments: number; replies: number; statuses: number }] | null>("comments_import");
  if (!r) return;
  setOut(r[0]);
  const m = r[1];
  toast(m.comments || m.replies || m.statuses ? `Took in ${m.comments} new comment${m.comments === 1 ? "" : "s"}, ${m.replies} repl${m.replies === 1 ? "y" : "ies"} and ${m.statuses} change${m.statuses === 1 ? "" : "s"} of status.` : "Nothing new in that copy.", true);
};

const Card = (p: { c: Comment; n: number }) => {
  const [reply, setReply] = createSignal("");
  const [editing, setEditing] = createSignal(false);
  const go = () => { setSheetId(p.c.sheet); setSel(p.c.on); setFocus(p.c.uid); };
  const sheetNo = () => proj()?.sheets.find(s => s.id === p.c.sheet)?.number ?? "";
  return (
    <div class="cmcard" classList={{ resolved: p.c.resolved, on: focus() === p.c.uid }} onClick={go}>
      <header><span class="cmn">{p.n}</span><b>{p.c.by || "Someone"}</b><span class="muted small">{when(p.c.time)} · {sheetNo()}</span><span class="grow" />
        <Show when={p.c.resolved} fallback={<button class="btn sm" onClick={e => { e.stopPropagation(); act({ Resolve: { uid: p.c.uid, resolved: true } }); }}>Resolve</button>}>
          <span class="pill ok">Resolved{p.c.resolved_by ? ` by ${p.c.resolved_by}` : ""}</span><button class="btn sm" onClick={e => { e.stopPropagation(); act({ Resolve: { uid: p.c.uid, resolved: false } }); }}>Reopen</button>
        </Show>
      </header>
      <Show when={editing()} fallback={<p class="cmbody" onDblClick={() => setEditing(true)}>{p.c.text}</p>}>
        <textarea class="cmtext" rows="3" value={p.c.text} onClick={e => e.stopPropagation()} onChange={e => { act({ EditComment: { uid: p.c.uid, text: e.currentTarget.value } }); setEditing(false); }} />
      </Show>
      <For each={p.c.replies}>{r => <div class="cmreply"><b>{r.by || "Someone"}</b> <span class="muted small">{when(r.time)}</span><p>{r.text}</p></div>}</For>
      <div class="cmrow" onClick={e => e.stopPropagation()}>
        <input placeholder="Reply…" value={reply()} onInput={e => setReply(e.currentTarget.value)} onKeyDown={e => { if (e.key === "Enter" && reply().trim()) { act({ Reply: { uid: p.c.uid, text: reply() } }).then(o => o && setReply("")); } }} />
        <button class="btn sm" disabled={!reply().trim()} onClick={() => act({ Reply: { uid: p.c.uid, text: reply() } }).then(o => o && setReply(""))}>Reply</button>
        <button class="btn sm" onClick={() => setEditing(true)}>Edit</button>
        <button class="x" title="Delete this comment" onClick={() => act({ DeleteComment: { uid: p.c.uid } })}>×</button>
      </div>
    </div>
  );
};

/** The Comments tab under the drawing. */
export const CommentsPanel = () => {
  const [show, setShow] = createSignal<"open" | "all">("open");
  const [here, setHere] = createSignal(false);
  const all = createMemo(() => (proj()?.comments ?? []).map((c, i) => ({ c, n: i + 1 })));
  const list = createMemo(() => all().filter(x => (show() === "all" || !x.c.resolved) && (!here() || x.c.sheet === sheet()?.id)));
  const open = () => all().filter(x => !x.c.resolved).length;
  return (
    <div class="cmpanel">
      <div class="cfilter">
        <label class="chk"><input type="radio" checked={show() === "open"} onChange={() => setShow("open")} />Open ({open()})</label>
        <label class="chk"><input type="radio" checked={show() === "all"} onChange={() => setShow("all")} />All ({all().length})</label>
        <label class="chk"><input type="checkbox" checked={here()} onChange={e => setHere(e.currentTarget.checked)} />This sheet only</label>
        <span class="grow" />
        <button class="btn sm" onClick={importComments} title="Take in the comments, replies and resolutions of a reviewer's copy of this project">Import a reviewer's copy…</button>
      </div>
      <div class="cmlist">
        <For each={list()} fallback={<p class="muted" style={{ padding: "8px" }}>{all().length ? "No open comments: everything is resolved." : "No comments yet: Check › Comment, then click the sheet where it applies."}</p>}>{x => <Card c={x.c} n={x.n} />}</For>
      </div>
    </div>
  );
};
