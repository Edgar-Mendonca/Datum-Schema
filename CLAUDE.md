# Datum Schema: notes for Claude

Read `docs/MEMORY.md` first. It is the project's memory: architecture, decisions, roadmap, and a log of every session. The family design guide is `FAMILY.md` in `Edgar-Mendonca/Datum` (read-only from here; `docs/FAMILY.md` links to it). The brief this app started from is `briefs/DATUM-SCHEMA.md` in the same hub repo.
At the end of every session that changes anything, update `docs/MEMORY.md`:
- tick the roadmap
- add a session-log entry
- record any new decisions
- refresh the architecture section if files moved

User preferences:
- Rust and Tauri 2, small and efficient code, Windows first. The same look, ribbon, help, printing and updater as every Datum app (FAMILY §2–9); copy from Datum Plan (shell), Datum Sim (block canvas, wire routing), Datum Calc (units) and Datum Pipe (pipe sizes) rather than reinventing. Every copy becomes Schema's own code: never link another app.
- Ship in small numbered releases (0.1.0, 0.1.1, …). Every release appears in the in-app updater.
- Engine logic belongs in `crates/core` (pure Rust, unit-tested), not in the UI. The window never calculates: connectivity, checks, lists, hydraulics, routing, the sheet frame and DXF are the engine's.
- The UI is SolidJS + Vite in `ui/src/*.tsx` (built to `ui/dist`). State lives in `ui/src/store.ts`.
- The engine and the `.dschema` file use SI base units (m, kg, s, Pa, K). The engine sends display units (SI and US) from its unit table (`crates/core/src/units.rs`, copied from Datum Calc by way of Datum Pipe); `ui/src/units.ts` only applies them.
- Symbols are our own vector drawings following the conventions of ISO 10628-2 (equipment, valves, lines) and ISA-5.1 (instruments). Never copy a symbol library from other software.
- Public words (README, CHANGELOG, site.json, help, commits): never name, compare with, or say the app copies or replaces any commercial drawing, CAD or plant-design product (FAMILY §13). Standards may be named by number; file formats (DXF, CSV, Excel) may be named.
- Hydraulic results are a sizing aid checked by the engineer; say so in the README and help.
- Licences: MIT/BSD/Apache/MPL only. Record every dependency in `NOTICE`.

Quality bar (brief §4):
- Every check has a passing and a failing test; tags, split and merge, off-page walking, hydraulics against a hand calculation and Datum Calc's pipe pack, DXF parsed back; every example opens with 0 errors and "Find the faults" has exactly its planted faults.
- Hardening (FAMILY §11): random projects never panic, damaged files open as far as possible, hostile text is shown as text.
- Never cut the connectivity, the checks, the lists, the hydraulics, printing or DXF.

Checks before pushing:
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (includes the `hardening` tests)
- `npm run check:ui`, then `npm run test:ui` (dumps the engine fixture, builds the window, runs the Playwright checks; set `NODE_PATH=$(npm root -g)` if Playwright is installed globally)
- `cargo deny check` (licences, advisories, sources; CI runs it)

Releasing:
1. Bump the version in `src-tauri/Cargo.toml` and `crates/core/Cargo.toml`.
2. Add a CHANGELOG section (`## x.y.z — date`).
3. Keep `docs/site.json` true with each release (`about`, `steps`, `highlights`, `roadmap`).
4. Run `release.yml` from the Actions tab (workflow_dispatch; tag pushes are blocked from Claude sessions) or push the tag `vX.Y.Z`. The workflow creates the tag and publishes `DatumSchema-x.y.z-setup.exe` and `SHA256SUMS.txt`.
