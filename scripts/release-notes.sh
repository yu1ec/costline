#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION=""
FROM_TAG=""
TO_REF="HEAD"
OUT=""
USE_AI=1
PUBLISH=0
DRY_RUN=0
MODEL="${AI_MODEL:-}"
# 只为这次生成调用设置 effort，避免继承交互式会话的默认值。
EFFORT="${AI_EFFORT:-high}"

usage() {
  cat >&2 <<'EOF'
用法：scripts/release-notes.sh [version] [options]

根据上一个 tag 到当前 HEAD 的提交，以及工作区中未提交/未跟踪的改动，
生成面向用户的中文发布说明，并可推送到 GitHub Release。

参数：
  version            目标版本（默认取 package.json 中的版本）
                     接受 0.2.0 和 v0.2.0 两种写法

选项：
  --from <tag>       上一个发布 tag（默认：最近可达的 tag）
  --to <ref>         结束 ref（默认：HEAD）
  --out <file>       输出文件（默认：release-notes/v<version>.md）
  --model <name>     传给 AI CLI 的模型
  --effort <level>   传给 AI CLI 的 effort（low|medium|high|xhigh|max，
                     默认 high；传 none 则不传该参数）
  --no-ai            不调用 AI，直接按提交标题归类
  --publish          执行 `gh release edit v<version> --notes-file <file>`
  --dry-run          只打印说明，不写文件
  -h, --help         显示帮助

环境变量：
  AI_CMD             自定义命令：从 stdin 读取 prompt，向 stdout 输出 Markdown。
                     默认使用 `claude -p`。
  AI_MODEL           同 --model。
  AI_EFFORT          同 --effort。

示例：
  scripts/release-notes.sh 0.2.0
  scripts/release-notes.sh 0.2.0 --publish
  make notes VERSION=0.2.0
  make notes VERSION=0.2.0 NO_AI=1
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --from)
      FROM_TAG="${2:-}"
      [[ -n "$FROM_TAG" ]] || { echo "--from 需要指定 tag" >&2; exit 1; }
      shift 2
      ;;
    --to)
      TO_REF="${2:-}"
      [[ -n "$TO_REF" ]] || { echo "--to 需要指定 ref" >&2; exit 1; }
      shift 2
      ;;
    --out)
      OUT="${2:-}"
      [[ -n "$OUT" ]] || { echo "--out 需要指定文件" >&2; exit 1; }
      shift 2
      ;;
    --model)
      MODEL="${2:-}"
      [[ -n "$MODEL" ]] || { echo "--model 需要指定名称" >&2; exit 1; }
      shift 2
      ;;
    --effort)
      EFFORT="${2:-}"
      [[ -n "$EFFORT" ]] || { echo "--effort 需要指定级别" >&2; exit 1; }
      shift 2
      ;;
    --no-ai) USE_AI=0; shift ;;
    --publish) PUBLISH=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help) usage; exit 0 ;;
    -*)
      echo "未知选项：$1" >&2
      usage
      exit 1
      ;;
    *)
      if [[ -n "$VERSION" ]]; then
        echo "多余的参数：$1" >&2
        usage
        exit 1
      fi
      VERSION="$1"
      shift
      ;;
  esac
done

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "当前目录不是 git 仓库。" >&2
  exit 1
fi

if [[ -z "$VERSION" ]]; then
  VERSION="$(node -p "require('./package.json').version")"
fi
VERSION="${VERSION#v}"

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
  echo "版本号不合法：$VERSION（应形如 0.2.0）" >&2
  exit 1
fi

case "$EFFORT" in
  ""|none|low|medium|high|xhigh|max) ;;
  *)
    echo "effort 不合法：${EFFORT}（可选 low|medium|high|xhigh|max|none）" >&2
    exit 1
    ;;
esac

HAS_HEAD=1
if ! git rev-parse --verify --quiet "HEAD^{commit}" >/dev/null; then
  HAS_HEAD=0
fi

if [[ "$TO_REF" != "HEAD" || "$HAS_HEAD" == "1" ]]; then
  if ! git rev-parse --verify --quiet "$TO_REF^{commit}" >/dev/null; then
    echo "未知的 ref：$TO_REF" >&2
    exit 1
  fi
fi

if [[ -z "$FROM_TAG" && "$HAS_HEAD" == "1" ]]; then
  FROM_TAG="$(git describe --tags --abbrev=0 "$TO_REF" 2>/dev/null || true)"
  # 当前版本已经打过 tag 时，跟前一个 tag 比较，方便重新生成说明。
  if [[ -n "$FROM_TAG" && "${FROM_TAG#v}" == "$VERSION" ]]; then
    FROM_TAG="$(git describe --tags --abbrev=0 "${FROM_TAG}^" 2>/dev/null || true)"
  fi
fi

if [[ -n "$FROM_TAG" ]]; then
  if ! git rev-parse --verify --quiet "$FROM_TAG^{commit}" >/dev/null; then
    echo "未知的上一个 tag：$FROM_TAG" >&2
    exit 1
  fi
  RANGE="$FROM_TAG..$TO_REF"
elif [[ "$HAS_HEAD" == "1" ]]; then
  FROM_TAG="$(git rev-list --max-parents=0 "$TO_REF" | tail -1)"
  RANGE="$FROM_TAG..$TO_REF"
  FROM_TAG=""
else
  RANGE=""
fi

COMMIT_COUNT=0
if [[ -n "$RANGE" ]]; then
  COMMIT_COUNT="$(git rev-list --count "$RANGE")"
fi

# `make release` 之后工作区通常还没提交，因此把工作区改动也算进本次发布，
# 但排除版本号与 release notes 自身的文件。
EXCLUDES=(
  ':(exclude)package.json'
  ':(exclude)package-lock.json'
  ':(exclude)Cargo.toml'
  ':(exclude)Cargo.lock'
  ':(exclude)src-tauri/tauri.conf.json'
  ':(exclude)release-notes'
)
DIFF_BASE="HEAD"
if [[ "$HAS_HEAD" == "0" ]]; then
  # 新仓库还没有第一个提交时，用暂存区作为比较基线。
  DIFF_BASE="--cached"
fi
UNCOMMITTED_STAT="$(git diff --shortstat $DIFF_BASE -- . "${EXCLUDES[@]}" 2>/dev/null || true)"
UNCOMMITTED_FILES_RAW="$(git diff --name-status $DIFF_BASE -- . "${EXCLUDES[@]}" 2>/dev/null || true)"
UNTRACKED_FILES_RAW="$(git ls-files --others --exclude-standard -- . "${EXCLUDES[@]}" 2>/dev/null || true)"
UNCOMMITTED=0
if [[ -n "$UNCOMMITTED_FILES_RAW" || -n "$UNTRACKED_FILES_RAW" ]]; then
  UNCOMMITTED=1
fi

if [[ "$COMMIT_COUNT" == "0" && "$UNCOMMITTED" == "0" ]]; then
  echo "范围 $RANGE 内没有提交，工作区也没有改动；没有可发布的内容。" >&2
  exit 1
fi

if [[ -z "$OUT" ]]; then
  OUT="release-notes/v$VERSION.md"
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/costline-release-notes.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT

COMMIT_LOG="$TMP_DIR/commits.txt"
CHANGED_FILES="$TMP_DIR/files.txt"
UNCOMMITTED_FILES="$TMP_DIR/uncommitted-files.txt"
UNCOMMITTED_DIFF="$TMP_DIR/uncommitted-diff.txt"
PROMPT_FILE="$TMP_DIR/prompt.md"
AI_OUTPUT_FILE="$TMP_DIR/ai.md"
: > "$COMMIT_LOG"
: > "$CHANGED_FILES"
: > "$UNCOMMITTED_FILES"
: > "$UNCOMMITTED_DIFF"

if [[ -n "$RANGE" ]]; then
  git log --no-merges --pretty=format:'- %s%n%b' "$RANGE" | sed -n '1,400p' > "$COMMIT_LOG"
  git diff --name-status "$RANGE" | sed -n '1,200p' >> "$CHANGED_FILES"
fi
if [[ "$UNCOMMITTED" == "1" ]]; then
  {
    printf '%s\n' "$UNCOMMITTED_FILES_RAW"
    if [[ -n "$UNTRACKED_FILES_RAW" ]]; then
      printf '%s\n' "$UNTRACKED_FILES_RAW" | sed 's/^/? /'
    fi
  } | sed '/^$/d' | sed -n '1,200p' > "$UNCOMMITTED_FILES"
  git diff $DIFF_BASE -- . "${EXCLUDES[@]}" | sed -n '1,800p' > "$UNCOMMITTED_DIFF"
fi

DIFF_STAT=""
if [[ -n "$RANGE" ]]; then
  DIFF_STAT="$(git diff --shortstat "$RANGE" 2>/dev/null || true)"
fi

{
  cat <<'EOF'
你是 Costline 的发布负责人。Costline 是一个常驻 macOS 菜单栏的盯盘工具，
使用 Tauri 2 + React + Rust 构建；用户配置股票、基金、黄金的持仓成本，
当现价越过成本线时通过 Bark、Telegram、企业微信等渠道提醒。

请为本次新版本编写面向用户的简体中文发布说明。

## 发布上下文
EOF

  printf -- '- 新版本：v%s\n' "$VERSION"
  if [[ -n "$FROM_TAG" ]]; then
    printf -- '- 上一个版本：%s\n' "$FROM_TAG"
  else
    printf -- '- 上一个版本：无（首个版本）\n'
  fi
  printf -- '- 范围内的提交数：%s\n' "$COMMIT_COUNT"
  if [[ "$UNCOMMITTED" == "1" ]]; then
    printf -- '- 工作区存在未提交改动：是（属于本次发布）\n'
    if [[ -n "$UNCOMMITTED_STAT" ]]; then
      printf -- '- 未提交改动摘要：%s\n' "$UNCOMMITTED_STAT"
    fi
  fi
  if [[ -n "$DIFF_STAT" ]]; then
    printf -- '- 改动摘要：%s\n' "$DIFF_STAT"
  fi

  cat <<'EOF'

## 源码结构
先把改动文件对应到用户可见的模块，再写说明：
- src/App.tsx, src/pages/Dashboard.tsx, src/components/AssetModal.tsx: 盯盘面板、持仓资产的添加/编辑/删除
- src/pages/Settings.tsx, src/components/ChannelModal.tsx, src/components/UpdateSection.tsx, src/updater.ts: 设置、通知渠道、应用内更新
- src/components/Disguise.tsx, src/components/DisguiseSection.tsx, src/disguise.ts: 伪装模式（数据换皮 / 全屏伪装）
- src/api.ts, src/types.ts, src/format.ts, src/drafts.ts: 前端数据、格式化与草稿
- src-tauri/src/tray.rs, src-tauri/src/commands.rs, src-tauri/src/desktop.rs: 托盘菜单、桌面端命令
- src-tauri/src/watcher.rs, src-tauri/src/state.rs: 后台盯盘循环与状态
- crates/costline-core/src/market.rs: 行情抓取与价格解析（A 股、港股、美股、基金、黄金）
- crates/costline-core/src/monitor.rs: 回本判断、冷却时间与重复提醒
- crates/costline-core/src/notify.rs: Bark、Server 酱、PushPlus、Telegram、企业微信、钉钉、飞书、ntfy、Gotify、Webhook
- crates/costline-core/src/config.rs, crates/costline-core/src/state.rs: 配置与提醒状态的读写
- cli/src/: 命令行版
- src/styles.css: 全局视觉样式

## 提交
EOF

  cat "$COMMIT_LOG"

  cat <<'EOF'

## 改动文件
EOF

  cat "$CHANGED_FILES"

  if [[ "$UNCOMMITTED" == "1" ]]; then
    cat <<'EOF'

## 未提交改动（属于本次发布）
工作区还有未提交的改动，请把它们和上面的提交一起视为本次发布内容。

### 文件
EOF
    cat "$UNCOMMITTED_FILES"
    cat <<'EOF'

### Diff（已截断）
```
EOF
    cat "$UNCOMMITTED_DIFF"
    cat <<'EOF'
```
EOF
  fi

  cat <<'EOF'

## 输出要求
1. 只输出 Markdown 正文，不要用代码块包裹，也不要写一级版本标题；调用方会补版本头。
2. 只在有内容时使用这些小节：
   ### 新增功能
   ### 体验优化
   ### 问题修复
   ### 其他变更
3. 每条以 "- " 开头，描述用户能感知到的变化。不要出现文件路径、函数名、commit hash 或内部类型名。
4. 合并重复或琐碎的提交。测试、CI、依赖升级、重构归入「其他变更」；用户感知不到时可以省略。
5. 不要编造提交中不存在的功能。不确定时使用保守描述。
6. 简洁、客观，避免营销式措辞。
EOF
} > "$PROMPT_FILE"

ai_available() {
  if [[ -n "${AI_CMD:-}" ]]; then
    return 0
  fi
  command -v claude >/dev/null 2>&1
}

run_ai() {
  if [[ -n "${AI_CMD:-}" ]]; then
    # shellcheck disable=SC2086
    eval "$AI_CMD" < "$PROMPT_FILE"
    return
  fi

  local args=(-p --tools "" --output-format text)
  if [[ -n "$MODEL" ]]; then
    args+=(--model "$MODEL")
  fi
  if [[ -n "$EFFORT" && "$EFFORT" != "none" ]]; then
    args+=(--effort "$EFFORT")
  fi
  claude "${args[@]}" < "$PROMPT_FILE"
}

fallback_notes() {
  # 没有 AI CLI 时，按提交标题简单归类。
  local added="$TMP_DIR/added.txt"
  local fixed="$TMP_DIR/fixed.txt"
  local improved="$TMP_DIR/improved.txt"
  local other="$TMP_DIR/other.txt"
  : > "$added"; : > "$fixed"; : > "$improved"; : > "$other"

  local subject lower
  if [[ -n "$RANGE" ]]; then
    while IFS= read -r subject || [[ -n "$subject" ]]; do
      [[ -n "$subject" ]] || continue
      subject="$(printf '%s' "$subject" | sed -E 's/^[a-zA-Z]+(\([^)]*\))?!?:[[:space:]]*//')"
      lower="$(printf '%s' "$subject" | tr '[:upper:]' '[:lower:]')"
      case "$lower" in
        *feat*|*add*|*support*|*新增*|*添加*|*支持*) printf -- '- %s\n' "$subject" >> "$added" ;;
        *fix*|*bug*|*修复*|*修正*) printf -- '- %s\n' "$subject" >> "$fixed" ;;
        *perf*|*optimi*|*improve*|*refactor*|*优化*|*改进*|*提升*|*重构*) printf -- '- %s\n' "$subject" >> "$improved" ;;
        *) printf -- '- %s\n' "$subject" >> "$other" ;;
      esac
    done < <(git log --no-merges --pretty=format:'%s' "$RANGE")
  fi

  if [[ -s "$added" ]]; then echo "### 新增功能"; sort -u "$added"; echo; fi
  if [[ -s "$improved" ]]; then echo "### 体验优化"; sort -u "$improved"; echo; fi
  if [[ -s "$fixed" ]]; then echo "### 问题修复"; sort -u "$fixed"; echo; fi
  if [[ -s "$other" ]]; then echo "### 其他变更"; sort -u "$other"; echo; fi
}

BODY=""
if [[ "$USE_AI" == "1" ]] && ai_available; then
  echo "正在使用 AI 生成发布说明..." >&2
  if run_ai > "$AI_OUTPUT_FILE" 2>"$TMP_DIR/ai.err"; then
    BODY="$(sed -E -e '/^[[:space:]]*```/d' -e '/^##[[:space:]]+Costline/d' "$AI_OUTPUT_FILE")"
  else
    echo "AI 生成失败，改用提交标题生成。" >&2
    sed 's/^/  /' "$TMP_DIR/ai.err" >&2 || true
  fi
else
  if [[ "$USE_AI" == "1" ]]; then
    echo "未找到 AI CLI（可设置 AI_CMD 或安装 claude），改用提交标题生成。" >&2
  fi
fi

if [[ -z "${BODY//[[:space:]]/}" ]]; then
  BODY="$(fallback_notes)"
fi

if [[ -z "${BODY//[[:space:]]/}" ]]; then
  BODY="### 其他变更
- 本次发布包含若干功能与体验调整。"
fi

{
  printf '## Costline v%s\n\n' "$VERSION"
  if [[ -n "$FROM_TAG" ]]; then
    if [[ "$UNCOMMITTED" == "1" && "$COMMIT_COUNT" == "0" ]]; then
      printf '> 相对 `%s`，本次发布包含未提交的工作区改动。\n\n' "$FROM_TAG"
    elif [[ "$UNCOMMITTED" == "1" ]]; then
      printf '> 相对 `%s`，本次共 %s 个提交，并包含未提交的工作区改动。\n\n' "$FROM_TAG" "$COMMIT_COUNT"
    else
      printf '> 相对 `%s`，本次共 %s 个提交。\n\n' "$FROM_TAG" "$COMMIT_COUNT"
    fi
  else
    if [[ "$UNCOMMITTED" == "1" && "$COMMIT_COUNT" == "0" ]]; then
      printf '> 首个版本，包含未提交的工作区改动。\n\n'
    elif [[ "$UNCOMMITTED" == "1" ]]; then
      printf '> 首个版本，共 %s 个提交，并包含未提交的工作区改动。\n\n' "$COMMIT_COUNT"
    else
      printf '> 首个版本，共 %s 个提交。\n\n' "$COMMIT_COUNT"
    fi
  fi

  printf '%s\n\n' "$BODY"

  cat <<EOF
---

### 下载
- \`Costline_${VERSION}_universal.dmg\` — macOS（Universal，Apple Silicon + Intel）

> macOS 安装包未使用 Apple Developer 证书签名或公证。首次打开如提示
> “无法验证开发者”，请到「系统设置 → 隐私与安全性」点击「仍要打开」。
> 应用内自动更新会校验 Tauri updater 签名，不依赖 Apple 签名。
EOF
} > "$TMP_DIR/notes.md"

if [[ "$DRY_RUN" == "1" ]]; then
  cat "$TMP_DIR/notes.md"
  exit 0
fi

mkdir -p "$(dirname "$OUT")"
cp "$TMP_DIR/notes.md" "$OUT"
echo "发布说明已写入 $OUT"

if [[ "$PUBLISH" == "1" ]]; then
  TAG="v$VERSION"
  command -v gh >/dev/null 2>&1 || {
    echo "未安装 gh CLI，无法推送。" >&2
    exit 1
  }
  if ! gh release view "$TAG" >/dev/null 2>&1; then
    echo "GitHub Release $TAG 还不存在；请先创建并推送 tag，然后运行：" >&2
    echo "  gh release edit $TAG --notes-file $OUT" >&2
    exit 1
  fi
  gh release edit "$TAG" --notes-file "$OUT"
  echo "发布说明已推送到 GitHub Release $TAG"
fi
