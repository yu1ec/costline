#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  echo "用法：scripts/bump-version.sh <version>" >&2
  echo "示例：scripts/bump-version.sh 0.2.0" >&2
  exit 1
fi

VERSION="${VERSION#v}"

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
  echo "版本号不合法：${VERSION}（应形如 0.1.2）" >&2
  exit 1
fi

node scripts/set-version.mjs "$VERSION"

echo "已同步版本号 ${VERSION}："
echo "  package.json / package-lock.json"
echo "  Cargo.toml（workspace.package）/ Cargo.lock"
echo "  src-tauri/tauri.conf.json"
