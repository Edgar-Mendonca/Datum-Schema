// Instrument wiring (engine: wiring.rs): adding junction boxes, cables and I/O cards, from their
// lists. Devices name them in the Inspector › Wiring (junction box, terminals, cable, and the I/O
// channel as <card>/<channel>).
import { For, Show, createSignal } from "solid-js";
import { act, openDialog, setPanel, setPanelOpen, toast } from "./store";
import { Dialog } from "./ui";

type Kind = "JunctionBox" | "Cable" | "IOCard";
const WHAT: Record<Kind, string> = { JunctionBox: "a junction box", Cable: "a cable", IOCard: "an I/O card" };

export const addWiring = (kind: Kind) => openDialog(() => {
  const [tag, setTag] = createSignal(kind === "JunctionBox" ? "JB-101" : kind === "Cable" ? "C-101" : "AI-01");
  const [a, setA] = createSignal(kind === "JunctionBox" ? "24" : kind === "Cable" ? "2" : "16");
  const [b, setB] = createSignal(kind === "Cable" ? "2-pair 1.5 mm², overall screen, armoured" : kind === "IOCard" ? "AI" : "");
  const [c, setC] = createSignal("");
  const go = async () => {
    const n = parseFloat(a());
    const data: Record<string, string | number> = {};
    if (kind === "JunctionBox") { if (Number.isFinite(n)) data.terminals = n; if (c()) data.location = c(); }
    if (kind === "Cable") { if (Number.isFinite(n)) data.pairs = n; if (b()) data.cable_type = b(); }
    if (kind === "IOCard") { if (Number.isFinite(n)) data.channels = n; data.io_kind = b(); if (c()) data.system = c(); }
    const o = await act({ AddObject: { class: kind, tag: tag(), data } });
    if (!o) return false;
    toast(`Added ${tag().trim()}. Name it on the devices in the Inspector › Wiring${kind === "IOCard" ? ` (I/O channel ${tag().trim()}/1, /2…)` : ""}.`, true);
  };
  return (
    <Dialog title={`Add ${WHAT[kind]}`} okText="Add" ok={go} help="wiring">
      <div class="form">
        <label>Tag</label><div><input class="mono" value={tag()} onInput={e => setTag(e.currentTarget.value)} /></div>
        <label>{kind === "JunctionBox" ? "Terminals" : kind === "Cable" ? "Pairs (or cores)" : "Channels"}</label><div><input class="mono" style={{ width: "80px" }} value={a()} onInput={e => setA(e.currentTarget.value)} /></div>
        <Show when={kind === "Cable"}><label>Type</label><div><input value={b()} onInput={e => setB(e.currentTarget.value)} /></div></Show>
        <Show when={kind === "IOCard"}><label>Kind</label><div><select value={b()} onChange={e => setB(e.currentTarget.value)}><For each={["AI", "AO", "DI", "DO", "Fieldbus"]}>{k => <option value={k}>{k}</option>}</For></select></div>
          <label>Control system</label><div><input placeholder="DCS cabinet 1" value={c()} onInput={e => setC(e.currentTarget.value)} /></div></Show>
        <Show when={kind === "JunctionBox"}><label>Location</label><div><input placeholder="Pump area" value={c()} onInput={e => setC(e.currentTarget.value)} /></div></Show>
      </div>
    </Dialog>
  );
});

export const WIRING_TABS: [string, string][] = [["jbs", "Junction boxes"], ["cables", "Cables"], ["ioalloc", "I/O allocation"]];
export const ADD: Record<string, Kind> = { jbs: "JunctionBox", cables: "Cable", ioalloc: "IOCard" };
export const openWiring = (k: string) => { setPanel(k); setPanelOpen(true); };
