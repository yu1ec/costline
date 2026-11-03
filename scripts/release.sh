#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

DRY_RUN=0
BUMP=""
GENERATE_NOTES=1

usage() {
  cat >&2 <<'EOF'
用法：scripts/release.sh <type> [--dry-run] [--no-notes]

类型：
  patch / fix / bug / 修复       问题修复：0.1.2 -> 0.1.3
  minor / feature / feat / 功能  新增功能：0.1.2 -> 0.2.0
  major / breaking / 大版本     不兼容变更：0.1.2 -> 1.0.0

选项：
  --dry-run    只预览版本变化，不修改文件
  --no-notes   跳过生成 release-notes/v<version>.md

示例：
  make release TYPE=patch
  make release TYPE=minor DRY_RUN=1
  ./scripts/release.sh fix --dry-run --no-notes
EOF
}

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    --no-notes) GENERATE_NOTES=0 ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      if [[ -n "$BUMP" ]]; then
        echo "多余的参数：$arg" >&2
        usage
        exit 1
      fi
      BUMP="$arg"
      ;;
  esac
done

if [[ -z "$BUMP" ]]; then
  usage
  exit 1
fi

current_version() {
  node -p "require('./package.json').version"
}

bump_label() {
  case "$1" in
    patch|fix|bug|bugfix|修复|修复bug|补丁) echo "patch" ;;
    minor|feature|feat|功能|小版本) echo "minor" ;;
    major|breaking|break|大版本|主版本) echo "major" ;;
    *)
      echo "未知的发布类型：$1" >&2
      echo "请使用 patch、minor 或 major（也支持 fix / feature / 大版本）。" >&2
      exit 1
      ;;
  esac
}

next_version() {
  local current="$1"
  local bump="$2"
  local base="${current%%[-+]*}"
  local major minor patch

  IFS='.' read -r major minor patch <<< "$base"
  if [[ -z "$major" || -z "$minor" || -z "$patch" ]]; then
    echo "当前版本号不合法：$current" >&2
    exit 1
  fi

  case "$bump" in
    patch)
      patch=$((patch + 1))
      ;;
    minor)
      minor=$((minor + 1))
      patch=0
      ;;
    major)
      major=$((major + 1))
      minor=0
      patch=0
      ;;
  esac

  echo "${major}.${minor}.${patch}"
}

CURRENT="$(current_version)"
KIND="$(bump_label "$BUMP")"
NEXT="$(next_version "$CURRENT" "$KIND")"

case "$KIND" in
  patch) KIND_DESC="问题修复（patch）" ;;
  minor) KIND_DESC="新增功能（minor）" ;;
  major) KIND_DESC="不兼容变更（major）" ;;
esac

echo "发布类型：$KIND_DESC"
echo "版本变化：$CURRENT -> $NEXT"

if [[ "$DRY_RUN" == "1" ]]; then
  echo ""
  echo "仅为预览，未修改任何文件。"
else
  bash "$(dirname "$0")/bump-version.sh" "$NEXT"
  bash "$(dirname "$0")/check-version.sh" "$NEXT"

  if [[ "$GENERATE_NOTES" == "1" ]]; then
    echo ""
    if ! bash "$(dirname "$0")/release-notes.sh" "$NEXT"; then
      echo "警告：发布说明生成失败，可稍后手动运行：" >&2
      echo "  scripts/release-notes.sh $NEXT" >&2
    fi
  fi
fi

echo ""
echo "下一步（请手动执行）："
echo "  git add -A"
echo "  git commit -m \"chore: release v$NEXT\""
echo "  git tag v$NEXT"
echo "  git push origin HEAD --tags"
echo ""
echo "Release 创建后，CI 会自动应用 release-notes/v$NEXT.md。"
echo "重新生成或编辑说明：scripts/release-notes.sh $NEXT"
echo "推送到已存在的 Release：scripts/release-notes.sh $NEXT --publish"
