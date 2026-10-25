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
# --debug builds a debug version: "(debug version)" in the title bar and output in a
# per-version folder (<pdf name>/v0.8.0b/). The flag is baked into the bundled package.json.
debug_args=()
name_suffix=""
builder_args=()
for arg in "$@"; do
  if [[ "${arg}" == "--debug" ]]; then
    debug_args=(-c.extraMetadata.debugBuild=true)
    name_suffix="-debug"
  else
    builder_args+=("${arg}")
  fi
done

display_version="$(node scripts/bump-revision.js)"
artifact="Pete-s-PDF-to-MD-v${display_version}${name_suffix}-macOS"
printf 'Building macOS DMG for v%s%s...\n' "${display_version}" "${name_suffix:+ (debug)}"
npx electron-builder --mac dmg \
  -c.dmg.artifactName="${artifact}.\${ext}" \
  ${debug_args[@]+"${debug_args[@]}"} \
  ${builder_args[@]+"${builder_args[@]}"}
printf 'Build complete: dist/%s.dmg\n' "${artifact}"
