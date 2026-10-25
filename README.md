# Pete's PDF to MD

Download the latest app from GitHub Releases: [https://github.com/pbeens/Pete-s-PDF-to-MD/releases](https://github.com/pbeens/Pete-s-PDF-to-MD/releases)

Open-source utility to convert PDF documents to high-quality Markdown for AI workflows.

<p align="center">
  <a href="images/petes-pdf-to-md-v0.7.0.png" target="_blank" rel="noopener noreferrer">
    <img src="images/petes-pdf-to-md-v0.7.0.png" alt="Pete's PDF to MD GUI screenshot" width="75%">
  </a>
</p>

## Current Version

`v0.8.0`

## Releases

- GitHub Releases: `https://github.com/pbeens/Pete-s-PDF-to-MD/releases`
- Changelog: `CHANGELOG.md`

## Requirements

The app does not include Python. It runs its converter with the Python already installed on your computer, so even the downloaded app needs:

- Python 3.10+ (or newer)
- PyMuPDF Python package, version 1.23 or newer (older versions convert but skip tables; the app warns you when this happens)
- Optional: `pdf-inspector` Python package. When installed, PDFs tagged for accessibility get their real multi-level headings (`python3 -m pip install pdf-inspector`)

Install or upgrade PyMuPDF:

- macOS/Linux: `python3 -m pip install --upgrade pymupdf`
- Windows (PowerShell): `py -m pip install --upgrade pymupdf`

Building from source also needs Node.js 20+ (includes `npm`).

### Running from source: use a virtual environment (recommended)

The downloaded app does not use a virtual environment; it looks for Python on the system (see above). When running from source, a local `.venv` keeps this project's packages separate. `npm run gui` and `npm run phase1` use the project's `.venv` automatically when it exists.

From the project root, create a local virtual environment named `.venv`:

- macOS/Linux: `python3 -m venv .venv`
- Windows (PowerShell): `py -m venv .venv`

Activate it before installing Python packages:

- macOS/Linux: `source .venv/bin/activate`
- Windows (PowerShell): `.\.venv\Scripts\Activate.ps1`

Then install the packages above (`python -m pip install --upgrade pymupdf pdf-inspector`).

## Build From Source

### macOS

1. `npm install`
2. `python3 -m pip install pymupdf`
3. Run unpackaged app for development:
   `npm run gui`
4. Build release artifact (DMG):
   `npm run release:prep -- --version 0.8.0 --build`


- `dist/Pete-s-PDF-to-MD-v0.8.0<letter>-macOS.dmg`

Each macOS build gets the next revision letter (`0.8.0a`, `0.8.0b`, …) so you can tell builds apart. The same version appears in the app's window title and build stamp. The letter is stored in `package.json` (`buildRevision`) and restarts at `a` when the version changes. Show the current one with `npm run version:show`.

Debug builds (`npm run build:mac:debug`, file `…v0.8.0b-debug-macOS.dmg`) show "(debug version)" in the title bar and write each conversion to a per-version folder, `<output folder>/<PDF name>/v0.8.0b/`, so results from earlier builds are kept for comparison. Normal runs only replace the `.md` files directly in `<PDF name>/`, never these folders. When running from source, `npm run gui:debug` does the same.

### Windows (PowerShell)

1. `npm install`
2. `py -m pip install pymupdf`
3. Run unpackaged app for development:
   `npm run gui`
4. Build release installer:
   `npm run build:win`

Release artifact:

- `dist/*.exe`

Upload release artifacts to:

- `https://github.com/pbeens/Pete-s-PDF-to-MD/releases`

Build notes:

- `dist/mac-arm64/*.app` is an intermediate output for macOS packaging.
- Upload the `.dmg` from `dist/` for macOS releases.

## Project Status

Active development (usable beta).

Current priority:

- extract a reliable heading outline from PDFs
- use headings to split output into smaller sub-documents for LLM workflows

Future considerations:

- package Python and its libraries inside the app so nothing needs to be installed (a Rust/Tauri rewrite is being evaluated)
- add a search function in the app to quickly find sections/content within converted output

## Planning

Implementation options and milestone plan:

- `docs/implementation-plan.md`

## Phase 1 (Cross-Platform)

Current Phase 1 implements heading outline extraction and split planning using a cross-platform backend:

- launcher: `scripts/phase1.js` (Node.js)
- extractor: `scripts/extract_outline.py` (Python + PyMuPDF)

Run:

- `npm run phase1 -- --input "tests/pdfs/<file>.pdf"`

Experimental, command line only: `--text-engine hybrid` takes body text from `pdf-inspector` (better word spacing, bold/italic, multi-column order) and applies this app's cleanup. It needs `pdf-inspector` installed and falls back to PyMuPDF with a warning otherwise.

Outputs:

- `output/<pdf-name>/outline.json`
- `output/<pdf-name>/outline.md`
- `output/<pdf-name>/segments.json`
- `single` mode: `output/<pdf-name>/<pdf-name>.md` (root, no subfolder)
- `sections` mode: `output/<pdf-name>/Sections/*.md` (one merged markdown file per heading)
- `major` mode: `output/<pdf-name>/By Major Heading/*.md` (one merged markdown file per major heading)

## Extraction Accuracy

- PDF structure varies significantly across files; extracted headings, section boundaries, and text flow may not be 100% exact for every document.
- Best results come from well-structured PDFs with a reliable embedded outline/table of contents.
- Always review output before downstream publishing or automation.

## GUI (Electron)

Minimal desktop UI is scaffolded and wired to the existing conversion pipeline.

Files:

- `electron/main.cjs` (main process + IPC)
- `electron/preload.cjs` (safe renderer bridge)
- `app/index.html`, `app/styles.css`, `app/renderer.js` (UI)

### macOS Run

1. `npm install`
2. `python3 -m pip install pymupdf`
3. `npm run gui`

For release packaging instructions, see **Build From Source** above.

### Windows Run (PowerShell)

1. Install Node.js (LTS) and Python 3.
2. `npm install`
3. `py -m pip install pymupdf`
4. `npm run gui`

Notes for Windows:

- GUI launch (`npm run gui`) and conversion pipeline are cross-platform.

Current GUI features:

- select a PDF file
- drag and drop a PDF onto the `Select PDF` button
- select output root folder
- choose conversion output mode: `One file`, `By major heading`, or `Individual sections`
- run conversion (calls existing `scripts/phase1.js`)
- preview `outline.md`
- browse sections from `outline.json`
- view section markdown content
- right-click section action: `Open in folder`
- right-click section action: `Open in default program`
- right-click section action: `Copy path`
- open output folder
- warnings dialog after a conversion (for example, PyMuPDF too old for table detection)
- exact build shown in the window title (`v0.8.0e`); debug builds add "(debug version)"

Notes:

- The app now quits when the last window is closed (so terminal returns immediately).
- Build stamp uses local time format: `yyyy-mm-dd hh:mm`.
- Section path display uses middle truncation to preserve start and filename.

## Troubleshooting

### "Could not open output folder ... Output directory not found"

- The app now defaults to `~/Documents/Pete's PDF to MD Output`.
- If the error appears, set Output Folder explicitly in the app, then run conversion once.

### "Conversion failed ... spawn ENOTDIR"

- This indicates an older packaged build.
- Rebuild and reinstall using:
  `npm run release:prep -- --version 0.8.0 --build`

### "Conversion failed ... Output files are locked by another program"

- Close any open files, previews, or editors under the output markdown folder for the selected mode.
- Retry conversion after releasing file locks.

### "Conversion failed ... invalid/corrupt PDF"

- The file may be incomplete/corrupt or not a valid PDF despite extension.
- Open in a PDF viewer and re-export/save as a new PDF, then retry.

### "PyMuPDF is not installed"

- Install PyMuPDF for the same Python interpreter used by the app:
  `/opt/homebrew/bin/python3 -m pip install pymupdf`
- If needed, launch the app from Terminal with:
  `PDF_TO_MD_PYTHON=/opt/homebrew/bin/python3 open /Applications/Pete\\'s\\ PDF\\ to\\ MD.app`

### "PyMuPDF ... has no table detection"

- The Python interpreter the app found has PyMuPDF older than 1.23, so tables are written as plain text.
- Upgrade PyMuPDF for that interpreter (the warning shows the version), for example:
  `/usr/local/bin/python3 -m pip install --upgrade pymupdf`

### "I only see a .app file after build"

- `dist/mac-arm64/*.app` is an intermediate output.
- Upload the newest `dist/Pete-s-PDF-to-MD-v0.8.0<letter>-macOS.dmg` to GitHub Releases.

## License

This project is licensed under GPL-3.0. See `LICENSE`.

