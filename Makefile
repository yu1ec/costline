# Costline — macOS 托盘盯盘工具
APP_NAME     := Costline
APP_DIR      := src-tauri
BIN_NAME     := costline-desktop
TARGET_DIR   := target
CARGO        ?= cargo
NPM          ?= npm
# 统一走项目内的 @tauri-apps/cli，首次 npm install 后即可使用。
TAURI        ?= $(NPM) run tauri --
BUNDLES      ?= app,dmg
# SIGN=0（默认）使用 --no-sign；SIGN=1 交给 Tauri 自动选择签名身份
SIGN         ?= 0
SIGN_IDENTITY ?=

# create-dmg 的 Finder 美化步骤依赖 AppleScript，部分机器会卡在等待 .DS_Store。
# 默认设置 CI=1，让 Tauri 传 --skip-jenkins，产出稳定可用但不带自定义窗口布局。
# 需要 Finder 里的图标位置/背景时用 make build DMG_LAYOUT=1。
DMG_LAYOUT ?= 0

# 版本号以 package.json 为准，scripts/set-version.mjs 会同步到 Cargo.toml / tauri.conf.json。
VERSION      := $(shell node -p "require('./package.json').version" 2>/dev/null)
REPO         ?= yu1ec/costline
RELEASE_TAG  ?= v$(VERSION)

# 自动更新签名私钥（不要提交到仓库），默认放在用户目录。
SIGN_KEY          ?= $(HOME)/.tauri/costline.key
SIGN_KEY_PASSWORD ?=

# UNIVERSAL=1 时构建 aarch64 + x86_64 通用包；也可以用 TARGET=... 指定单个目标。
UNIVERSAL ?= 0
ifeq ($(UNIVERSAL),1)
TARGET := universal-apple-darwin
endif

ifneq ($(strip $(TARGET)),)
TARGET_ARGS := --target $(TARGET)
RELEASE_DIR := $(TARGET_DIR)/$(TARGET)/release
else
TARGET_ARGS :=
RELEASE_DIR := $(TARGET_DIR)/release
endif

UNIVERSAL_FLAG := $(if $(filter 1,$(UNIVERSAL)),--universal,)

.PHONY: help init init-rust init-frontend init-tauri icons build build-app build-binary build-frontend run run-cli dev dev-app test lint fmt fmt-check check ci cli cli-build clean open-app dist package release notes bump-version check-version latest-json updater-key version

.DEFAULT_GOAL := help

help:
	@echo "$(APP_NAME) — Makefile"
	@echo ""
	@echo "常用命令："
	@echo "  make init        安装前端依赖（含 tauri-cli），并预取 Rust 依赖"
	@echo "  make dev         启动 Tauri 开发模式（热更新）"
	@echo "  make build       构建 macOS 应用与 dmg（默认 --no-sign）"
	@echo "  make build-binary  只构建可执行文件（不打包 dmg）"
	@echo "  make run         构建并直接运行可执行文件"
	@echo "  make open-app    打开构建好的 .app"
	@echo "  make test        运行 Rust 单元测试"
	@echo "  make lint        cargo clippy（-D warnings）"
	@echo "  make fmt         格式化 Rust 代码"
	@echo "  make fmt-check   检查 Rust 代码格式（CI 用）"
	@echo "  make check       前后端一起做编译检查"
	@echo "  make ci          本地跑一遍 CI：fmt-check + lint + test + 前端构建"
	@echo "  make cli         运行命令行版（ARGS 透传，如 make cli ARGS='check --json'）"
	@echo "  make icons       从 src-tauri/icons/app-icon.png 重新生成图标"
	@echo "  make clean       清理 target/ 与 dist/"
	@echo ""
	@echo "发布流程："
	@echo "  make release TYPE=patch|minor|major  自动升版本、生成 release-notes，并提示 git 命令"
	@echo "  make release TYPE=minor DRY_RUN=1    只预览版本变化，不修改文件"
	@echo "  make notes VERSION=x.y.z            单独生成发布说明（AI 辅助）"
	@echo "  make notes VERSION=x.y.z NO_AI=1    不调用 AI，按提交标题归类"
	@echo "  make notes VERSION=x.y.z PUBLISH=1  推送到已存在的 GitHub Release"
	@echo "  make bump-version VERSION=x.y.z     把指定版本同步到所有版本文件"
	@echo "  make check-version TAG=vx.y.z       校验各处版本与 tag 一致（CI 用）"
	@echo ""
	@echo "本地打包 / 自动更新："
	@echo "  make updater-key 生成自动更新签名密钥（默认 ~/.tauri/costline.key）"
	@echo "  make package     打包带更新签名的 .app/.dmg/.app.tar.gz，并生成 latest.json"
	@echo "  make package UNIVERSAL=1    打包 macOS 通用二进制（aarch64 + x86_64）"
	@echo "  make latest-json 根据已有产物重新生成 latest.json"
	@echo ""
	@echo "别名（mole-desktop 风格）：build-app / build-cli / dev-app / run-cli"
	@echo ""
	@echo "可覆盖变量："
	@echo "  TYPE          发布类型：patch | minor | major（make release 用）"
	@echo "  DRY_RUN       1=只预览，不修改文件"
	@echo "  NO_AI         1=不调用 AI 生成发布说明"
	@echo "  PUBLISH       1=把发布说明推送到已存在的 GitHub Release"
	@echo "  FROM          生成说明时的上一个 tag（默认最近可达 tag）"
	@echo "  EFFORT        AI effort：low | medium | high | xhigh | max（默认 high）"
	@echo "  TAG           make check-version 校验的 tag（默认当前 git tag）"
	@echo "  BUNDLES       打包类型（默认 app,dmg，可设为 app 只出 .app）"
	@echo "  SIGN          1=允许 Tauri 签名，0=--no-sign（默认 0）"
	@echo "  SIGN_IDENTITY macOS 签名证书 CN"
	@echo "  DMG_LAYOUT   1=使用 Finder 自定义 dmg 布局（默认 0，跳过以免卡住）"
	@echo "  TAURI_SIGNING_PRIVATE_KEY / TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
	@echo "                更新签名私钥；未设置时 make package 读取 SIGN_KEY"
	@echo "  REPO          GitHub 仓库（默认 $(REPO)）"
	@echo "  RELEASE_TAG   更新包下载地址使用的 tag（默认 v$(VERSION)）"
	@echo "  ARGS          传给 make cli 的参数"
	@echo ""
	@echo "示例："
	@echo "  make init"
	@echo "  make dev"
	@echo "  make release TYPE=minor"
	@echo "  make release TYPE=patch DRY_RUN=1"
	@echo "  make build BUNDLES=app"
	@echo "  make build SIGN=1 SIGN_IDENTITY='Developer ID Application: ...'"
	@echo "  make bump-version VERSION=0.2.0 && make package UNIVERSAL=1"
	@echo "  ARGS='check --json' make cli"

init: init-frontend init-rust
	@echo ""
	@echo "==> 完成。运行 make dev 启动开发模式，或 make build 打包。"

init-rust:
	@echo "==> 预取 Rust 依赖..."
	$(CARGO) fetch

init-frontend:
	@echo "==> 安装前端依赖..."
	$(NPM) install
	@$(TAURI) --version >/dev/null 2>&1 || { \
		echo "tauri-cli 不可用，请检查 @tauri-apps/cli 是否安装成功" >&2; exit 1; \
	}

init-tauri:
	@$(TAURI) --version >/dev/null 2>&1 || { \
		echo "tauri-cli 不可用，请先运行 make init-frontend" >&2; exit 1; \
	}

icons:
	@echo "==> 从 $(APP_DIR)/icons/app-icon.png 生成图标..."
	$(TAURI) icon $(APP_DIR)/icons/app-icon.png
	@rm -rf $(APP_DIR)/icons/android $(APP_DIR)/icons/ios

dev: init-frontend
	$(TAURI) dev

build: init-frontend
	@sign_args=""; \
	if [ "$(SIGN)" != "1" ]; then sign_args="--no-sign"; fi; \
	if [ -n "$(SIGN_IDENTITY)" ]; then export APPLE_SIGNING_IDENTITY="$(SIGN_IDENTITY)"; fi; \
	if [ "$(DMG_LAYOUT)" = "1" ]; then unset CI; else export CI=true; fi; \
	echo "==> 打包 $(APP_NAME)（bundles: $(BUNDLES)）"; \
	$(TAURI) build $(TARGET_ARGS) --bundles $(BUNDLES) $$sign_args; \
	echo ""; \
	echo "==> 产物："; \
	find "$(RELEASE_DIR)/bundle" -maxdepth 4 \( \
		-type f \( -name "*.dmg" -o -name "*.app.tar.gz" \) \
		-o -type d -name "*.app" \
	\) 2>/dev/null | sort | sed 's/^/    /'

# 前端资源在编译时嵌入，所以先构建 dist/。
build-frontend: init-frontend
	$(NPM) run build

build-binary: build-frontend
	@echo "==> 构建可执行文件..."
	$(CARGO) build --release -p costline-desktop
	@echo "==> 输出：$(TARGET_DIR)/release/$(BIN_NAME)"

run: build-binary
	@echo "==> 运行 $(TARGET_DIR)/release/$(BIN_NAME)"
	@"$(TARGET_DIR)/release/$(BIN_NAME)"

open-app:
	@if [ -d "$(RELEASE_DIR)/bundle/macos/$(APP_NAME).app" ]; then \
		open "$(RELEASE_DIR)/bundle/macos/$(APP_NAME).app"; \
	else \
		echo "未找到 .app，请先运行 make build" >&2; exit 1; \
	fi

test:
	$(CARGO) test --workspace

lint:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

check:
	$(CARGO) check --workspace
	$(NPM) run build

ci: fmt-check lint test build-frontend
	@echo "==> CI 检查全部通过"

cli:
	$(CARGO) run -p costline-cli -- $(ARGS)

cli-build:
	$(CARGO) build --release -p costline-cli
	@echo "==> 输出：$(TARGET_DIR)/release/costline"

# 生成自动更新签名密钥；私钥不要提交，公钥填到 src-tauri/tauri.conf.json。
updater-key:
	@echo "==> 生成更新签名密钥：$(SIGN_KEY)"
	@mkdir -p "$(dir $(SIGN_KEY))"
	$(TAURI) signer generate -w "$(SIGN_KEY)" -p "$(SIGN_KEY_PASSWORD)" --ci
	@echo ""
	@echo "==> 公钥（填到 $(APP_DIR)/tauri.conf.json 的 plugins.updater.pubkey）："
	@cat "$(SIGN_KEY).pub"
	@echo ""
	@echo "==> 把私钥内容配置为 GitHub Secret：TAURI_SIGNING_PRIVATE_KEY"
	@echo "    如果设置了密码，再配置 TAURI_SIGNING_PRIVATE_KEY_PASSWORD"

# 同步版本号：package.json、package-lock.json、workspace Cargo.toml、Cargo.lock、tauri.conf.json
version:
	@case "$(VERSION)" in \
		[0-9]*.[0-9]*.[0-9]*) ;; \
		*) echo "版本号格式应为 x.y.z，例如 make version VERSION=0.2.0" >&2; exit 1 ;; \
	esac
	@node scripts/set-version.mjs "$(VERSION)"

bump-version:
	@test -n "$(VERSION)" || (echo "用法：make bump-version VERSION=0.2.0" >&2; exit 1)
	@bash scripts/bump-version.sh "$(VERSION)"

check-version:
	@bash scripts/check-version.sh "$(TAG)"

# 发布流程：升级版本 + 生成 release notes + 提示后续 git 命令（不打包）。
release:
	@test -n "$(TYPE)" || (echo "用法：make release TYPE=patch|minor|major" >&2; exit 1)
	@if [ "$(DRY_RUN)" = "1" ]; then \
		bash scripts/release.sh "$(TYPE)" --dry-run; \
	else \
		bash scripts/release.sh "$(TYPE)"; \
	fi

# 单独生成 / 重新生成发布说明。
notes:
	@args=""; \
	if [ -n "$(VERSION)" ]; then args="$$args $(VERSION)"; fi; \
	if [ -n "$(FROM)" ]; then args="$$args --from $(FROM)"; fi; \
	if [ -n "$(EFFORT)" ]; then args="$$args --effort $(EFFORT)"; fi; \
	if [ "$(NO_AI)" = "1" ]; then args="$$args --no-ai"; fi; \
	if [ "$(PUBLISH)" = "1" ]; then args="$$args --publish"; fi; \
	if [ "$(DRY_RUN)" = "1" ]; then args="$$args --dry-run"; fi; \
	bash scripts/release-notes.sh $$args

# 本地打包：开启 createUpdaterArtifacts，产出 .app.tar.gz/.sig，并生成 latest.json。
package: init-frontend
	@set -e; \
	if [ "$(UNIVERSAL)" = "1" ]; then \
		echo "==> 安装 universal 编译目标..."; \
		rustup target add aarch64-apple-darwin x86_64-apple-darwin; \
	fi; \
	key="$${TAURI_SIGNING_PRIVATE_KEY:-$(SIGN_KEY)}"; \
	if [ -z "$$TAURI_SIGNING_PRIVATE_KEY" ] && [ ! -f "$$key" ]; then \
		echo "未找到签名私钥：$$key" >&2; \
		echo "先运行 make updater-key，或设置 TAURI_SIGNING_PRIVATE_KEY" >&2; \
		exit 1; \
	fi; \
	if [ -n "$(SIGN_IDENTITY)" ]; then export APPLE_SIGNING_IDENTITY="$(SIGN_IDENTITY)"; fi; \
	if [ "$(DMG_LAYOUT)" = "1" ]; then unset CI; else export CI=true; fi; \
	echo "==> 打包 $(APP_NAME) v$(VERSION)（tag: $(RELEASE_TAG)）"; \
	TAURI_SIGNING_PRIVATE_KEY="$$key" \
	TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-$(SIGN_KEY_PASSWORD)}" \
	$(TAURI) build --config $(APP_DIR)/tauri.release.conf.json $(TARGET_ARGS) --bundles $(BUNDLES); \
	node scripts/latest-json.mjs \
		--bundle-dir "$(RELEASE_DIR)/bundle" \
		--version "$(VERSION)" \
		--repo "$(REPO)" \
		--tag "$(RELEASE_TAG)" \
		$(UNIVERSAL_FLAG); \
	echo ""; \
	echo "==> 发布产物："; \
	find "$(RELEASE_DIR)/bundle" -maxdepth 4 \( \
		-type f \( -name "*.dmg" -o -name "*.app.tar.gz" -o -name "*.sig" -o -name "latest.json" \) \
		-o -type d -name "*.app" \
	\) 2>/dev/null | sort | sed 's/^/    /'

latest-json:
	@node scripts/latest-json.mjs \
		--bundle-dir "$(RELEASE_DIR)/bundle" \
		--version "$(VERSION)" \
		--repo "$(REPO)" \
		--tag "$(RELEASE_TAG)" \
		$(UNIVERSAL_FLAG)

# mole-desktop 风格的别名，方便沿用同一套命令习惯。
build-app: build
build-cli: build-binary
dev-app: dev
run-cli: run

clean:
	$(CARGO) clean
	rm -rf dist
