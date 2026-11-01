# AGENTS.md

Project-level operating instructions for Codex and other contributors working in this repository.

## Project Overview

`Pete's PDF to MD` is a desktop app plus a command-line converter that turns PDF documents into Markdown better suited for AI and LLM workflows. Since 1.0 both are written in Rust: the converter is a Rust library and command-line tool, and the app is a Tauri window around it.

The codebase focuses on:

- extracting a reliable heading outline from a PDF
- using that outline to split the PDF content into manageable Markdown files
- supporting both command-line and desktop GUI workflows
- producing output that is easy to inspect, review, and reuse

This project is not a generic PDF renderer. Its main job is to convert structured PDFs into high-quality Markdown organized by document headings.

## Main Technologies

- Rust (stable, via rustup) for the converter and the app
- [pdf-inspector](https://github.com/firecrawl/pdf-inspector) (MIT) for text, structure tags and tables; `lopdf` for bookmarks and page sizes
- Tauri 2 for the desktop app (uses the system web view; the window is plain HTML/CSS/JavaScript)
- Node.js 20+ for the Tauri CLI (`@tauri-apps/cli`), the test runners and small build scripts
- Quarto for the website in `docs/`

No Python is used. The Electron app and Python/PyMuPDF converter used up to v0.9 are in `archive/electron-python/` for reference only; do not build on them.

## Repository Layout

- `README.md`: end-user overview, install, build, troubleshooting
- `CHANGELOG.md`: release history
- `package.json`: npm scripts, app version and build revision letter
- `rust/converter/`: the converter crate (`petes-pdf-to-md`; library `petes_pdf_core`, binary `petes-pdf-to-md`). See `rust/README.md`.
- `rust/app/`: the Tauri app crate (`petes-pdf-to-md-app`), with `tauri.conf.json`, `capabilities/`, `icons/`
- `app/`: the app window (`index.html`, `styles.css`, `renderer.js`, `tauri-bridge.js`)
- `scripts/build-mac.sh`: macOS DMG build
- `scripts/bump-revision.js`: revision letters and the display version
- `scripts/release-prep.sh`: release-readiness checks and optional build
- `scripts/test-smoke.js`, `scripts/test-regression.js`: test runners
- `docs/`: Quarto website source (user guide). `quarto render docs` builds it into `docs/_site/` (ignored). `docs/_changelog.md` is regenerated from `CHANGELOG.md` by `docs/_copy-changelog.py` on every render; commit it with changelog changes.
- `tests/`: test plan and cases; sample PDFs in `tests/pdfs/` (not committed)
- `archive/electron-python/`: retired Electron app and Python converter (see its README)
- `.agents/skills/`: project-local skills

## How The App Works

There are two entry points, and both call the same library function, `petes_pdf_core::convert_with_progress`:

- CLI: `npm run convert -- --input "path/to/file.pdf"` (runs `rust/target/release/petes-pdf-to-md`)
- GUI: `npm run gui` (Tauri dev mode) or the built app

Conversion steps (`rust/converter/src/lib.rs`):

1. Read the PDF with pdf-inspector (whole-document Markdown, positioned text, structure tags) and lopdf (bookmarks, page sizes). Pages that are images with an invisible text layer are read through region extraction.
2. Build the outline. Heading sources, in order: PDF bookmarks; accessibility tags when there are no bookmarks or the bookmarks are flat (`PDF_TO_MD_NO_TAGS=1` turns this off), plus curriculum-code lines (`A.`, `A1.`) tagged as paragraphs but set in large bold type; text heuristics.
3. Fix the reading order (`reading.rs`), remove page furniture, split the Markdown at each heading, clean each section and repair run-on tables (`body.rs`, `tables.rs`).
4. Write the Markdown files and `outline.json`, `outline.md`, `segments.json` (`output.rs`).

The app then reads those files back from disk to show the outline and section content.

## Core Application Files

### App (Tauri)

- `rust/app/src/main.rs`: commands called by the window (pick/validate/drop a PDF, run a conversion, load the outline and sections, open files and folders), window title, file-drop forwarding, and mock-runtime tests
- `rust/app/build.rs`: display version from `package.json`, debug-build flag
- `app/tauri-bridge.js`: defines `window.pdfToMdApi` on top of the commands and events
- `app/renderer.js`, `app/index.html`, `app/styles.css`: the window's UI and state

The command names and return shapes began as the Electron IPC handlers and `renderer.js` still uses that API (`window.pdfToMdApi`). Change a command, its bridge function and its use in `renderer.js` together.

### Converter

- `rust/converter/src/lib.rs`: `convert` / `convert_with_progress`, the pipeline
- `outline.rs`, `layout.rs`, `reading.rs`, `body.rs`, `tables.rs`, `output.rs`, `text.rs`: see the table in `rust/README.md`
- `rust/converter/src/main.rs`: the command-line tool

## Output Model

For an input PDF named `example.pdf`, output is written under:

- `<output-root>/example/`

Common generated files:

- `outline.json`: machine-readable outline and section metadata
- `outline.md`: human-readable outline summary
- `segments.json`: split planning / segment metadata

Depending on conversion mode:

- `single`: one merged Markdown file in the output root folder for that PDF
- `sections`: one Markdown file per heading in `Sections/`
- `major`: one Markdown file per major heading in `By Major Heading/`

In the app, the default output root is:

- `~/Documents/Pete's PDF to MD Output`

## Converter Notes

- `PDF_TO_MD_DEBUG_TABLES=1` prints the run-on table repair decisions to stderr; `PDF_TO_MD_DEBUG_READING=1` prints the reading-order decisions; `PDF_TO_MD_NO_REORDER=1` keeps pdf-inspector's own order (for comparisons); `PDF_TO_MD_NO_TAGS=1` ignores accessibility tags.
- Every cleanup or reordering step must keep all words; the table and reading-order code check this and fall back when it fails. Losing document text is the worst kind of regression here.
- The rules were ported from the Python converter; `archive/electron-python/scripts/extract_outline.py` can explain why a rule exists, but the Rust code is now the source of truth.

## App Notes

- File drops: `forward_file_drops` in `main.rs` sends `file-drag` events (`enter`, `leave`, `drop` with paths) to the window; a drop anywhere on the window selects the PDF. The drop position is not used, because macOS reports it in different units from the other platforms.
- Conversion progress goes to the window as `conversion-progress` events.
- `cargo test -p petes-pdf-to-md-app` calls the commands through Tauri's mock runtime the way the bridge does (same names and argument JSON), including a full conversion and a file drop.

## Versions, Debug Builds and Revision Letters

- `package.json`, `rust/*/Cargo.toml` and `rust/app/tauri.conf.json` hold the semver version (`1.0.0`); keep them equal. Users see the display version, which drops a zero patch number (`1.0`).
- Every build advances `buildRevision` in `package.json` (`a`, `b`, …) via `scripts/bump-revision.js`; the letter restarts when the version changes. Release builds don't show it; debug builds do (`1.0b`). `rust/app/build.rs` applies the same rule.
- `npm run build:mac:debug` makes a debug build: "(debug version)" in the title bar and output in `<output>/<PDF name>/v1.0b/`. `npm run gui:debug` does the same from source, and `PDF_TO_MD_DEBUG=1` turns on the per-version folders at run time.

## Development Workflows

### Install

- Rust via [rustup](https://rustup.rs), Node.js 20+
- `npm install` (installs the Tauri CLI)

### Run The GUI

- `npm run gui`

### Run The CLI

- `npm run convert -- --input "tests/pdfs/<file>.pdf" --out-dir output --conversion-mode single`

### Build

- macOS DMG: `npm run build:mac` → `dist/Pete-s-PDF-to-MD-v<display version>-macOS.dmg`
- `dist/` (not committed) holds only the current release DMG and its `RELEASE-NOTES-v<version>.md`; earlier release DMGs move to `dist/archive/`. Debug builds are deleted once superseded.
- Windows: not yet built or tested for the Tauri app.

### Release Prep

- `npm run release:prep -- --version <x.y.z>`
- `npm run release:prep -- --version <x.y.z> --build`

`scripts/release-prep.sh` checks:

- `package.json`, both `Cargo.toml` files and `tauri.conf.json` match the requested version
- `CHANGELOG.md` has a matching release section (`## [1.0] - …` or `## [1.0.0] - …`)
- `README.md` references the display version (`v1.0`)
- `README.md` includes the GitHub Releases URL

## Local Skills Convention

This repository keeps project skills under:

- `.agents/skills/<skill-name>/SKILL.md`

When a user request clearly matches a local skill in `.agents/skills/`, open and follow that skill.

## Project Skill

### Available skills

- `petes-pdf-release`: prepare and publish a macOS GitHub release, including version checks, changelog/README release updates, and DMG build flow. File: `.agents/skills/petes-pdf-release/SKILL.md`

Use this skill when the task is about release preparation, DMG packaging, or release publication.

## Execution Rules

1. Prefer deterministic scripts over ad-hoc shell commands.
2. Keep scripts idempotent where possible.
3. When adding a new workflow, expose it through `scripts/` first.
4. Prefer extending the existing CLI or packaging scripts over embedding one-off behavior inside documentation.
5. Preserve cross-platform behavior where practical. Releases are macOS-only for now, but the converter and the Tauri app are meant to build on Windows too; avoid macOS-only code outside clearly marked platform branches.

## Change/Test Gate

All code changes must pass an automated test gate before they are considered complete.

Canonical test inventory: `tests/TEST_PLAN.md`.
Executable smoke test list: `tests/cases/smoke.json`.
Latest smoke report: `tests/reports/smoke-latest.md`.
Executable regression test list: `tests/cases/regression.json`.
Latest regression report: `tests/reports/regression-latest.md`.

### Required process on every change

1. Identify impacted behavior (feature, bugfix, or regression risk).
2. Add or update at least one automated test that would fail without the change.
3. Run the fast smoke suite.
4. Run the full regression suite.
5. Only finalize when all required tests pass.

### Test sources and ownership

- Keep test inputs in `tests/pdfs/`.
- Keep generated test run outputs in `tests/runs/` (and `tests/runs-single/` for ad-hoc single-mode runs).
- Keep executable test runners in `scripts/` (deterministic, non-interactive).
- Keep package entry points in `package.json` scripts so tests are runnable by one command.

### Minimum standard for agents

- Do not treat a change as done if automated tests were not run, unless explicitly instructed.
- If tests cannot be run, report exactly why and what remains unverified.
- When behavior changes intentionally, update golden files/tests in the same change.
- Prefer adding a smoke test first for new behavior, then expand to full regression coverage.

### CI expectation

- CI should execute the same full regression command used locally.
- A pull request should be considered blocked if the regression suite fails.

## Code Change Guidance

- Keep the GUI and CLI aligned. Both call the same library; if you add an option, expose it in both.
- Treat `outline.json`, `outline.md`, and segmented Markdown files as part of the public workflow; avoid changing their names or layout casually.
- When changing output folder names or layout, review both the generator (`output.rs`) and the app code that reads outputs back in (`load_outline` / `load_section` in `rust/app/src/main.rs`).
- Maintain helpful failure messages. The GUI already translates several technical failures into clearer user-facing errors.

## Testing Guidance

Run `npm test` (builds the converter, then the smoke + regression suites; see "Change/Test Gate" above) and `npm run test:rust` (unit tests and app command tests). The suites use PDFs in `tests/pdfs/` (for example `Science SNC1W.pdf`), which are not committed; a regression case whose PDF is missing is skipped.

When changing extraction or output behavior, also verify with:

- `npm run convert -- --input "tests/pdfs/<file>.pdf"`
- inspection of generated `outline.json`, `outline.md`, and Markdown outputs
- a check that no document text was lost compared with the previous version (cleanup rules that drop paragraphs are the usual cause)

When changing GUI behavior, verify with:

- `npm run gui`
- `cargo test -p petes-pdf-to-md-app` when a command or the bridge changes
- one conversion run through the desktop app
- outline loading, section browsing, and output-folder actions when relevant

When changing packaging or release flows, verify the relevant script directly instead of documenting assumptions.

## Documentation Guidance

- Keep `README.md` focused on end users: install, run, build, troubleshoot, release artifact locations.
- Keep `AGENTS.md` focused on contributor and agent context: architecture, workflows, conventions, and guardrails.
- If behavior changes, update the code and the relevant docs in the same turn when feasible. User-facing changes also belong on the website pages in `docs/` (for example `using-the-app.qmd`, `troubleshooting.qmd`).
- Use exact script names, paths, and output folder names. Avoid vague documentation.

## Tutorial Writing Guidance

- Tutorials should be more descriptive than concise. Do not assume the reader already knows the terminology.
- When introducing a term, acronym, tool, or concept for the first time, briefly define it in plain language before using it without explanation.
- Prefer adding a short explanation, example, or why-it-matters sentence when a step depends on unfamiliar vocabulary or prior knowledge.
- Write for a beginner reader by default. If a section uses technical language, make the meaning clear in the surrounding text instead of relying on the reader to infer it.
- If instructions mention a script, package, artifact, or build term, explain what it is and what role it plays in the workflow.

## Contributor Expectations

- Read the existing scripts before changing process behavior.
- Keep naming consistent with the rest of the repo. This project already has established names for modes, outputs, and release artifacts.
- Avoid introducing large abstractions unless they solve a concrete problem in this small codebase.
- Favor straightforward, inspectable code over cleverness.
- If you discover a repo-specific behavior that future contributors need to know, add it here instead of relying on memory.
