# Contributing

Ideas and bug reports are welcome as [issues](https://github.com/Edgar-Mendonca/Datum-Schema/issues/new). File › Roadmap in the app (and `docs/site.json`) shows what is planned.

## How the code is laid out

- `crates/core`: the engine in pure Rust: the project model and its edits, the symbol library, connectivity, tags, checks, line hydraulics, the lists, the sheet frame, routing and the DXF writer. All calculation lives here, with tests.
- `src-tauri`: the desktop shell (commands, files, options, recovery, the updater).
- `ui/src`: the window (SolidJS + TypeScript). It draws what the engine computes and sends edits back as operations.
- `docs/MEMORY.md`: architecture, decisions, roadmap and a log of every session. Read it first.

## Before you send a change

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace            # engine tests, hardening
npm run check:ui                  # type-check the window
npm run test:ui                   # UI tests (Playwright, against the real engine)
cargo deny check                  # licences, advisories, sources (cargo install cargo-deny)
```

Keep code compact and in the style around it. A new check or calculation comes with a test (one passing and one failing case for a check, a hand-worked answer for a calculation); a new window feature with a UI test where it can. Symbols are our own drawings: never copy a symbol library. Never link GPL, LGPL or AGPL code into the app.

By contributing you agree your work is released under the MIT licence.
