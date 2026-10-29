// Display units. The engine and the file use SI base units (m, kg, s, Pa, K); the engine also sends
// how every quantity is shown in SI and US units (crates/core/src/units.rs, from Datum Calc's unit
// table), so this file holds no conversion factors of its own: it only applies them.
import { createSignal } from "solid-js";
import { proj } from "./store";
import { num, Shown } from "./api";

const [tables, setTables] = createSignal<[Shown[], Shown[]] | null>(null);
export const setUnitTable = (t: [Shown[], Shown[]]) => setTables(t);

const row = (q: string, us?: boolean): Shown => {
  const t = tables(); const sys = us ?? proj()?.units === "US";
  return t?.[sys ? 1 : 0].find(s => s.q === q) ?? { q, label: "", factor: 1, offset: 0 };
};

/** SI value → display value. */
export const toU = (q: string, v: number) => { const r = row(q); return v / r.factor - r.offset; };
/** Display value → SI value. */
export const fromU = (q: string, v: number) => { const r = row(q); return (v + r.offset) * r.factor; };
export const unit = (q: string, us?: boolean) => row(q, us).label;
/** A value with its unit, for labels. */
export const fmt = (q: string, v: number, sig = 4) => `${num(toU(q, v), sig)}${unit(q) ? " " + unit(q) : ""}`;
