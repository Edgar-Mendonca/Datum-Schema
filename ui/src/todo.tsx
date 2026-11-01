// The To-do tab: what to look at after a change (engine: propagate::todo): a component whose own
// size or pipe class differs from its line's, a datasheet or a sheet changed since it was issued.
import { For, Show, createResource } from "solid-js";
import { act, call, out } from "./store";
import { C } from "./cmds";
import { openDatasheet } from "./datasheet";
import { compareSheet } from "./revisions";

export interface Todo { kind: "override" | "datasheet" | "sheet"; what: string; ids: number[]; sheet: number; key: string; value: string; rev: string; obj: number }

/** The list, fetched again after every change. */
export const [todos, { refetch: refetchTodos }] = createResource(() => out(), () => call<Todo[]>("todo").then(v => v ?? []));

export const TodoPanel = () => (
  <div class="todolist">
    <For each={todos() ?? []} fallback={<p class="muted" style={{ padding: "8px" }}>Nothing to do: components follow their lines, and nothing issued has changed since.</p>}>{t => (
      <div class={`todo ${t.kind}`}>
        <span class="tk">{t.kind === "override" ? "Line value" : t.kind === "datasheet" ? "Datasheet" : "Sheet"}</span>
        <span class="tw">{t.what}</span>
        <Show when={t.kind === "override"}>
          <button class="btn sm" onClick={() => act({ SetData: { ids: t.ids, key: t.key, val: t.value } })}>Take the line's {t.key === "size" ? "size" : "class"}</button>
          <button class="btn sm" onClick={() => C.goTo(t.ids, t.sheet)}>Go to</button>
        </Show>
        <Show when={t.kind === "datasheet"}><button class="btn sm" onClick={() => openDatasheet(t.obj)}>Open the datasheet</button></Show>
        <Show when={t.kind === "sheet"}><button class="btn sm" onClick={() => compareSheet(t.sheet, t.rev)}>Compare with {t.rev}…</button></Show>
      </div>
    )}</For>
  </div>
);
