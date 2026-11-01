---
name: petes-pdf-release
description: Prepare and publish a macOS GitHub release for this repository, including version consistency checks, changelog/readme release updates, and DMG artifact creation. Use when the user asks to cut a release, build/upload a DMG, or verify release readiness.
---

# petes-pdf-release

Run this workflow for repository release prep:

1. Confirm target version as semver (for example `1.0.0`). Its display form drops a zero patch number (`1.0`).
2. Ensure `package.json`, `rust/converter/Cargo.toml`, `rust/app/Cargo.toml` and `rust/app/tauri.conf.json` all have that version.
3. Ensure `CHANGELOG.md` contains `## [<display version>] - YYYY-MM-DD`.
4. Ensure `README.md` includes:
- current version (`v<display version>`)
- GitHub Releases URL: `https://github.com/pbeens/Petes-PDF-to-MD/releases`
- DMG artifact instructions
5. Run metadata checks:
- `npm run release:prep -- --version <version>`
6. Build DMG when requested:
- `npm run release:prep -- --version <version> --build`
7. Verify artifact exists in `dist/` and report exact filename: `Pete-s-PDF-to-MD-v<display version>-macOS.dmg` (for example `v1.0`). Each build also advances the revision letter in `package.json` (`buildRevision`); only debug builds show it. Commit the `package.json` change with the build.
8. In the final response, provide:
- release tag suggestion (`v<display version>`, for example `v1.0`)
- upload path (`dist/<artifact>.dmg`)
- any blocker (for example no network for `npm install`).

If build fails because dependencies are missing, run `npm install` (Tauri CLI) and check that `cargo` is installed, then retry. If network is unavailable, report that limitation and stop after metadata checks.
