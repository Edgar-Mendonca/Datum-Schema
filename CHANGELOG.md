# Changelog

## 0.1.0 — 2026-10-29

The first release: the window, the sheet, symbols and lines, and the updater.

**The window.** The family window in magenta: the ribbon (Home, Draw, Symbols, Lines, Instruments, Check, Lists, Sheets, View, Help and the Format tab for a selection), the File page, Options, the symbol palette on the left, the sheet in the middle, the Inspector on the right, and a panel below for the checks and the lists. Light, dark and automatic themes.

**Sheets.** A blank A3 sheet with its border, grid zones (A–H across, 1–8 down), title block and revision table. Sheets can be A3, A1, ANSI B or ANSI D; add sheets and a legend sheet from the tabs under the drawing. With nothing selected, the Inspector edits the sheet, its revisions and the title block.

**Symbols and lines.** About 80 symbols of our own: vessels, pumps and compressors, exchangers, manual, control and relief valves, fittings, instrument bubbles, flowmeters, line marks and off-page connectors. Drag one from the palette or double-click the sheet and type its name; it gets the next free tag. Drag from a port to another to draw a line; lines route themselves at right angles around symbols, carry a flow arrow on every segment and hop over each other. A valve dropped on a line goes into it, and both halves keep the line's data.

**Files.** Projects save as `.dschema` files and open again, with recovery of unsaved work. Every save is written safely.

**Updates.** Datum Schema checks for new versions when it starts and installs them from File › Updates, after checking the download against its checksum.
