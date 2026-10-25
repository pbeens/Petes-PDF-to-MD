# Changelog

All notable changes to this project are documented in this file.

## [0.8.0] - 2026-10-25

### Fixed

- Heading detection no longer turns bulleted list items into headings. The bullet filter's `•` character had been garbled by a file-encoding mix-up in v0.7.1, so it never matched. On `ICD2O_Curriculum.pdf` this cuts the outline from 281 entries to 128.
- `repair_mojibake` works again: its replacement table had been garbled the same way, and is now written with ASCII escapes.
- Removed a byte-order mark added to the top of `scripts/extract_outline.py` in v0.7.1.
- Table cells keep the spaces between words (previously `Knowledgeofcontent(eg facts . .,`). Cell text is now rebuilt from page words inside each cell box instead of `table.extract()`.
- 5-column tables (for example, the course list) no longer get an achievement-chart `Categories / Level 1–4` header unless their cells read like an achievement chart.
- Duplicate `Categories / Level 1–4` header rows inside achievement charts are removed again.
- Long or sentence-like numbered list items (`3. Students discuss their choices …`) are no longer detected as headings.
- Tables no longer carry empty columns left behind by merged cells, and a column split across two (header in one, values in the next) is folded back into one. On `Digital-Technology…pdf` the course table goes from 7 columns to 5 and the achievement charts from 13 to 5.
- Footnote numbers glued to bookmark titles are removed (`Avoiding the Dunning–Kruger Effect107` → `Avoiding the Dunning–Kruger Effect`). Numbering with a space, such as `Part 1`, is kept.
- `One file` and `By major heading` output no longer shows `(No extractable text in this range.)` under headings that are immediately followed by the next heading.
- Document text is no longer deleted by the v0.7.2 page-header cleanup. It removed every paragraph that began with a number and a capital letter, which included all numbered footnotes (`2 The word parent(s) is used …`), and any short paragraph that began with the section title. On `AI-in-Education.pdf` v0.7.2a lost about 66,000 characters (258 distinct words); now only short, header-like lines and exact title echoes are removed.
- `AGENTS.md` had leftover merge-conflict markers.
- Separate tagged headings stacked on one page are no longer joined into one (`Part 1 What Should Students Learn? The Impact of AI on Curriculum` is three headings again). Lines are joined only when the font size matches and the first line is unfinished.
- Page numbers printed next to the running header (top of the page) are removed. They used to be glued to the next page's first line (`7 academic piece…`), which then looked like a footnote and was moved to the end of the section. On `AI-in-Education.pdf` such glued paragraphs drop from 139 to 4.
- Footnotes are split by their small print, so consecutive footnotes are no longer run together (111 → 13 on `AI-in-Education.pdf`). Short footnotes such as `4 Dr Roger Schank, https://…` are no longer mistaken for page headers: the number-led header rule now applies only to the top of the page.
- Footnote markers after a closing quote (`clarity is power.”11`) are recognised, and a missing marker is no longer attached to a sentence cut off at a page break (`in-depth 10 academic piece`).
- Conversion is much faster with PyMuPDF 2.0: `AI-in-Education.pdf` (242 pages) drops from about 42 s to 7 s. When the optional `pymupdf-layout` package is installed, PyMuPDF 2.0's table finder runs an AI layout model on every page; the app now turns that off, so results also no longer depend on whether that package is present. Tables are also detected once per page instead of once per section.
- Table cells no longer repeat text when detected cells overlap (a merged cell over split ones); each word now goes to one cell only.

### Changed

- Works with PyMuPDF 2.0, which deprecates the old `fitz` module name: the app now imports `pymupdf` and falls back to `fitz` on older versions.
- README states plainly that the app uses the Python installed on the computer, including for the downloaded app.
- For a tagged PDF without bookmarks, single-file output now uses the tagged headings instead of the v0.7.2 one-heading fallback. The fallback still applies when there are neither bookmarks nor tags.
- When run from source, the converter uses the project's `.venv` Python automatically if it exists.

### Added

- Warning when the installed PyMuPDF is older than 1.23 and has no table detection. Previously, tables were silently written as plain text.
- Debug builds (`npm run build:mac:debug`): "(debug version)" in the title bar, and output written to a per-version folder (`<PDF name>/v0.8.0b/`) so earlier builds' results are kept. `npm run gui:debug` does the same from source; `--out-subdir` on the CLI.
- Regression tests `HEAD-001`, `PAGENUM-001` and `DEBUG-001`. A test whose PDF is missing on this machine is now skipped instead of failed.
- Build revision letters: each macOS build is named `0.8.0a`, `0.8.0b`, … in the DMG filename, window title and build stamp, so you can tell which build you are running.
- Conversion warnings now appear in a dialog when the conversion finishes. Before, they flashed by in the status line and were replaced within a second.
- Tagged-PDF headings: when a PDF has no bookmarks but is tagged for accessibility, the outline comes from its H1–H6 tags (requires the optional `pdf-inspector` package). On `ICD2O_Curriculum.pdf` this gives exactly the document's 101 headings at their real levels. Tags are also preferred over bookmarks when the bookmarks are flat and the tags have more levels (e.g. `AI-in-Education.pdf`: 172 single-level bookmarks become a three-level Part/chapter/section outline). Superscript footnote numbers are stripped from tagged heading titles.
- Regression tests: `TAGS-001` (tagged headings) and `FOOTNOTE-001` (footnotes kept). `SINGLE-001`/`SINGLE-002` now run with `PDF_TO_MD_NO_TAGS=1` so they keep testing the no-structure fallback, and `SINGLE-001` checks for the generated ToC only, not for a table rebuilt from the PDF's printed contents page.
- Experimental hybrid text engine, command line only (`--text-engine hybrid`). It takes body text from pdf-inspector (correct word spacing, bold/italic, multi-column order, about 10× faster) and applies this app's cleanup: ToC tables, margin-label and running-header removal, list spacing, and sentence re-joining. It falls back to PyMuPDF with a warning when `pdf-inspector` is not installed.

## [0.7.2a] - 2026-07-24

### Changed

- Release version baseline set to `v0.7.2a`.

### Fixed

- Prevented conversion crashes when section titles contain Unicode replacement characters that break progress logging on Windows consoles.

## [0.7.2] - 2026-03-21

### Changed

- Release version baseline set to `v0.7.2`.
- Single-file output no longer injects a generated top-level Table of Contents when the source PDF has no embedded TOC.
- For single-file conversion without embedded PDF TOC, heading fallback now emits one document-level heading instead of noisy heuristic heading splits.
- Margin-noise normalization improved for short PDFs so recurring running headers/footers are filtered more reliably.

### Fixed

- Columnar PDF extraction now preserves reading order for mixed text/table layouts by processing text and tables per column instead of interleaving by global `y` position.
- Text near table boundaries is no longer dropped by broad table-range skipping; only significant line/table overlap is filtered.
- Table rendering corruption from malformed row splitting was fixed so markdown tables remain valid and render correctly.
- Paragraph stitching no longer merges across column resets, reducing out-of-order content in two-column documents.

## [0.7.1] - 2026-03-20

### Added

- README quick-start download link to GitHub Releases.
- README GUI screenshot image for the app.

### Changed

- Release version baseline set to `v0.7.1`.
- Section hint text ("Right-click a section for options.") now auto-hides when it is not applicable (for example, single-file mode or only one section).
- TOC extraction now infers heading `y` anchors from page text when embedded TOC positions are missing.
- Section boundary handling improved to avoid overlapping content when consecutive headings have ambiguous same-page boundaries.
- Conversion now preserves content before the first detected heading as synthetic `Front Matter` when needed.
- Repeated page-margin noise (running headers/footers) and page-number footers are filtered from extracted content.
- Post-processing now better normalizes list/table output:
- Avoids converting table-leading row numbers into footnote superscripts.
- Converts "as follows:" strand lines into markdown bullets.
- Removes stray `<sup>n</sup>` prefixes that can break markdown table rows.

## [0.7.0] - 2026-03-01

### Added

- Conversion output mode options (GUI + pipeline):
- `One file` (single merged markdown output), `By major heading` (one file per top-level heading group), and `Individual sections` (one file per heading; prior default behavior).
- Section context menu action: `Copy path`.

### Changed

- Phase 1 extractor now supports output grouping via `--conversion-mode single|major|sections`.
- Output files are now organized per mode:
- `One file`: writes in the document output root (no subfolder)
- `Individual sections`: writes to `Sections/`
- `By major heading`: writes to `By Major Heading/`
- In `One file` mode, output markdown filename now matches the source PDF basename (for example, `Science SNC1W.md`).
- GUI conversion options now include output mode selection and persist locally between runs.
- README and release docs updated for `v0.7.0`.

## [0.6.0] - 2026-02-27

### Added

- Input PDF drag-and-drop support with validation for Windows drag payload variants.
- Section context menu action: `Open in folder`.
- Section context menu action: `Open in default program`.
- Section context menu action: `Copy path`.

### Fixed

- Windows packaged app no longer opens a console window during conversion subprocess execution by setting `windowsHide: true` on Node/Python child process launches.
- Conversion failures now show a clear invalid/corrupt PDF message when PyMuPDF reports format parsing errors (for example, "no objects found"), while still including technical details.
- Section content pane now resets scroll position to top when loading a different section.
- GUI error popups now suppress long traceback/progress blocks and display concise user-facing messages.
- Rendered markdown now correctly displays inline superscript tags such as `<sup>51</sup>`.
- Conversion reliability on Windows improved by avoiding full output tree deletion; locked file scenarios now return a clear "output files are locked" message.
- Drag/drop path normalization improved for file URIs and Windows path formats.

### Changed

- Release version baseline set to `v0.6.0`.
- Path display now uses rendered-width middle truncation that preserves the start of the path and full trailing filename.
- Build stamp now displays local time in `yyyy-mm-dd hh:mm` format.
- PDF section output now writes one merged markdown file per heading (no `-part-*` section files).
- Section viewer now combines multi-part section files when reading legacy outputs created before merged-file mode.
- Drag/drop target moved from a large panel to the `Select PDF` button to reduce occupied UI space.

## [0.5.0] - 2026-02-26

### Added

- Cross-platform Phase 1 conversion pipeline.
- `scripts/phase1.js` launcher (Node.js) and `scripts/extract_outline.py` extractor (Python + PyMuPDF).
- Output generation for `outline.json`, `outline.md`, `segments.json`, and section markdown files.
- Electron desktop GUI with PDF selection, output-folder selection, conversion run, outline preview, section browsing, and output-folder open action.
- Release workflow scripts:
- `scripts/build-mac.sh`
- `scripts/release-prep.sh`
- npm scripts:
- `build:mac`
- `build:win`
- `release:prep`
- Local skill for release workflow:
- `.agents/skills/petes-pdf-release/SKILL.md`

### Changed

- Release version baseline set to `v0.5.0`.
- Electron Builder config updated for packaged runtime reliability:
- `asarUnpack` includes `scripts/**`
- build resources configured under `build.directories`
- default macOS target remains DMG
- Default app output root moved to user documents:
- `~/Documents/Pete's PDF to MD Output`
- `.gitignore` updated to ignore `dist/` artifacts.
- README consolidated to one canonical "Build From Source" release-build section.

### Fixed

- Packaged app conversion failure `spawn ENOTDIR` by using packaged-safe script paths and working directory in `electron/main.cjs`.
- Packaged app subprocess mode by setting `ELECTRON_RUN_AS_NODE=1` for conversion child process.
- Script path resolution in packaged builds by adding robust `extract_outline.py` lookup in `scripts/phase1.js`.
- "Output directory not found" behavior by ensuring output root exists and falling back to opening root when per-PDF folder is not yet created.
- PyMuPDF detection issues in packaged app by probing multiple Python interpreters and allowing override via `PDF_TO_MD_PYTHON` / `PYTHON_BIN`.

### Docs

- README expanded with:
- source build instructions for macOS and Windows
- release upload guidance for GitHub Releases
- troubleshooting for:
- `.app` vs `.dmg` output confusion
- `spawn ENOTDIR`
- missing PyMuPDF in packaged runs
- output-folder open errors
