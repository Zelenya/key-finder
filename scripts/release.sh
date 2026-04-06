#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: ./scripts/release.sh <version>

Examples:
  ./scripts/release.sh 0.1.0
  ./scripts/release.sh v0.1.0
EOF
}

die() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

VERSION_INPUT="${1:-}"

if [[ -z "${VERSION_INPUT}" ]]; then
  usage
  exit 1
fi

if [[ "${VERSION_INPUT}" == v* ]]; then
  TAG="${VERSION_INPUT}"
else
  TAG="v${VERSION_INPUT}"
fi

APP_PATH="target/release/bundle/osx/Key Finder.app"
DIST_DIR="dist"
ZIP_PATH="${DIST_DIR}/Key-Finder-${TAG}-macos-arm64.zip"
ASSET_LABEL="Key Finder for macOS (Apple Silicon)"
RELEASE_NOTES="Apple Silicon macOS build."

require_command cargo
require_command git
require_command gh
require_command codesign
require_command ditto

if ! cargo bundle --help >/dev/null 2>&1; then
  printf 'Installing cargo-bundle...\n'
  cargo install cargo-bundle
fi

if ! gh auth status >/dev/null 2>&1; then
  die "gh is not authenticated. Run: gh auth login"
fi

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null 2>&1; then
  die "git tag already exists locally: ${TAG}"
fi

if [[ -e "${ZIP_PATH}" ]]; then
  die "release archive already exists: ${ZIP_PATH}"
fi

printf 'Building release bundle for %s...\n' "${TAG}"
cargo bundle --release

if [[ ! -d "${APP_PATH}" ]]; then
  die "expected app bundle was not created: ${APP_PATH}"
fi

printf 'Ad-hoc signing app bundle...\n'
codesign --force --deep --sign - "${APP_PATH}"

mkdir -p "${DIST_DIR}"

printf 'Creating archive %s...\n' "${ZIP_PATH}"
ditto -c -k --sequesterRsrc --keepParent "${APP_PATH}" "${ZIP_PATH}"

printf 'Creating and pushing git tag %s...\n' "${TAG}"
git tag "${TAG}"
git push origin "${TAG}"

printf 'Creating GitHub release %s...\n' "${TAG}"
gh release create "${TAG}" \
  "${ZIP_PATH}#${ASSET_LABEL}" \
  --title "${TAG}" \
  --notes "${RELEASE_NOTES}"

printf 'Release complete: %s\n' "${TAG}"
