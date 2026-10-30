// Issuing a sheet at a revision, and comparing it with an issued revision (engine: revs.rs).
import { For, Show, createResource, createSignal } from "solid-js";
import { Compare, Sheet } from "./api";
import { act, call, openDialog, closeDialog, proj, setSel, setSheetId, toast } from "./store";
import { Dialog } from "./ui";

/** The revision after another: A → B, Z → AA, 0 → 1, P1 → P2 (as the engine numbers them). */
const nextRev = (r: string) => {
  const m = r.match(/^(.*?)(\d+)$/);
  if (m) return `${m[1]}${+m[2] + 1}`;
  if (!r) return "A";
  const cs = r.split("");
  for (let i = cs.length - 1; i >= 0; i--) {
    if (cs[i] === "Z") { cs[i] = "A"; continue; }
    if (/[A-Ya-y]/.test(cs[i])) { cs[i] = String.fromCharCode(cs[i].charCodeAt(0) + 1); return cs.join(""); }
    return `${r}1`;
  }
  return `A${cs.join("")}`;
};

/** Issue a sheet: a row in its revision table, and a copy of the sheet kept by the engine. */
export const issueRevision = (s: Sheet) => openDialog(() => {
  const last = s.revs[s.revs.length - 1]?.rev ?? "";
  const issued = s.snaps.some(x => x.rev === s.rev);
  const [rev, setRev] = createSignal(s.revs.length === 0 ? s.rev || "A" : issued || last === s.rev ? nextRev(last || s.rev) : s.rev);
  const [date, setDate] = createSignal(new Date().toLocaleDateString("en-GB", { day: "2-digit", month: "short", year: "2-digit" }).replace(/ /g, "-"));
  const [desc, setDesc] = createSignal(s.revs.length ? "Revised" : "Issued for review");
  const [by, setBy] = createSignal(proj()?.title.by ?? "");
  const go = async () => {
    const o = await act({ IssueRevision: { sheet: s.id, rev: { rev: rev(), date: date(), desc: desc(), by: by(), checked: "", approved: "" } } });
    if (o) toast(`${s.number} issued at revision ${rev()}. Changes from now on can be compared with it and clouded.`, true);
  };
  return (
    <Dialog title={`Issue ${s.number}`} ok={go} okText="Issue" help="revisions">
      <div class="form">
        <label>Revision</label><div><input class="mono" value={rev()} onInput={e => setRev(e.currentTarget.value)} /></div>
        <label>Date</label><div><input value={date()} onInput={e => setDate(e.currentTarget.value)} /></div>
        <label>Description</label><div><input value={desc()} onInput={e => setDesc(e.currentTarget.value)} /></div>
        <label>By</label><div><input value={by()} onInput={e => setBy(e.currentTarget.value)} /></div>
      </div>
      <p class="muted small">The sheet as it is now is kept with the revision, in the project file. Later, compare with it, or cloud the changes for the next issue.</p>
    </Dialog>
  );
});

/** What has changed since an issued revision; click a change to go to it. */
export const compareSheet = (sheet: number, rev: string) => openDialog(() => {
  const [c] = createResource(() => call<Compare>("sheet_compare", { sheet, rev }));
  const kinds: Record<string, string> = { added: "Added", removed: "Removed", changed: "Changed" };
  const go = (ids: number[]) => { setSheetId(sheet); if (ids.length) setSel(ids); closeDialog(); };
  const cloud = () => { const s = proj()?.sheets.find(x => x.id === sheet); if (s) act({ SetSheet: { sheet: { ...s, clouds: rev } } }); closeDialog(); };
  return (
    <Dialog title={`Changes since revision ${rev}`} help="revisions" extra={<button class="btn" onClick={cloud}>Cloud them</button>}>
      <Show when={c()} fallback={<p class="muted">Comparing…</p>}>{cc => <>
        <p class="muted small">{cc().changes.length ? `${cc().changes.length} changes, in ${cc().areas.length} clouded areas, for revision ${cc().next}.` : "Nothing has changed since this revision was issued."}</p>
        <Show when={cc().changes.length}>
          <div class="cmpscroll"><table class="tbl cmptbl"><tbody>
            <For each={cc().changes}>{x => <tr class={x.kind} onClick={() => go(x.ids)} title={x.ids.length ? "Go to it" : "No longer on the sheet"}><td class="k">{kinds[x.kind]}</td><td>{x.what}</td></tr>}</For>
          </tbody></table></div>
        </Show>
      </>}</Show>
    </Dialog>
  );
});

