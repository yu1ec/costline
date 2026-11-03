#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

TAG_VERSION="${1:-}"
if [[ -z "$TAG_VERSION" ]]; then
  if ! TAG_VERSION="$(git describe --tags --exact-match 2>/dev/null)"; then
    echo "用法：scripts/check-version.sh <version-or-tag>" >&2
    echo "      或在已打 tag 的提交上直接运行" >&2
    exit 1
  fi
fi

TAG_VERSION="${TAG_VERSION#v}"
NPM_VERSION="$(node -p "require('./package.json').version")"
NPM_LOCK_VERSION="$(node -p "require('./package-lock.json').version")"
TAURI_VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
CARGO_VERSION="$(
  awk '
    /^\[workspace\.package\]/ { in_section = 1; next }
    /^\[/ { in_section = 0 }
    in_section && /^version[[:space:]]*=/ {
      gsub(/^version[[:space:]]*=[[:space:]]*"|"[[:space:]]*$/, "", $0)
      print
      exit
    }
  ' Cargo.toml
)"

if [[ -z "$CARGO_VERSION" ]]; then
  echo "没有在 Cargo.toml 的 [workspace.package] 中找到版本号" >&2
  exit 1
fi

MISMATCH=0
if [[ "$TAG_VERSION" != "$NPM_VERSION" || "$TAG_VERSION" != "$NPM_LOCK_VERSION" || "$TAG_VERSION" != "$TAURI_VERSION" || "$TAG_VERSION" != "$CARGO_VERSION" ]]; then
  MISMATCH=1
fi

# Cargo.lock 里的 workspace crate 也跟随统一版本号。
for pkg in costline-core costline-cli costline-desktop; do
  LOCK_VERSION="$(
    awk -v pkg="$pkg" '
      /^\[\[package\]\]/ { name = ""; version = "" }
      /^name = "/ {
        name = $0
        gsub(/^name = "|"$/, "", name)
      }
      /^version = "/ {
        version = $0
        gsub(/^version = "|"$/, "", version)
        if (name == pkg) { print version; exit }
      }
    ' Cargo.lock
  )"
  if [[ -n "$LOCK_VERSION" && "$LOCK_VERSION" != "$TAG_VERSION" ]]; then
    MISMATCH=1
    echo "Cargo.lock 中 $pkg 的版本为 ${LOCK_VERSION}，与 tag $TAG_VERSION 不一致" >&2
  fi
done

if [[ "$MISMATCH" == "1" ]]; then
  echo "版本不一致："
  echo "  tag:   $TAG_VERSION"
  echo "  cargo: $CARGO_VERSION"
  echo "  tauri: $TAURI_VERSION"
  echo "  npm:   $NPM_VERSION"
  echo "  lock:  $NPM_LOCK_VERSION"
  exit 1
fi

echo "版本一致：$TAG_VERSION"
