# Archive: Electron app and Python converter (up to v0.9)

Pete's PDF to MD 1.0 replaced these files with the Rust converter and the Tauri app in `rust/`. They are kept here for reference and are no longer built, tested or maintained. Nothing in the current app loads them.

The last release built from them is **v0.8.0e** (Electron window, Python converter). The 0.9 development builds still used the Electron window, by then running the Rust converter. To run the old version, check out a commit from before 1.0 (for example `git checkout a447420`); the files in this folder will not run from here, because their paths assume the original layout.

| File | What it was |
|---|---|
| `electron/main.cjs` | Electron main process: window, menus, file dialogs, conversion subprocess, output loading. Replaced by `rust/app/src/main.rs` (same commands). |
| `electron/preload.cjs` | Bridge between the Electron window and the main process. Replaced by `app/tauri-bridge.js`. |
| `scripts/phase1.js` | Command-line launcher: found a Python with PyMuPDF (or the Rust binary) and ran the converter. Replaced by running `rust/target/release/petes-pdf-to-md` directly (`npm run convert`). |
| `scripts/extract_outline.py` | The Python/PyMuPDF converter. Its rules were ported to `rust/converter/` (see `rust/README.md`). |
| `scripts/build-mac.sh` | electron-builder DMG build (about 100 MB). Replaced by the Tauri build, `scripts/build-mac.sh` at the repository root (about 6 MB). |
| `petes-pdf-to-md-v0.7.0.png` | Screenshot of the v0.7.0 window. |
| `audit-anydoc-pdf-inspector.md` | The October 2026 review of firecrawl/anydoc and pdf-inspector that led to the Rust rewrite. |

The `electron`, `electron-builder` and Python dependencies were removed from `package.json` at the same time; see the 1.0 entry in `CHANGELOG.md`.
