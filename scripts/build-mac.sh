#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"

cd "${repo_root}"

if ! command -v npm >/dev/null 2>&1; then
  printf '%s\n' 'npm is required to build the macOS DMG.'
  exit 1
fi

if ! command -v npx >/dev/null 2>&1; then
  printf '%s\n' 'npx is required to run electron-builder.'
  exit 1
fi

if [[ ! -d "node_modules" ]]; then
  printf '%s\n' 'Missing node_modules. Run: npm install'
  exit 1
fi

# Every build gets the next revision letter (0.8.0a, 0.8.0b, ...) so installers never share a name.
display_version="$(node scripts/bump-revision.js)"
printf 'Building macOS DMG for v%s...\n' "${display_version}"
npx electron-builder --mac dmg \
  -c.dmg.artifactName="Pete-s-PDF-to-MD-v${display_version}-macOS.\${ext}" \
  "$@"
printf 'Build complete: dist/Pete-s-PDF-to-MD-v%s-macOS.dmg\n' "${display_version}"
