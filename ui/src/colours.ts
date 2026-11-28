// How the drawing is coloured (0.11.3): in colour (line types and kinds of symbol told apart), in
// greys, or black and white, chosen separately for the screen and for print. What the drawing is
// doing as you work (the line being drawn, the port it will join, the selection, open ends and
// problems) keeps its own colours on the screen whatever is chosen, and never goes to print.

export type ColourMode = "colour" | "grey" | "bw";
export const MODES: [ColourMode, string][] = [["colour", "Colour"], ["grey", "Grayscale"], ["bw", "Black and white"]];

/** Line types: process lines dark blue, utilities green, signals violet, pneumatic teal, data orange. */
const LINE: Record<string, [string, string]> = {
  major: ["#1F3A8A", "#111111"], minor: ["#3352A8", "#3A3A3A"], utility: ["#17804A", "#4A4A4A"],
  pneumatic: ["#0E7C86", "#5A5A5A"], electric: ["#7E2FC4", "#5A5A5A"], data: ["#C2410C", "#666666"], capillary: ["#7E2FC4", "#666666"],
};
/** Kinds of symbol: equipment brown, valves red, fittings teal, instruments violet. */
const CAT: Record<string, [string, string]> = {
  vessel: ["#7C4A12", "#222222"], rotating: ["#7C4A12", "#222222"], heat: ["#7C4A12", "#222222"],
  valve: ["#B42318", "#333333"], control: ["#B42318", "#333333"], fitting: ["#0F766E", "#444444"],
  instrument: ["#6D28D9", "#3A3A3A"], primary: ["#6D28D9", "#3A3A3A"], mark: ["#333333", "#444444"], connector: ["#333333", "#444444"],
};

/** A line type's colour in a mode ("" for the drawing's own ink). */
export const lineColour = (m: ColourMode, kind: string) => (m === "bw" ? "" : (LINE[kind]?.[m === "colour" ? 0 : 1] ?? ""));
/** A kind of symbol's colour in a mode ("" for the drawing's own ink). */
export const catColour = (m: ColourMode, cat: string) => (m === "bw" ? "" : (CAT[cat]?.[m === "colour" ? 0 : 1] ?? ""));

/** CSS for the screen: each line type and kind of symbol gets its colour as a custom property, so
 *  the selection, hover and problem colours (set on the same elements) still win. */
export const screenCss = (m: ColourMode) => [
  ...Object.keys(LINE).map(k => { const c = lineColour(m, k); return c ? `.dsvg .ln.${k}{--lc:${c}}` : ""; }),
  ...Object.keys(CAT).map(k => { const c = catColour(m, k); return c ? `.dsvg .it.${k}{--ic:${c}}` : ""; }),
].join("");
