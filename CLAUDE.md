# CLAUDE.md — greenread

## Project overview
greenread is a desktop reader application: [what it reads — e-books (EPUB/FB2), PDFs, plain text, articles?].
Core idea: [one sentence, e.g. "distraction-free reading with saved progress"].

## Language rules
- Write all code, comments, commit messages, documentation, and responses in English.
- Think and plan in English as well.
- If the user writes in another language, reply in English unless they explicitly ask otherwise.

## Working style: break work into stages
- For any task that touches more than one file or takes more than a few steps, split it into stages before writing code.
- Each stage should be small, testable, and finished before starting the next one.
- Stage format:
  1. Goal of the stage (one sentence)
  2. Files to change
  3. Acceptance check (test to run or behavior to verify)
- Do not start the next stage until the current one passes its check.
- Report progress after each stage: what changed, what passed, what is next.

## Tech stack
- Language: Rust (stable toolchain, edition 2021 or later)
- UI: GTK4 via `gtk4-rs` (`gtk4`, `glib`, `gio`); use `libadwaita` (`libadwaita-rs`) for GNOME-style widgets if needed
- Build: Cargo; system GTK4 development libraries required
- C: only for FFI wrappers around C libraries that have no suitable Rust crate. Keep C code in `[native/]`, with a safe Rust wrapper in `[src/ffi/]`. Do not write new C for features Rust can handle.
- Format parsers: [crates such as `epub`, `lopdf` or `pdfium-render`, `quick-xml` — confirm before adding]
- Storage: [SQLite via `rusqlite` / JSON via `serde_json` in `$XDG_CONFIG_HOME/greenread`]

## Commands
- Build: `cargo build`
- Run: `cargo run`
- Test: `cargo test`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format: `cargo fmt`

## Project structure
- `src/main.rs` — application entry point and GTK application setup
- `src/ui/` — GTK widgets and windows; no parsing or storage logic here
- `src/parsers/` — one module per book format, each exposing the same trait
- `src/storage/` — reading progress, bookmarks, library index
- `src/ffi/` — safe wrappers around C code (only if present)
- `native/` — C sources (only if present)
- `tests/` — integration tests; sample books live in `tests/fixtures/`

## Rules
- Never modify the user's book files. greenread opens them read-only.
- UI code must not block the GTK main thread. Parsing and file I/O run on background threads or async tasks; results return to the UI via `glib::MainContext`.
- Each new format gets its own module in `src/parsers/` that implements the shared parser trait, plus a test using a fixture.
- Avoid `unsafe` outside `src/ffi/`. Any `unsafe` block needs a comment explaining why it is sound.
- Do not add crates without a clear reason; check the crate's maintenance status and license first.
- Reading progress (position, bookmarks) is stored in `[path / table]`. Schema changes require a migration.
- Run `cargo fmt`, `cargo clippy`, and `cargo test` before every commit.
- Commit every code change. Once a change passes those checks, commit it right away. Do not leave modified code uncommitted or start the next stage or task on a dirty working tree. Stage work uses `stage N: short description`; other changes use conventional types (`fix: ...`, `docs: ...`, `refactor: ...`).
- Push after every commit: `git push origin main`, so GitHub always matches the local branch.

## Current focus
- [Current task or milestone, e.g. "EPUB parser, basic window with chapter list"]
- Known issues: [e.g. "large PDFs block the UI"]