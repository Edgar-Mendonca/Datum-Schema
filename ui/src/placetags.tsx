// Help tracing an old drawing (engine: convert.rs to_place): the tags in the drawing behind a
// sheet that the project lacks, each placed as the symbol its letters suggest, one or all at once.
import { For, Show, createSignal } from "solid-js";
import { act, call, openDialog, sheet as curSheet, toast } from "./store";
import { Dialog } from "./ui";

interface Place { tag: string; sym: string; name: string; x: number; y: number }

export const openTagsToPlace = async () => {
  const sheet = curSheet()?.id ?? 0;
  const first = await call<Place[] | null>("tags_to_place", { sheet });
  if (!first) return;
  openDialog(() => {
    const [rows, setRows] = createSignal<Place[]>(first);
    const refresh = async () => setRows((await call<Place[] | null>("tags_to_place", { sheet })) ?? []);
    const place = async (tags: string[]) => {
      const n = tags.length || rows().length;
      const o = await act({ PlaceTags: { sheet, tags } });
      if (o) { toast(`Placed ${n} tag${n === 1 ? "" : "s"} where the old drawing has them: move each onto its symbol and draw the lines.`); await refresh(); }
    };
    return (
      <Dialog title="Tags to place" wide help="underlay" ok={async () => { if (rows().length) await place([]); return true; }} okText="Place all">
        <Show when={rows().length} fallback={<p>Every tag in the old drawing behind this sheet is in the project{first.length ? " now" : " already, or the drawing is a scan whose text cannot be read"}.</p>}>
          <p><b>{rows().length}</b> tags in the old drawing behind this sheet are not in the project yet. Each is placed at its text (an instrument bubble's in its middle) as the symbol its letters suggest; move it onto the drawing's symbol, or swap the symbol, then draw the lines.</p>
          <div class="hzscroll"><table class="tbl small"><thead><tr><th>Tag</th><th>Becomes</th><th /></tr></thead><tbody>
            <For each={rows()}>{r => <tr><td class="mono">{r.tag}</td><td>{r.name}</td><td><button class="btn sm" onClick={() => place([r.tag])}>Place</button></td></tr>}</For>
          </tbody></table></div>
        </Show>
      </Dialog>
    );
  });
};
