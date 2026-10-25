# Audit: firecrawl/anydoc vs. Pete's PDF to MD

- **Date:** 2026-10-24
- **Reviewed:** [firecrawl/anydoc](https://github.com/firecrawl/anydoc) at commit `e7178e0` (v0.2.4, MIT), and its PDF engine [firecrawl/pdf-inspector](https://github.com/firecrawl/pdf-inspector) (v1.25.2, MIT)
- **Compared against:** Pete's PDF to MD v0.7.1 (`scripts/extract_outline.py`, `scripts/phase1.js`, `electron/main.cjs`)
- **Scope:** Read-only review. No changes were made to this program.

## TL;DR

anydoc itself is not very interesting for this program. It is a converter for **office documents** (doc/docx, ppt/pptx, xls/xlsx, ODF, RTF, EPUB, CSV). For PDFs it adds almost nothing: `src/formats/pdf.rs` is 52 lines that call another Firecrawl crate, **pdf-inspector**, and handle its errors.

**pdf-inspector is the part worth our attention.** It is a pure-Rust PDF-to-Markdown engine with Python and Node bindings. It already does much of what `extract_outline.py` does by hand, and some things it doesn't do at all. We should evaluate it as a possible replacement for, or second engine beside, PyMuPDF for the **body text**. Our own code should stay in charge of the **outline and splitting**, because that is this app's real value and neither Firecrawl project does it.

My recommendation: run a hands-on comparison on our test PDFs (see "Next step" at the bottom) before committing to anything. The ideas below are worth borrowing either way.

## Hands-on results (added after testing)

On 2026-10-24 I ran pdf-inspector 1.25.2 (`process_pdf` + `extract_structure_elements`) and our `extract_outline.py --conversion-mode single` on both PDFs in `test-data/pdfs/`. All output went to a scratch folder outside the project.

| | ICD2O_Curriculum (88 pp.) | Digital-Technology… (30 pp.) |
|---|---|---|
| pdf-inspector time | 173 ms | 98 ms |
| Classified as | text-based, no OCR pages, no encoding issues | text-based, no OCR pages, no encoding issues |
| **Tagged PDF?** | **Yes**: 101 tagged headings (H1 6, H2 52, H3 5, H4 38) | **Yes**: 65 tagged headings (H1 7, H2 26, H3 12, H4 10, H5 10) |
| Embedded bookmarks used by us? | No (all our headings came from `text-heuristic`) | — |
| Our outline entries | **281, of which 151 are bullet points** (`• look for and act on…`) | 57 headings in output |
| pdf-inspector heading lines | 230 | 79 |

### What I found

1. **Our outline is badly over-split on ICD2O, and tags would fix it.** The PDF has no usable bookmarks, so `build_outline_heuristic` takes over. It promotes over half of all bulleted list items to `###` headings. In the output, each bullet becomes its own "section" with its own metadata block, and the bullet text appears twice (once as a heading, once as a list item). In `sections` mode that means roughly 150 junk section files. The PDF's own tags list exactly 101 real headings. **This is the single most valuable finding:** reading the structure tree (idea #1 below) would give a correct outline for this whole class of Ontario Ministry documents. Separately, the heuristic should probably never treat a line starting with `•` as a heading. That's a cheap fix, independent of any library.

2. **pdf-inspector's heading levels follow the document structure well.** "Considerations for program planning" → `#`, "Roles and Responsibilities" → `##`, "Students" / "Parents" → `###`. Ours flattens nearly everything to `###`. Its mistakes: it turns "View in ASL" links (43×) and lines like "© King's Printer for Ontario, 2023" or "Last updated: June 2023" into `#####` headings.

3. **Our cleanup still wins in several places.**
   - **Page furniture:** pdf-inspector leaves a stray `Extract` label in the text **86 times** (a page-edge marker in these documents), sometimes mid-sentence: "…sixteen years of age or older) that / Extract / secondary students…". We strip it and stitch the sentence back together correctly.
   - **TOC pages:** pdf-inspector leaves 78 raw dot-leader lines (`Introduction……………7`) with its default settings; its `compact` profile may help, but I didn't test that. We turn the TOC into a clean `| Section | Page |` table.
   - **Lists:** pdf-inspector sometimes runs a paragraph straight into the last bullet with no blank line, which breaks the Markdown.

4. **Tables are flawed in both, in different ways.** For the "Version History" table, **ours glues words together** (`NewGrade10DigitalTechnologyandInnovations…`, `June14 , 2023`), which is a real bug worth fixing. pdf-inspector keeps the words intact but splits one logical row across four rows (`June 14,` / `2023` in separate rows).

5. **Small pdf-inspector wins:** curriculum expectation codes come out bold (`**A1.1** apply computational thinking…`), which helps both readers and LLMs. Ours are plain. Body paragraphs and list items are otherwise very similar in quality.

### Tables, head to head (ICD2O)

Our extractor has several table-specific tricks in `extract_outline.py`. Here is how each one performed on ICD2O, and what pdf-inspector produced for the same table:

| Our trick | Did it work? | pdf-inspector on the same content |
|---|---|---|
| `format_dot_leader_blocks` (TOC lines → `Section / Page` table) | **Yes**, clean 81-row table | **Worse**: 78 raw `Title……7` lines left as paragraphs |
| `format_course_table_blocks` + `parse_course_rows_from_flat` (rebuild the Grade/Course/Type/Code/Prerequisite table from flat text) | **Did not fire.** The table came from `find_tables()` instead, so it has glued words (`DigitalTechnologyand Innovationsinthe…`) and a wrong `Categories / Level 1–4` header on top (see below) | **Better**: perfect 5-column table, correct words, no special-case code needed |
| `markdown_table_from_rows`: any 5-column table is assumed to be an achievement chart and gets the `Categories / Level 1…4` header | Right for achievement charts, **wrong for the course table**, which also has 5 columns | n/a (uses the table's own header row) |
| `remove_duplicate_markdown_table_headers` / `is_achievement_header_row` | **Partly.** It matches `Level 1` with a space, but extracted cells read `Level1`, so most charts still show a duplicate `Categories / Level1…` row | Header row usually correct, but sometimes leaks out as a `##### Categories Level 1 Level 2…` heading above the table |
| Merging multi-line cells into one row | **Yes.** Each achievement-chart criterion is one row, which is the right structure | **Mixed.** Some charts are perfect (Science & Technology: one row per criterion, full sentences). Others split every *line* of a cell into its own row (`The student` / `demonstrates` / `limited` …), and a page number (`56`) plus `Extract` ended up inside one table |
| `strip_footnote_prefix_from_table_rows`, `strip_inline_sup_markers` | No problems seen in this file | No `<sup>` problems either |
| (no trick) Text inside table cells | **Broken across almost every table.** Spaces between words are lost and punctuation is scrambled: `Knowledgeofcontent(eg facts . ., , genres terms…`, `Thestudent demonstrates limited knowledgeof content .` | **Correct.** Normal words and punctuation in every cell (`Knowledge of content (e.g., facts, terminology, definitions)`) |

**Bottom line on tables:** your tricks get the *row structure* right more often, especially merging multi-line cells, and your TOC handling is clearly better. But the *text inside* the tables, which is what an LLM actually reads, is much worse in ours. pdf-inspector's cells read correctly almost everywhere. That's because it reconstructs spaces from the gaps between glyphs (`extractor/word_gaps.rs`), while `table.extract()` in PyMuPDF only gets a space where the PDF actually draws a space character, and these Ministry PDFs often don't. Overall, **pdf-inspector produces the more usable tables on this document**, even though some of its row splits are messy.

**Likely fix in our code, no new dependency:** keep `find_tables()` for the cell *boundaries*, but rebuild each cell's text from `page.get_text("words", clip=cell_bbox)` joined with spaces, instead of using `table.extract()`. That should repair most of the gluing while keeping our good row structure. Two smaller fixes: only apply the `Categories / Level 1–4` header when the first column actually looks like achievement-chart categories, not merely when there are 5 columns; and match `Level\s*1` instead of `Level 1` when removing duplicate headers. *(These fixes are untested proposals.)*

### Verdict after testing

pdf-inspector is **not** a drop-in replacement. On these Ontario curriculum PDFs its body text is roughly on par with ours, and our page-furniture and TOC handling are better. But **its structure-tree reader is exactly what our outline step is missing.** Using the PDF's tags for headings would replace 281 noisy heuristic entries with the document's 101 real headings. That moves my recommendation from "(a) now, (b) maybe" to: **adopt tag-based headings first** (via `pdf_inspector.extract_structure_elements` as an extra Python dependency, or by reading `/StructTreeRoot` ourselves), and keep PyMuPDF and our cleanup for the body text.

Not tested: our pipeline's run time (it printed to the terminal and wasn't captured), the `compact` Markdown profile, and the other output modes.

## What the two projects actually are

| | anydoc | pdf-inspector | Pete's PDF to MD |
|---|---|---|---|
| Input | 14 office/ebook formats + PDF | PDF only | PDF only |
| Engine | Rust, one shared document model → one GFM writer | Rust, lopdf-based, ~137k lines | Python + PyMuPDF, ~1,600 lines of heuristics |
| Output | One Markdown string | One Markdown string, or one per page | Outline + split section files + GUI |
| Splitting by heading | No | No | **Yes (core feature)** |
| Uses PDF bookmarks (`/Outlines`) | No | No | **Yes** (`build_outline_from_toc`) |
| Tagged-PDF headings (structure tree) | via pdf-inspector | **Yes** (real H1–H6) | No |
| Scanned / image-only page detection | Yes (errors with page list) | **Yes**, per page, with reasons | No |
| Running header/footer removal | via pdf-inspector | Yes, two strategies | Yes (`build_margin_noise_profile`) |
| Tables | via pdf-inspector | Ruled + alignment-based, cross-page continuation | PyMuPDF `find_tables()` + custom fixes |
| Multi-column reading order | via pdf-inspector | Yes | Relies on PyMuPDF block order |
| Encrypted PDFs | Typed `Encrypted` error | Typed error, password option | Generic failure |
| Install burden | `npm` / `pip` prebuilt binaries | `npm` / `pip` prebuilt binaries | User must install Python 3.10+ **and** PyMuPDF |
| License | MIT | MIT | GPL-3.0 (MIT deps are compatible) |

## Things worth taking advantage of

### 1. Tagged-PDF headings (highest value, low effort)

Many government and education PDFs, including Ontario Ministry curriculum documents like our `ICD2O_Curriculum.pdf`, are *tagged* for accessibility. The tags label every heading as H1–H6. pdf-inspector reads them (`structure_tree.rs`, `extract_structure_elements()` in Python), and uses them in Markdown conversion (`markdown/convert.rs`).

Today we get headings two ways: from the bookmark TOC, or by guessing from font sizes (`build_outline_heuristic`). Tags would add a third source that sits between them. It is more reliable than font guessing and has exact positions, so we wouldn't need `infer_toc_heading_positions`. PyMuPDF can't easily read tags, which is one reason to look at pdf-inspector.

**Idea:** heading-source priority becomes *bookmarks → tags → font heuristics*. We could also use the tags to cross-check bookmark positions.

### 2. Detecting scanned or image-only pages

We have no OCR detection. A scanned PDF, or one with a few scanned pages, comes out silently empty or with gaps, and the user can't tell. anydoc's position (from `pdf.rs`) is a good one: *"output missing those pages would read as complete"*, so it refuses and names the pages.

pdf-inspector's `detect_pdf()` / `classify_pdf()` runs in milliseconds and returns `pdf_type` (`text_based` / `scanned` / `image_based` / `mixed`), `pages_needing_ocr`, and `has_encoding_issues` (broken font encodings that produce garbled text).

**Idea:** even if we stay on PyMuPDF, add a pre-flight check that warns in the GUI: "Pages 12–14 appear to be scanned images and will be missing from the output." We could do a rough version in PyMuPDF (page has images but no text), but pdf-inspector's version is more thorough.

### 3. Clear error types for users

anydoc maps every failure to a small set of named errors (`Encrypted`, `NeedsOcr { pages }`, `Malformed`, `Unsupported`, `ResourceLimit`). The CHANGELOG shows we already put work into friendlier error messages (v0.6.0: invalid/corrupt PDF, locked output files). A fixed error vocabulary passed from the Python script to Electron (for example a JSON `error_code` field) would let the GUI show a specific message for each case instead of matching text. **This is a pure design idea; no dependency needed.**

### 4. Removing the Python requirement (bigger, longer-term)

Our biggest install hurdle is that users must install Python and PyMuPDF themselves. The README spends a lot of space on it, and `phase1.js` has to search for `python3` in several places. pdf-inspector ships a prebuilt Node package (`@firecrawl/pdf-inspector`, napi-rs). In principle the Electron main process could call it directly, and the app would need no Python at all.

The catch is that our ~1,600 lines of post-processing in `extract_outline.py` are Python. Moving to Node means porting them, or deciding that pdf-inspector's built-in cleanup makes most of them unnecessary. Only a real comparison can tell us which. This is not a quick win, but it is the most strategically interesting option.

A side note: PyMuPDF is AGPL-3.0 (or commercial). That's fine for us today because users install it themselves, but it would complicate bundling it into the installer. pdf-inspector is MIT and has no such issue.

### 5. Specific cleanup features to compare against ours

pdf-inspector already implements several things we built by hand. Each is worth a side-by-side test:

| Our function(s) | pdf-inspector equivalent |
|---|---|
| `build_margin_noise_profile` (repeats on ≥15% of pages, top/bottom 10% bands) | `strip_repeated_lines` plus `markdown/furniture.rs`, which also handles **short documents** (<3 pages) where repetition can't be measured, using position and font evidence. Their rule list is a good reference even if we don't adopt the library. |
| `format_dot_leader_blocks` | `--compact` / token-efficient profile collapses dot leaders |
| `extract_page_tables` + `markdown_table_from_rows` + three table-header fixes | Ruled-line and alignment-based detectors, plus **tables continuing across pages** |
| `stitch_orphan_continuations`, hyphen handling | `fix_hyphenation`, reading-order engine |
| `repair_mojibake` | ToUnicode/CID decoding plus a `has_encoding_issues` flag |
| — (none) | Multi-column reading order, RTL text, bold/italic/underline, links |

The headline benchmark is relevant. On a 200-PDF public benchmark (OCR off), pdf-inspector scored **0.875 overall vs. 0.735 for pymupdf4llm**, with a large gap on tables (0.81 vs. 0.40). It also ran about 36× faster. That isn't the same as our pipeline, which adds a lot on top of raw PyMuPDF, but it suggests their base extraction is stronger than what we start from.

### 6. Per-page Markdown for splitting

`extract_pages_markdown()` returns one Markdown string per page plus `pages_with_tables` and `pages_with_columns`, and there is an option to insert `<!-- Page N -->` markers. Our splitting works from page numbers and y-positions, so per-page output with page markers would plug into `segments.json` fairly naturally. That's true at least where a section starts on a page boundary. Mid-page splits would still need positional text (`extract_text_with_positions` / `extract_text_in_regions` give us that).

### 7. Testing practices (from anydoc)

anydoc's repo is well-engineered in ways we could copy cheaply:

- **Snapshot tests:** a committed set of sample files, with the expected Markdown checked in and diffed on every run. We have `test-data/pdfs/` but no automated check that output stays the same between versions. Our own acceptance criterion #3 ("output is deterministic across repeated runs") would be easy to enforce this way.
- **Robustness tests:** every sample file is deliberately damaged (truncated, corrupted) to make sure the converter fails cleanly instead of crashing.
- **LLM-judged quality benchmark** (`bench/`): blind A/B comparison against rendered ground truth. That's overkill for now, but it is a good way to judge changes to our post-processing rules.

## Things NOT worth taking

- **anydoc's office formats** (Word, PowerPoint, Excel, and so on). They are impressive, but outside this app's purpose ("PDF to MD"). If you ever want "Pete's Anything to MD", anydoc is the obvious engine and its Node package would drop straight into Electron. Treat that as a product decision, not an improvement.
- **Hosted OCR (Firecrawl Parse).** anydoc's `--ocr hosted` uploads the *whole document* to Firecrawl's servers. For a local, privacy-friendly desktop tool that's a poor default. pdf-inspector's *local* OCR (PP-OCRv6 via ONNX Runtime + PDFium) is more fitting, but it needs extra native libraries and a model download, so it's a later-phase idea at best.
- **Replacing our outline/split logic.** Neither project reads PDF bookmarks or splits output by heading. Our TOC-first approach, Front Matter handling, footnote rebalancing across sections and three output modes are the app's real value and should stay ours.
- **The WebAssembly build.** It's a cool demo, but Electron gives us native Node, so wasm buys nothing.

## Risks and unknowns

- **Small test set.** The hands-on comparison above covers only two PDFs, both from the same publisher (Ontario Ministry of Education). Both happen to be tagged. Untagged or multi-column PDFs could change the picture.
- **Young project, moving fast.** anydoc is v0.2.x. pdf-inspector is at 1.25, with lots of recent change (benchmarks refreshed July 2026). The API is broad, so pin exact versions.
- **Our document-specific fixes.** Functions like `parse_course_rows_from_flat` and `format_course_table_blocks` target Ontario curriculum layouts. A general-purpose engine won't know about those, so some post-processing will survive whatever we choose.
- **Heading levels.** pdf-inspector's font-based headings cap at H1–H4 (its own heading score, 0.788, is its weakest). Tagged PDFs and our bookmark TOC are better sources for heading levels.

## Suggested next step (when you want to act)

1. Install `pdf-inspector` in a test virtual environment and run `process_pdf()` and `extract_structure_elements()` on both PDFs in `test-data/pdfs/`.
2. Compare against our `--conversion-mode single` output for: table quality, header/footer noise, reading order, and dot-leader TOC pages.
3. Check whether `ICD2O_Curriculum.pdf` is tagged, and whether its tags match the bookmark outline.
4. Decide among three options:
   - **(a)** Borrow ideas only: scanned-page warning, error codes, snapshot tests.
   - **(b)** Add pdf-inspector as an optional second engine (`--engine pdf-inspector`; `phase1.js` already has an `--engine` flag).
   - **(c)** Plan a longer migration to the Node binding to drop the Python requirement.

*Steps 1–3 are done; see "Hands-on results" above.* Updated lean: **do tag-based headings first**, plus the two cheap bug fixes the test exposed (bullets promoted to headings; table cells losing spaces between words). Then do (a). Hold off on (b) and (c): pdf-inspector's body text wasn't clearly better than ours on these documents.
