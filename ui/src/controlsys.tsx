// Virtual instruments and the control-system export (engine: controlsys.rs). A virtual instrument
// is in the loops, lists, I/O and datasheets but not drawn on the sheet.
import { For, createSignal } from "solid-js";
import { act, call, openDialog, sheet, setSel, setPanel, setPanelOpen, toast } from "./store";
import { Dialog } from "./ui";

const KINDS: [string, string][] = [["inst_dcs", "Function in the control system (DCS)"], ["inst_plc", "Function in a PLC"], ["inst_interlock", "Interlock or trip"], ["inst_panel", "Panel instrument"], ["inst_field", "Field instrument"]];

export const addVirtual = () => openDialog(() => {
  const [tag, setTag] = createSignal(""), [kind, setKind] = createSignal("inst_dcs"), [fn, setFn] = createSignal(""), [sp, setSp] = createSignal("");
  const ok = async () => {
    const s = sheet(); if (!s) return false;
    if (!tag().trim()) { toast("Give it a tag (FY-101, LAHH-101…)."); return false; }
    const o = await act({ AddItem: { item: { sheet: s.id, sym: kind(), x: 20, y: 20, virt: true }, on: null, tag: tag().trim() } });
    if (!o) return false;
    const id = o.created[0];
    const obj = o.p.items.find(i => i.id === id)?.obj;
    if (obj) {
      for (const [k, v] of [["function", fn()], ["setpoints", sp()], ["system", kind() === "inst_dcs" ? "DCS" : kind() === "inst_plc" ? "PLC" : ""]] as [string, string][]) if (v.trim()) await act({ SetData: { ids: [obj], key: k, val: v.trim() } }, true);
    }
    setSel([id]);
    toast(`${tag().trim()} is a virtual instrument: in its loop, the lists, the I/O and its datasheet, not drawn. Draw it later from the Inspector.`, true);
    return true;
  };
  return (
    <Dialog title="Virtual instrument" ok={ok} okText="Add" help="virtual">
      <div class="form">
        <label>Tag</label><input class="vtag" value={tag()} placeholder="FY-101" onInput={e => setTag(e.currentTarget.value)} />
        <label>Kind</label><select value={kind()} onChange={e => setKind(e.currentTarget.value)}><For each={KINDS}>{([k, l]) => <option value={k}>{l}</option>}</For></select>
        <label>Function</label><input value={fn()} placeholder="Flow ratio calculation" onInput={e => setFn(e.currentTarget.value)} />
        <label>Set points</label><input value={sp()} placeholder="SP 50 %, LAH 80 %, LAHH 90 %" onInput={e => setSp(e.currentTarget.value)} />
      </div>
      <p class="muted small">It joins the loop its tag's number names (FY-101 in loop 101), and shows in the instrument index (marked virtual), the loop diagram, the I/O list if it takes I/O, the alarms list and its datasheet. It is not drawn, printed or written to DXF or DEXPI until you draw it.</p>
    </Dialog>
  );
});

/** Saves the control-system workbook: I/O list, tag list, alarms and trips, I/O summary. */
export const exportControlSystem = async () => {
  const f = await call<string | null>("export", { kind: "controlsys", what: "", sheet: 0 });
  if (f) toast(`Saved ${f}: the I/O list, instrument tag list, alarms and trips, and I/O summary for the control-system team.`, true);
};

export const showList = (k: string) => { setPanel(k); setPanelOpen(true); };
