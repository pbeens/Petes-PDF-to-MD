# Test Plan and Inventory

This file is the canonical list of automated tests for this repository.

## Where This List Lives

- Canonical inventory: `tests/TEST_PLAN.md` (this file)
- Executable smoke case list: `tests/cases/smoke.json`
- Latest smoke run results: `tests/reports/smoke-latest.json` and `tests/reports/smoke-latest.md`
- Executable regression case list: `tests/cases/regression.json`
- Latest regression run results: `tests/reports/regression-latest.json` and `tests/reports/regression-latest.md`
- Test fixtures (inputs): `tests/pdfs/`
- Generated outputs from test executions: `tests/runs/` (and `tests/runs-single/` for ad-hoc single-mode runs)
- Executable runners: `scripts/` (they run the converter binary, `rust/target/release/petes-pdf-to-md`, which `npm test` builds first)
- Rust unit tests and the app's command tests: `npm run test:rust` (`cargo test --release` in `rust/`)
- Command entry points: `package.json` scripts

## Test Requirements

Every automated test must:

1. Be deterministic (same input -> same pass/fail result).
2. Be non-interactive (no prompts, no GUI clicks).
3. Use local fixtures only (no network dependencies).
4. Have machine-verifiable assertions (exact or rule-based checks).
5. Write to an isolated temp/output folder and clean up after itself.
6. Complete within a defined budget (`smoke` fast, `regression` slower).
7. Fail with a non-zero exit code on any unmet assertion.

## Required Test Tiers

### Smoke (required on every code change)

1. `CLI-01` A conversion runs successfully on a known PDF fixture.
Pass criteria: command exits 0 and creates `outline.json`, `segments.json`, `outline.md`, and at least one `.md` output file.

2. `CLI-02` An unknown conversion mode is rejected.
Pass criteria: command exits non-zero and prints an `Unknown mode` error.

3. `OUT-01` Single-mode output shape is valid.
Pass criteria: with `--conversion-mode single`, exactly one document markdown file is generated plus outline/segments indexes.

4. `TOC-001` Dot-leader table-of-contents reconstruction.
Pass criteria: generated markdown includes a TOC markdown table (`| Section | Page |` + separator row).

### Regression (required before merge/release)

Regression cases are defined in `tests/cases/regression.json` and include:

1. Changelog-derived extraction and post-processing fixes (`EXTRACT-*`, `TABLE-*`, `PARA-*`, `POST-*`).
2. Mode and fallback behavior checks (`SINGLE-*`, `MODE-*`, `FRONT-*`).
3. GUI regression checks (`GUI-*`, `OUT-*`), done by hand in the app.
4. Heading-source and text-preservation checks (`TAGS-*`, `FOOTNOTE-*`, `CODE-*`, `HEAD-*`, `PAGENUM-*`). `SINGLE-001`/`SINGLE-002` run with `PDF_TO_MD_NO_TAGS=1` so the tagged fixture still exercises the no-structure fallback.
5. Reading-order checks on a two-column document (`READ-*`, `QUOTE-*`, fixture `tests/pdfs/GCPSE_Efficiency.pdf`) and hidden text layers (`LAYER-001`).

Any case whose PDF is not in `tests/pdfs/` is skipped.

The Python-era cases for finding Python and for packaged script paths (`PY-001`, `PKG-001`, `PKG-002`) were removed in 1.0 with the Python converter and the Electron app.

### Quality/Guardrail (run in full suite)

1. `Q-01` Outline integrity.
Pass criteria: every outline entry has `level >= 1`, `page_start >= 1`, and non-empty `title`.

2. `Q-02` Segment integrity.
Pass criteria: every segment has a readable file path, `page_start <= page_end`, and positive `char_count` unless explicitly marked no-text.

3. `Q-03` No duplicate section filenames.
Pass criteria: generated section filenames are unique per run.

## Change-to-Test Mapping Rule

For every behavior change, add or update at least one test case in this file and implement it in automation scripts.

- If the change affects command-line behavior: update/add `CLI-*`.
- If the change affects content extraction/formatting: update/add `REG-*`.
- If the change affects output schema or indexing: update/add `Q-*`.

A code change is not complete until required `smoke` tests pass, and required `regression` tests pass for merge/release.
