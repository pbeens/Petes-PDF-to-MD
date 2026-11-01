# Tasks

- [ ] **Merge prompt logs.** Combine `prompts-mac-mini-m4.md` (this Mac mini M4) with the prompt log on the other computer into a single `prompts.md`. Keep strict newest-first order, drop duplicate entries, and keep each entry's Host line. Then delete the per-computer files. Until this is done, new entries on the Mac mini go in `prompts-mac-mini-m4.md`.
- [ ] **Screenshot for 1.0.** Replace `docs/images/app-screenshot.png` (still the 0.8 Electron window) with the 1.0 app after a conversion. The README uses the same image.
- [ ] **Publish 1.0.** Commit, tag `v1.0`, upload `dist/Pete-s-PDF-to-MD-v1.0-macOS.dmg` to a GitHub release, and publish the website (`docs/`, site URL `https://peter.beens.ca/Petes-PDF-to-MD/`).
- [ ] **Windows version.** Build and test the Tauri app on Windows (`npx tauri build` in `rust/app`), add a `build:win` script, and update the README and website.
