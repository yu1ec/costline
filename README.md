# costline

一个常驻 macOS 菜单栏的盯盘工具：配置股票、基金或黄金的持仓成本，当现价越过成本线（回本）时，通过免费通知服务或自建开源工具提醒你。

同一套核心逻辑提供两种形态：

- **托盘应用（推荐）**：Tauri 2 + React，菜单栏常驻，图形界面里添加资产和通知渠道。
- **命令行版**：适合放进 launchd 或没有图形界面的场景。

## 功能

- 支持 A 股、港股、美股、ETF、场外基金、上海黄金、国际金价
- 黄金、黄金基金、黄金股票会同时显示国内金价（上海金 Au99.99，元/克）
- 成本价既可以直接填，也可以填「本金 ÷ 数量」自动计算
- 支持导入券商交割单（CSV / TXT），卖出后按摊薄成本自动扣减本金，无需手动改配置
- 股票、基金、黄金都可以配置定投提醒：每期金额、每周 / 每月、定投日与提醒时间
- 全局回本阈值，或单个资产单独阈值；也可以直接指定目标价
- 回本提醒只在「从下方向上穿越」时触发，避免每轮刷屏
- 支持冷却时间，以及持续高于成本线时的重复提醒
- 通知渠道：Bark、Server 酱、PushPlus、Telegram、企业微信、钉钉、飞书、ntfy、Gotify、通用 Webhook
- 桌面版触发时始终发送 macOS 系统通知，并在应用窗口内显示提醒；远程渠道可以同时配置
- 消息历史：回本提醒、定投提醒和测试消息都会记录推送时间、内容与各渠道结果，随时回看
- 配置与提醒状态都落在本地文件，无数据库、无云端账号
- 内置伪装模式：开发、运营、产品、财务四种工作场景，支持数据换皮与全屏伪装
- 内置自动更新：启动时检查 GitHub Releases，发现新版本后可在应用内下载、校验签名并重启

## 界面

### 盯盘面板

- 顶部汇总：资产数量、已回本数量、最近检查时间
- 实时表格：名称、代码、现价、成本、触发价、日涨跌、浮动盈亏、备注
- 现价高于触发价的资产会高亮
- 「实时价格」开关：开启后每 10 秒单独刷新一次股票、基金、黄金行情；暂停盯盘时也能继续看价格
- 开启「实时价格」后，现价下方会同步显示实时涨跌额和涨跌幅（相对昨收），随刷新一起更新
- 手动「检查」、暂停 / 恢复自动盯盘

### 消息历史

- 顶栏「消息」入口，未读消息会显示角标
- 按「回本提醒 / 定投提醒 / 测试消息」筛选，按日期分组
- 每条记录展示推送时间、完整内容，以及系统通知和每个远程渠道的发送结果；失败的渠道可悬停查看原因
- 定投提醒弹层里的「查看」会直接跳到对应的历史消息
- 最多保留最近 300 条，可在页面内一键清空

### 设置

- 基础参数：轮询间隔、请求超时、全局回本阈值、通知冷却、重复提醒
- 持仓资产：添加 / 编辑 / 删除股票、基金、黄金
- 通知渠道：添加 / 编辑 / 删除远程渠道；系统通知默认开启，可额外配置提示音
- 重置提醒状态、打开配置目录

### 托盘

- 左键点击图标打开面板
- 右键菜单：打开面板、立即检查、暂停 / 恢复盯盘、退出
- 关闭窗口只是隐藏，进程继续在后台盯盘

### 伪装模式

设置页可以选择「开发 / 运营 / 产品 / 财务」四种身份，进入方式有两种：

- **数据换皮**：保留真实行情数字，但把资产名称、代码、表头和状态替换成对应场景的文案
- **全屏伪装**：整页替换成 IDE、运营看板、PRD 文档或预算表，列表里仍是真实行情数字，只把名称、表头和状态换成了对应场景的说法

快捷键（窗口聚焦时）：

| 操作 | macOS | Windows / Linux |
| --- | --- | --- |
| 全屏伪装 | `⌘ ⇧ H` | `Ctrl + Shift + H` |
| 数据换皮 | `⌘ ⇧ M` | `Ctrl + Shift + M` |
| 退出伪装 | `Esc` 或连点三次左上角标识 | 同左 |

伪装里展示的仍然是真实数据：开发身份的「服务健康」、运营的「渠道明细」、产品的「需求进度」、财务的「预算执行表」都按你的持仓逐行生成，只是换了名称和列名。

想确认某一行对应哪只资产，有两种办法：

- 伪装中按住 `⌥`（Windows / Linux 为 `Alt`），名称会临时切回真实资产名，松开即恢复
- 设置页「伪装模式 → 数据对照」会列出当前身份下每个名称对应的真实资产和价格

伪装只改变界面内容和窗口标题，后台盯盘、系统通知与远程推送都不受影响。开启「启动时自动进入全屏伪装」后，每次启动应用会先显示伪装界面，再用快捷键切回盯盘。

## 环境要求

- macOS 11 或更高版本
- Xcode Command Line Tools：`xcode-select --install`
- Rust（stable）
- Node.js 18+
- `tauri-cli`（`make init` 会自动检查并按需安装）

## 快速开始

```bash
# 1. 安装前端依赖、检查 tauri-cli、预取 Rust 依赖
make init

# 2. 开发模式（前端热更新 + Tauri 窗口）
make dev

# 3. 打包 .app 与 .dmg（默认不签名）
make build

# 4. 打开打包好的应用
make open-app
```

打包产物：

```
target/release/bundle/macos/Costline.app
target/release/bundle/dmg/Costline_0.1.0_aarch64.dmg
```

首次启动时主窗口默认隐藏，点击菜单栏里的 Costline 图标即可打开面板。

### 常用 make 命令

```bash
make help             # 查看全部命令
make init             # 安装依赖 + 预取 Rust crate
make dev              # Tauri 开发模式
make build            # 打包 .app 与 .dmg
make build-binary     # 只构建可执行文件
make run              # 构建并直接运行可执行文件
make open-app         # 打开已打包的 .app
make test             # cargo test --workspace
make lint             # cargo clippy --workspace --all-targets -- -D warnings
make check            # cargo check + 前端生产构建
make cli ARGS='check' # 运行命令行版
make release TYPE=minor  # 自动升版本并生成 release-notes/vX.Y.Z.md
make notes VERSION=0.2.0 # 单独生成或重新生成发布说明
make package          # 本地打包更新包并生成 latest.json
make clean            # 清理 target/ 与 dist/
```

可选变量：

```bash
make build BUNDLES=app                    # 只出 .app，不生成 dmg
make build DMG_LAYOUT=1                   # 使用 Finder 自定义 dmg 窗口布局
make build SIGN=1                         # 交给 Tauri 自动选择签名身份
make build SIGN=1 SIGN_IDENTITY='Developer ID Application: ...'
make release TYPE=patch DRY_RUN=1         # 只预览版本变化，不修改文件
make notes VERSION=0.2.0 NO_AI=1          # 不调用 AI，按提交标题归类
make notes VERSION=0.2.0 PUBLISH=1        # 推送到已存在的 GitHub Release
make package UNIVERSAL=1                  # 通用二进制
make cli ARGS='check --json'              # 透传参数给命令行版
```

## 自动更新与发布

桌面版内置 Tauri updater。启动约 4 秒后会在后台检查
`https://github.com/yu1ec/costline/releases/latest/download/latest.json`；
发现新版本时顶栏会出现「更新 vX.Y.Z」入口，也可以在「设置 → 软件更新」里手动检查、下载、安装并重启。
更新包必须带 Tauri 签名，应用会先校验 `plugins.updater.pubkey` 再安装。

### 一次性配置

1. 生成签名密钥（私钥留在本机，不要提交到仓库）：

```bash
make updater-key
# 私钥：~/.tauri/costline.key
# 公钥：~/.tauri/costline.key.pub
```

2. 把公钥填到 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`。
   当前仓库已经生成了一份，换机器或更换密钥时按需替换。

3. 在 GitHub 仓库 Settings → Secrets and variables → Actions 添加：

- `TAURI_SIGNING_PRIVATE_KEY`：`~/.tauri/costline.key` 的内容
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：生成密钥时设置的密码；没有密码可以留空

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/costline.key
```

### 发布新版本

```bash
make release TYPE=minor         # 升版本 + 生成 release-notes/v0.2.0.md
# 检查一下生成的发布说明，然后提交：
git add -A && git commit -m "chore: release v0.2.0"
git tag v0.2.0
git push origin master --tags
```

`TYPE` 支持 `patch` / `minor` / `major`，也会同步 `package.json`、`package-lock.json`、`Cargo.toml`、`Cargo.lock` 与 `src-tauri/tauri.conf.json`。
只想预览版本变化时用 `make release TYPE=minor DRY_RUN=1`。

推送 tag 后 `.github/workflows/release.yml` 会：

- 校验 tag 与各处版本文件一致
- 在 macOS 上构建 aarch64 + x86_64 通用包
- 用 `TAURI_SIGNING_PRIVATE_KEY` 给更新包签名
- 上传 `.dmg`、`.app.tar.gz`、`.sig` 与 `latest.json` 到 GitHub Release
- 创建 Release 后用 `release-notes/v<tag>.md` 覆盖默认说明

应用里的更新地址指向 `releases/latest/download/latest.json`，因此 Release 需要正式发布（不能只停留在 draft）。

本地也可以直接打包更新包：

```bash
make package                  # 当前架构
make package UNIVERSAL=1      # 通用二进制
```

产物在 `target/release/bundle`（或 `target/universal-apple-darwin/release/bundle`），包含
`macos/Costline.app/`、`macos/Costline.app.tar.gz`、`.sig` 与 `latest.json`。

> 未做 Apple Developer 签名/公证时，首次打开仍可能被 Gatekeeper 拦截；这不影响 Tauri updater 的签名校验。

### 发布说明（Release Notes）

`scripts/release-notes.sh` 会读取上一个 tag 到当前 HEAD 之间的提交，以及工作区里尚未提交/尚未跟踪的改动，生成面向用户的中文更新说明，写入 `release-notes/v<version>.md`：

```bash
make release TYPE=minor              # 升版本并自动生成 release-notes/v0.2.0.md
make notes VERSION=0.2.0             # 单独生成或重新生成
make notes VERSION=0.2.0 NO_AI=1     # 不调用 AI，直接按提交标题归类
make notes VERSION=0.2.0 PUBLISH=1   # 直接推送到已存在的 GitHub Release
make notes VERSION=0.2.0 EFFORT=medium  # 覆盖默认的 high effort
scripts/release-notes.sh 0.2.0 --dry-run   # 只打印，不写文件
```

默认调用 `claude -p --effort high` 生成说明——`--effort high` 只作用于这次的 AI 调用，不影响交互式会话的 effort 设置。可以用 `AI_EFFORT` / `--effort` 改级别（`low|medium|high|xhigh|max`，传 `none` 则不传该参数）；也可以用 `AI_CMD` 指定任意命令（从 stdin 读取 prompt，向 stdout 输出 Markdown），或用 `AI_MODEL` / `--model` 指定模型。AI 不可用时会退化为按提交标题生成。

生成的文件随 tag 一起提交，CI 在 Release 创建完成后会自动用 `release-notes/v<tag>.md` 覆盖默认正文。AI 生成的内容建议先人工检查再提交。

### CI

`.github/workflows/ci.yml` 在 pull request 时执行（master 直接 push 不会触发）：

- 前端：`npm ci && npm run build`
- Rust：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`

本地等价命令：`make ci`。

## 配置位置

托盘应用：

```
~/Library/Application Support/com.costline.desktop/config.toml
~/Library/Application Support/com.costline.desktop/state.json
```

命令行版：

```
~/Library/Application Support/costline/config.toml
~/Library/Application Support/costline/state.json
```

托盘应用首次启动时，如果检测到命令行版的配置和状态，会自动复制过来。

设置页底部会显示当前配置文件路径，也可以点「配置目录」直接在 Finder 中打开。

## 触发规则

```
触发价 = target_price
      或者 成本价 × (1 + threshold_percent / 100)

当 现价 >= 触发价 时触发提醒
```

- 默认 `threshold_percent = 0`，即价格回到成本价就提醒
- 设置 `threshold_percent = 2` 表示盈利 2% 才提醒
- `target_price` 优先级最高，适合直接指定一个价格
- 第一次运行时如果现价已经在触发价上方，会提醒一次
- 之后只有价格跌回下方再穿越上来时才会再次提醒
- `cooldown_secs` 控制任意两次提醒的最小间隔
- `repeat_interval_secs > 0` 时，价格持续在触发价上方也会按该间隔重复提醒
- 通知全部失败时状态会回滚，下一轮继续重试
- 删除 `state.json`（或点「重置提醒状态」）可以重置提醒状态

## 配置文件

图形界面编辑的就是这份 TOML，两者可以混用；改完文件后重启应用或重新保存一次设置即可生效。

```toml
interval_secs = 60
timeout_secs = 10
threshold_percent = 0.0

[notify]
cooldown_secs = 1800
repeat_interval_secs = 0

[[notify.channels]]
type = "bark"
key = "你的BarkKey"

[[assets]]
kind = "stock"
name = "浦发银行"
symbol = "sh600000"
cost = 10.00
quantity = 1000

[[assets]]
kind = "fund"
name = "招商中证白酒"
code = "161725"
cost = 0.5877
quantity = 10000

# 定投提醒（可选）：股票、基金、黄金都支持
[assets.sip]
enabled = true
amount = 500.00
frequency = "monthly"
day = 1
time = "09:30"

[[assets]]
kind = "gold"
name = "黄金9999"
secid = "118.AU9999"
cost = 800.00
quantity = 100
```

### 资产字段

| 字段 | 说明 |
| --- | --- |
| `kind` | `stock` / `fund` / `gold` |
| `name` | 展示名称，可省略；省略时使用行情接口返回的名称 |
| `symbol` | 股票或黄金代码，如 `sh600000`、`000001`、`00700.HK`、`AAPL`、`AU9999`、`hf_XAU` |
| `secid` | 东方财富 secid，优先级最高，如 `1.600000`、`116.00700`、`118.AU9999` |
| `code` | 场外基金代码，如 `161725`；场内基金也可以直接写代码 |
| `cost` | 成本价 / 持仓均价 |
| `quantity` | 数量：股票为股数，基金为份额，黄金为克数；用于计算浮动盈亏 |
| `principal` | 本金；与 `quantity` 一起配置时自动计算成本价 |
| `as_of` | 期初持仓日期（`YYYY-MM-DD`），见下方「交割单自动导入」 |
| `trades` | 成交流水表（`[[assets.trades]]`），见下方「交割单自动导入」 |
| `threshold_percent` | 该资产单独的回本阈值，覆盖全局配置 |
| `target_price` | 直接指定触发价，优先级最高 |
| `sip` | 定投提醒配置，见下方「定投提醒」 |

成本价二选一：

```toml
# 方式一：直接填成本价
cost = 10.00
quantity = 1000

# 方式二：填本金和数量，自动计算成本价 = principal / quantity
principal = 10000
quantity = 1000
```

### 定投提醒

股票、基金、黄金都可以在资产下添加 `sip` 表，到定投日会通过已配置的通知渠道发一次提醒。定时只负责提醒，不会自动下单，也不会自动修改成本价。

```toml
[assets.sip]
enabled = true           # 关闭后保留配置但不提醒
amount = 500.00          # 每期金额
frequency = "monthly"    # weekly / monthly
day = 1                  # 每周：1-7（周一 = 1）；每月：1-31
time = "09:30"           # 提醒时间 HH:MM
```

- 每月定投日超过当月天数时按当月最后一天处理（例如 31 日在 2 月按 28/29 日提醒）
- 同一天只提醒一次；提醒发送成功后在状态文件里记录日期
- 提醒内容会带上当前价和按当前价估算的可买数量
- 暂停自动盯盘时，定投提醒也会一起暂停
- 场外基金会先查询天天基金申购状态（`SGZT` / `BUY` / `MINSG` / `MAXSG`）：
  - 暂停申购、封闭、低于最小申购金额或超过单笔上限时，发送「定投阻断提醒」，明确提示本期资金不要计入持仓
  - 状态查询失败时发送带警告的提醒，请人工确认后再买入
  - 阻断提醒与正常提醒分别记录，状态当天恢复开放后仍会正常提醒
- 申购状态会缓存在状态文件中：定投日 15 分钟、其他时间 6 小时，避免频繁请求接口
- 当前仍只提醒、不会自动下单，也不会自动修改本金 / 份额

### 交割单自动导入（卖出自动扣本金）

把券商导出的 CSV / TXT 交割单放进一个固定目录，每次盯盘前自动读取新成交，**不需要再手动修改本金**：

```toml
[import]
dir = "~/Documents/交割单"   # 支持 ~；留空则不自动导入
```

也可以不配目录，直接导入一次：

```bash
costline import ~/Downloads/交割单.csv
costline import ~/Documents/交割单 --force   # 忽略文件缓存，重新解析
```

导入的成交写入状态文件（不修改 `costline.toml`），按成交编号去重，重复导入同一个文件不会重复扣本金。

本金采用**摊薄成本（方案 B）**：

- 买入：`本金 += 成交金额 + 手续费`，`数量 += 成交数量`
- 卖出：`本金 -= 成交金额 - 手续费`，`数量 -= 成交数量`
- 成本价：`剩余本金 / 剩余数量`
- 卖出盈利会让成本价下降（回本线降低），卖出亏损会让成本价上升
- 剩余本金 ≤ 0 时标记「本金已收回」，自动关闭回本提醒
- 剩余数量为 0 时标记「已清仓」，不再提醒
- 卖出数量超过当前持有数量时拒绝该笔成交，并在导入结果里给出提示

期初持仓与流水的关系：

```toml
[[assets]]
kind = "stock"
symbol = "sh600000"
principal = 10000   # 期初本金
quantity = 1000     # 期初数量
as_of = "2026-10-01" # 期初持仓日期：只应用这一天之后的成交

[[assets.trades]]
side = "sell"
quantity = 500
price = 12.0
fee = 3.0
date = "2026-10-20"
```

如果交割单里包含建仓记录，可以省略 `principal` / `quantity`，只保留 `[[assets.trades]]` 或完全交给自动导入。`as_of` 用于避免「期初持仓」和交割单里的历史成交重复计算。

支持的列名（会自动识别常见券商表头）：

| 含义 | 可识别的列名 |
| --- | --- |
| 成交日期 | `成交日期`、`交易日期`、`发生日期`、`清算日期`、`成交时间`、`日期` |
| 代码 | `证券代码`、`股票代码`、`基金代码`、`产品代码`、`代码` |
| 买卖方向 | `买卖标志`、`买卖方向`、`交易方向`、`业务名称`、`操作`、`摘要` |
| 成交数量 | `成交数量`、`成交股数`、`成交份额`、`发生数量`、`数量`、`份额`、`股数` |
| 成交价格 | `成交价格`、`成交均价`、`成交价`、`成交净值`、`价格`、`净值` |
| 成交金额 | `成交金额`、`发生金额`、`成交额`、`金额` |
| 费用 | `总费用`、`交易费用`、`手续费`、`费用`、`佣金`，以及单独的`印花税`、`过户费`、`规费` 等 |
| 成交编号 | `成交编号`、`合同编号`、`成交序号`、`流水号`、`委托编号`、`序号`（用于去重） |

支持 UTF-8（含 BOM）与 GBK / GB18030 编码，自动识别逗号、制表符、分号等分隔符。

### 支持的代码格式

| 市场 | 写法示例 |
| --- | --- |
| A 股 | `sh600000`、`600000`、`600000.SH`、`0.000001`、`sz000001` |
| 港股 | `00700.HK`、`hk700`、`116.00700` |
| 美股 | `AAPL`、`AAPL.US`、`105.AAPL`、`106.IBM` |
| ETF / LOF | `kind = "fund"` + `code = "518880"` / `code = "159915"` |
| 场外基金 | `kind = "fund"` + `code = "161725"` |
| 上海黄金 | `118.AU9999`（Au99.99，元/克）、`118.AUTD`（黄金 T+D，元/克） |
| 沪金主连 | `nf_AU0` 或 `113.aum`（元/克） |
| 国际金价 | `hf_XAU`（美元/盎司） |

港股、美股的现价和盈亏使用对应币种（HKD / USD），不会自动换算成人民币。

## 通知渠道

设置页里可以逐个添加，也可以直接改 TOML。配置后建议先在设置页点「测试通知」。

### Bark

```toml
[[notify.channels]]
type = "bark"
key = "你的BarkKey"
server = "https://api.day.app"   # 可选，自建 Bark 时修改
group = "costline"               # 可选
sound = "default"                # 可选
level = "active"                 # 可选
```

### Server 酱

```toml
[[notify.channels]]
type = "serverchan"
sendkey = "sctp... 或 SCT..."
# version = 2                    # 旧版 SCT 开头的 SendKey 可显式指定
# base_url = "https://..."       # 可选，自建/反代
```

### PushPlus

```toml
[[notify.channels]]
type = "pushplus"
token = "你的PushPlusToken"
# topic = "群组编码"              # 可选
```

### Telegram

```toml
[[notify.channels]]
type = "telegram"
bot_token = "123456:ABC..."
chat_id = "123456789"
```

### 企业微信 / 钉钉 / 飞书

```toml
[[notify.channels]]
type = "wecom"
webhook = "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=..."

[[notify.channels]]
type = "dingtalk"
webhook = "https://oapi.dingtalk.com/robot/send?access_token=..."
# secret = "SEC..."               # 开启“加签”时填写

[[notify.channels]]
type = "feishu"
webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/..."
# secret = "..."                  # 开启签名校验时填写
```

### ntfy / Gotify

```toml
[[notify.channels]]
type = "ntfy"
topic = "costline-请换成随机主题"
server = "https://ntfy.sh"
# token = "tk_..."                # 自建服务需要鉴权时填写

[[notify.channels]]
type = "gotify"
server = "https://gotify.example.com"
token = "你的AppToken"
```

### 通用 Webhook

会向指定 URL POST JSON：

```json
{
  "title": "costline 回本提醒",
  "content": "...",
  "timestamp": "2026-11-03T15:04:05+08:00",
  "source": "costline"
}
```

```toml
[[notify.channels]]
type = "webhook"
url = "https://example.com/hook"

[notify.channels.headers]
Authorization = "Bearer xxx"
```

### 系统本地通知

桌面版不需要配置：每次触发时会自动发送一条以 Costline 名义出现的系统通知，同时在应用窗口顶部显示提醒条，并继续发送下面配置的所有远程渠道。

如果希望自定义提示音，可以额外添加 `desktop` 渠道：

```toml
[[notify.channels]]
type = "desktop"
sound = "Glass"
```

命令行版必须显式配置 `desktop` 渠道才会发送系统通知，macOS 上通过 `osascript` 调用通知中心。首次使用如果收不到，请到「系统设置 → 通知」里允许 Costline（命令行版需允许终端 / osascript）的通知。

开发模式（`make dev`）下，Tauri 会把通知伪装成 Terminal 发出；如果「系统设置 → 通知」里关闭了 Terminal，测试通知就看不到。本机安装过 Costline 时，应用会改为使用自己的 bundle id 发送，开发模式下也能正常显示。

## 命令行版

命令行版与托盘应用共用 `costline-core`，配置格式完全一致。

```bash
make cli ARGS='init'          # 生成示例配置
make cli ARGS='check'         # 检查一次
make cli ARGS='check --json'  # JSON 输出
make cli ARGS='check --no-notify'
make cli ARGS='check --force-notify'
make cli ARGS='watch'         # 持续盯盘
make cli ARGS='import'        # 导入配置中 import.dir 的交割单
make cli ARGS='import ~/Downloads/交割单.csv'
make cli ARGS='test-notify'   # 测试通知渠道
```

也可以直接构建并运行：

```bash
cargo build --release -p costline-cli
./target/release/costline check --config ./costline.toml
```

`--config` / `--state` 是全局参数，可以放在子命令前后，也支持环境变量：

```bash
export COSTLINE_CONFIG=/path/to/config.toml
export COSTLINE_STATE=/path/to/state.json
```

## 目录结构

```
.
├── crates/costline-core/   # 共享核心：配置、行情、通知、状态、检查循环
├── cli/                    # 命令行版
├── src/                    # React 前端
├── src-tauri/              # Tauri 托盘应用
├── scripts/                # 版本同步、latest.json 生成
├── .github/workflows/      # CI / Release
├── dist/                   # 前端构建产物（编译时嵌入）
└── Makefile
```

## 数据来源与注意事项

- A 股、港股、美股、ETF、上海金、沪金：东方财富公开行情接口
- 场外基金：优先天天基金盘中估值，失败时回退到蛋卷基金最新净值
- 国际金价：新浪财经 `hf_XAU`
- 这些公开接口可能限流、延迟或变更，建议轮询间隔不低于 30 秒
- 场外基金取不到盘中估值时，回退到「最新净值」，通常是上一个交易日的数据，不是实时价格
- 货币基金、部分 QDII 可能没有估值，需使用最新净值或改用其他资产类型
- 本工具只做价格提醒，不构成任何投资建议

## 后台常驻

托盘应用本身就是常驻进程。如果想开机自启，可以把 `Costline.app` 加入「系统设置 → 通用 → 登录项」。

命令行版可以用 launchd：

保存为 `~/Library/LaunchAgents/com.example.costline.plist`：

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.example.costline</string>
  <key>ProgramArguments</key>
  <array>
    <string>/Users/you/.local/bin/costline</string>
    <string>watch</string>
    <string>--config</string>
    <string>/Users/you/Library/Application Support/costline/config.toml</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>/tmp/costline.log</string>
  <key>StandardErrorPath</key>
  <string>/tmp/costline.err</string>
</dict>
</plist>
```

```bash
launchctl load ~/Library/LaunchAgents/com.example.costline.plist
```

## 安全提示

配置文件里可能包含 Bark Key、Telegram Bot Token、群机器人 Webhook 等敏感信息，建议：

```bash
chmod 600 ~/Library/Application\ Support/com.costline.app/config.toml
```

不要把真实配置文件提交到 Git。
