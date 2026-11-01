# Rust converter and Tauri app

`converter/` is the PDF-to-Markdown converter (library and command-line tool). `app/` is the Tauri desktop app, which shows the web page in `../app/` and calls the converter library directly; see the end of this file.

Since 1.0 this is the whole program. It replaced the Python/PyMuPDF converter (`archive/electron-python/scripts/extract_outline.py`) and writes the same files (`outline.json`, `outline.md`, `segments.json`, and the `single` / `major` / `sections` layouts). It reads PDFs with [pdf-inspector](https://github.com/firecrawl/pdf-inspector) (MIT) and [lopdf](https://github.com/J-F-Liu/lopdf); the command-line tool is one ~8 MB binary.

## Build and run

```bash
cd rust
cargo build --release
./target/release/petes-pdf-to-md --input ../tests/pdfs/ICD2O_Curriculum.pdf --out-dir /tmp/out --conversion-mode single
cargo test --release      # unit tests
```

Options: `--input`, `--out-dir`, `--out-subdir`, `--conversion-mode single|major|sections`, `--include-section-metadata 0|1`. Progress lines start with `PROGRESS:`. From the repository root, `npm run convert -- --input file.pdf` builds and runs it.

## How it works

| Step | Module | Python original |
|---|---|---|
| Headings from bookmarks, tags, and coded headings tagged as paragraphs | `outline.rs` | `build_outline_from_toc`, `build_outline_from_tags`, `add_untagged_code_headings`, `normalize_outline` |
| Group text into lines; text-based headings | `layout.rs`, `outline.rs` | `build_outline_heuristic`, `infer_document_title` |
| Split pdf-inspector's Markdown at each heading; remove running headers, page numbers and repeated labels; move footnotes to the end of each section; rejoin split sentences | `body.rs` | `split_markdown_by_outline`, `find_furniture_lines`, `clean_hybrid_body`, `format_dot_leader_blocks`, `deduplicate_body_paragraphs` |
| Reading order: place paragraphs on their page, re-sort two-column pages, split fake two-column tables, take out pull-quotes | `reading.rs` | (new; Python ordered lines by column itself) |
| Rebuild run-on tables | `tables.rs` | (new; Python used PyMuPDF's table finder) |
| Write section files and indexes | `output.rs` | `write_outputs` |

This follows the Python converter's experimental *hybrid* path (pdf-inspector text plus this app's cleanup).

## Compared with the Python converter (0.8)

Measured on the test PDFs in `tests/pdfs/` before the Python converter was retired.

- **Same outline**: identical heading counts and levels from bookmarks and tags, and identical text-based headings (`PDF_TO_MD_NO_TAGS=1`) on the two PDFs without bookmarks.
- **Same text**: no meaningful words missing; Rust keeps a few that Python loses (hyphenated words, links).
- **4–15× faster** (a 242-page book: 2.0 s vs 7.4 s).
- **Better**: keeps bold, italics and links; writes footnote markers as superscripts.
- **Run-on tables** (cells read row by row) are rebuilt from the page layout (`tables.rs`) and kept only when no words are lost; `PDF_TO_MD_DEBUG_TABLES=1` shows the decisions.
- **Hidden text layers**: pages that are pictures with an invisible text layer (exported slides, pre-OCR'd scans) are read through pdf-inspector's region extraction, which adopts that layer; bookmark titles are placed on their pages (`insert_pages`).
- **Reading order** (`reading.rs`): pdf-inspector's whole-document Markdown stays the base; each paragraph is assigned to its page, two-column pages are re-sorted (left column, then right), columns read across into a fake table are split back, and pull-quotes become blockquotes. Every split is checked to keep all words. `PDF_TO_MD_NO_REORDER=1` turns this off, `PDF_TO_MD_DEBUG_READING=1` shows the decisions. On `GCPSE_Efficiency.pdf` (two columns) it fixes the contents page, a column read out of order and two pull-quotes, and changes nothing on the four single-column test PDFs.

Known gaps:

- Four achievement charts in the Ontario PDFs still come out as run-on text (the Python converter renders them as tables). In those, pdf-inspector merges text from neighbouring cells into one piece.
- Footnotes whose text starts in lowercase stay where they are instead of moving to the end of the section.
- Photo captions and figure labels can still sit inside the sentence they interrupt.
- Pages converted from a hidden text layer (slides, pre-OCR'd scans) read side-by-side content across, one row per line.
- No Windows build tested yet.

## Tauri app (`app/`)

- `npm run gui` runs it from source; `npm run build:mac` (or `build:mac:debug`) builds a DMG into `dist/` (about 6 MB; the app is about 13 MB installed).
- `src/main.rs` holds the commands (their names and return shapes come from the retired Electron app); `../app/tauri-bridge.js` maps the page's `pdfToMdApi` calls onto them. Files dropped anywhere on the window reach the page as `file-drag` events.
- `cargo test -p petes-pdf-to-md-app` sends the bridge's calls (same command names and argument JSON) through Tauri's mock runtime, including a full conversion and a file drop.
- The icon in `icons/` is a placeholder; replace `icons/source.png` and run `npx tauri icon rust/app/icons/source.png -o rust/app/icons` to regenerate the sizes.
