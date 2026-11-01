#!/usr/bin/env bash
# Build the app (rust/app, Tauri) as a macOS DMG in dist/:
#   dist/Pete-s-PDF-to-MD-v1.0-macOS.dmg          (release)
#   dist/Pete-s-PDF-to-MD-v1.0b-debug-macOS.dmg   (--debug)
# A debug build shows "(debug version)" in the title bar and writes output to a
# per-version folder (<pdf name>/v1.0b/). Every build advances the revision
# letter in package.json; only debug builds show it.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

for tool in cargo npx node; do
  if ! command -v "${tool}" >/dev/null 2>&1; then
    printf '%s is required to build the app.\n' "${tool}"
    exit 1
  fi
done

debug=""
name_suffix=""
for arg in "$@"; do
  if [[ "${arg}" == "--debug" ]]; then
    debug="1"
    name_suffix="-debug"
  fi
done

display_version="$(node scripts/bump-revision.js ${debug:+--debug})"
printf 'Building app v%s%s...\n' "${display_version}" "${name_suffix:+ (debug)}"
(cd rust/app && PDF_TO_MD_DEBUG_BUILD="${debug}" npx tauri build --bundles dmg)

dmg="$(ls -t rust/target/release/bundle/dmg/*.dmg | head -1)"
mkdir -p dist
artifact="dist/Pete-s-PDF-to-MD-v${display_version}${name_suffix}-macOS.dmg"
cp "${dmg}" "${artifact}"
printf 'Build complete: %s\n' "${artifact}"
