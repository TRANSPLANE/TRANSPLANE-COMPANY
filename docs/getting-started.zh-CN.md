# 使用指南

[English](getting-started.md) · [项目首页](../README.zh-CN.md) · [下载最新版本](https://github.com/zhefox/TRANSPLANE-COMPANY/releases/latest)

从不需要模型的四周工作室开始，也可与自己的 AI 军师自由经营。还可以维护真实公司，或创建独立推演分支。

## 安装发行版

从[最新 Release](https://github.com/zhefox/TRANSPLANE-COMPANY/releases/latest) 下载与系统、处理器匹配的发行包和 `SHA256SUMS.txt`。解压前核对包的 SHA-256 与校验文件对应项。可下载的预编译平台以该 Release 实际附件为准。

| 系统 | 发行包后缀 |
|---|---|
| Windows，Intel/AMD 64 位 | `windows-x86_64.zip` |
| ARM Windows | `windows-aarch64.zip` |
| macOS，Intel | `macos-x86_64.tar.gz` |
| macOS，Apple Silicon | `macos-aarch64.tar.gz` |
| Linux，Intel/AMD 64 位 | `linux-x86_64.tar.gz` 或 `linux-x86_64-musl.tar.gz` |
| Linux，ARM 64 位 | `linux-aarch64.tar.gz` 或 `linux-aarch64-musl.tar.gz` |
| Linux，ARMv7 硬浮点 | `linux-armv7.tar.gz` |
| Linux，Intel/AMD 32 位 | `linux-i686.tar.gz` |
| Linux，RISC-V 64 位 | `linux-riscv64.tar.gz` |

GNU Linux 包需要 glibc 2.35 或更新版本。Alpine Linux 选择与处理器匹配的 musl 包。其他系统和处理器组合需兼容的源码构建或容器宿主机，同一个包不能在所有系统运行。

校验示例：

```powershell
Get-FileHash .\TRANSPLANE-COMPANY-*.zip -Algorithm SHA256
```

```bash
sha256sum TRANSPLANE-COMPANY-*.tar.gz
# macOS：
shasum -a 256 TRANSPLANE-COMPANY-*.tar.gz
```

发行程序已包含前端，运行不需要 Rust、Node.js 或 Docker。

### Windows：双击启动

1. 将 ZIP 解压到可写文件夹。
2. 双击 **`company-launch.exe`**，服务会启动并打开浏览器。
3. 首次启动时保存窗口中的主人凭据。点击 **Copy token（复制凭据）**，粘贴到网页登录框。
4. 游玩期间保持服务窗口开启；点击 **Stop service（停止服务）** 或关闭窗口会停止本次服务。

启动器默认把数据保存到程序旁的 `company-data`，从其他工作目录启动也不会改变。升级时保留该文件夹。再次启动已有部署不会替换或重新显示原主人凭据。

指定固定目录、端口和中文窗口：

```powershell
.\company-launch.exe --data-dir C:\CompanyData --bind 127.0.0.1:4177 --lang zh-CN
```

### macOS 与 Linux：终端启动

解压后进入发行目录：

```bash
./Start-Company.sh --data-dir "$HOME/.local/share/transplane-company" --lang zh-CN
```

也可直接启动程序：

```bash
chmod +x company company-server company-launch
./company-launch --data-dir "$HOME/.local/share/transplane-company" --lang zh-CN
```

启动器会输出地址，并在首次启动时输出主人凭据，保存后登录。它通过 macOS 的 `open` 或 Linux 的 `xdg-open` 打开浏览器，终端进程保持服务运行，按 **Ctrl+C** 停止。没有桌面浏览器的主机使用 `--no-browser`，通过已配置的 HTTPS 入口或 SSH 隧道访问。

### 由 Agent 或服务器托管

初始化一次，再使用同一固定目录启动 `company-server`：

```bash
./company --data-dir /srv/transplane-company/data init
./company-server --data-dir /srv/transplane-company/data --bind 127.0.0.1:4177
```

`company init` 只在创建主人凭据时显示一次；`company-server` 不创建凭据，也不打开浏览器。由主机服务管理器维持运行，远程访问使用自己管理的 HTTPS 反向代理。配置与备份见[部署与恢复](operations.md)。

如果指定地址被其他服务占用，改用其他 `--bind` 端口；启动器不会将那个服务当作本项目打开。

## 复制给 Agent 的部署提示词

### 部署到当前电脑

```text
请在这台电脑部署 TRANSPLANE-COMPANY，并带我开始游玩。

从 https://github.com/zhefox/TRANSPLANE-COMPANY/releases/latest 读取发行版。识别这台电脑的操作系统与处理器，下载实际提供的匹配包，并核对 SHA256SUMS.txt。没有匹配包时，先说明源码构建或容器方案，再安装所需依赖。

选择可写、固定且升级后保留的数据目录；发现已有部署时复用，保留公司、文件和凭据。服务只监听本机。Windows 使用 company-launch.exe 打开服务窗口与浏览器；由你托管时，使用 --no-browser 或由服务管理器运行 company-server。

主人和 Agent 凭据保存在安全的本机凭据存储，不写入页面链接、公司资料、截图或 Git 仓库。给我游戏入口和安全登录方法。

读取运行服务的 /api/v1/agent-entry，以及所需能力合同和引导。让我选择四周工作室、Agent 模拟或真实经营。若我选择工作室，创建或使用空白 CNY 模拟公司，先明确确认 ¥6,000 启动现金、每周 20 小时、每周 ¥800 开销和四周周期，再读取 studio 合同并展示首周工时方案。工作室不需要额外模型。其他玩法确认资源与目标，遵守 player-output 规范准备首个事件。不要重复创建公司；真实经营缺少的数据保持未知。
```

### 部署到我的服务器

```text
请把 TRANSPLANE-COMPANY 部署到我已授权你管理的服务器，并带我开始游玩。使用我提供的主机、域名和访问方式；缺少必要配置时向我询问。未经授权不要创建付费资源或使用其他账号。

从 https://github.com/zhefox/TRANSPLANE-COMPANY/releases/latest 选择与主机系统和处理器匹配的发行包，并核对 SHA256SUMS.txt。使用独立于版本程序目录的固定持久数据目录。保留已有数据和凭据，升级前备份。

仅在需要时初始化。用主机服务管理器运行 company-server，监听本机并由我管理的 HTTPS 反向代理提供远程入口，配置重启与备份。适合容器时，使用与主机兼容的镜像和持久卷。不要通过远程明文 HTTP 提供登录页。

秘密保存在主机或 Agent 的安全凭据存储，不放入 URL、公司文件、截图或 Git。给我 HTTPS 游戏地址、安全登录方法，以及启动、停止和备份方法。远程 Agent 只获得所需公司和能力范围的凭据。

核对健康接口与主人登录，再读取 /api/v1/agent-entry 和所需合同与引导。让我选择四周工作室或 Agent 自由经营。若选工作室，在空白 CNY 模拟公司先明确确认 ¥6,000、每周 20 小时、每周 ¥800 开销和四周周期，再读取 studio 合同并展示首周方案。其他玩法接续建档或已有游戏，遵守 player-output 规则并确认模式、资源与目标。真实经营数字使用实际来源，模拟假设明确保存。
```

## 开始经营

登录后新建公司，确认玩法、币种、时区和起始日期。四周工作室使用空白 CNY 模拟公司，在开局页面确认 ¥6,000、每周 20 小时、每周 ¥800 开销和四周周期。Agent 自由经营把开场任务发给自己的 Agent，补充资源、投入时间和近期目标。

| 玩法 | 起步方式 |
|---|---|
| 四周工作室 | 在固定委托与 24 小时产品之间分配工时，预览后由系统结算，不需要模型 |
| Agent 模拟经营 | 与 Agent 确认虚拟资源与背景，不需要已有真实公司 |
| 真实经营 | 使用实际经营资料，未知资金和客户信息按需补齐 |
| 从真实公司推演 | 建立独立分支尝试不同方案，不改动真实账目 |

工作室先预览一周分配，再确认结算。完工才按固定期限收款，闲置工时不结转，即使不工作也扣固定开销。第四周后核对三项目标，选择暂停、新建公司重玩或 Agent 自由经营。阅读[规则与四周示例](studio.zh-CN.md)。

Agent 经营可以说“我选……”、提出计划、询问状态，或在网页选择。叙事选择只保存意向，等待 Agent 处理，不扣款或执行外部行动，领取前可重选。需要手动交接时复制意向到 Agent 对话，也可以配置下文的持续接收适配器。

结果保存后说“继续”进入下一事件；当前尚未选择时，“继续”会展开当前方案。人物关系、承诺和延迟后果随经营进展延续。

“关系网”连接客户、伙伴、供应商和资源。“任务与信笺”展示目标、任务、机会与风险。经验来自已保存成果和复盘；没有完整成本依据时，收益归属中的利润保持未知。

需要经营预测时，把公司实际记录和相关资料交给 Agent。它可补充获准且能检索的最新公共来源，保留 URL、观察时间和假设；必要资料不足时提供数据或最小来源授权，实际未知保持未知。不接入 WORLD，本地独立推理也始终可用。

未来可选接入 WORLD，补充独立视角与 Agent 互动信号。适配器尚未实现，现有接入引导负责将自己的 Agent 连接到独立公司服务。模拟 Agent 反应仍是情景假设，不成为现实客户反馈。评估附加信号时标明来源，固定公司快照、预测窗口和指标口径，将本地与附加信号预测对照同一截止点的实际结果，依据这些证据判断是否改善。

## 接入自己的 Agent

告诉 Agent 固定数据目录或已授权的服务地址，无须本地安装 Skill。从当前部署读取：

```text
GET /api/v1/agent-entry
GET /api/v1/discovery
GET /api/v1/agent-guide?topic=connect
```

受保护操作使用 `Authorization: Bearer <凭据>`。主人会话为远程 Agent 授予指定公司与能力范围，输入格式和重试方式以部署当前合同为准。

本地 CLI 使用与服务相同的数据目录：

```powershell
.\company.exe --data-dir C:\CompanyData --lang zh-CN discover
.\company.exe --data-dir C:\CompanyData --lang zh-CN guide --topic connect
.\company.exe --data-dir C:\CompanyData --lang zh-CN play --company <公司标识>
```

Agent 按需读取部署内经营 Skills，并遵守玩家叙事规则。用户可在“插件”中管理 Skills，在“资料”中预览文件，也可向 Agent 索取状态、报告和页面入口。实际截图需要[可选渲染器](operations.md#可选前端截图)。

### 可选：持续连接宿主

Codex、OpenClaw 与远程 Agent 都可接收网页保存的选择。需要一个持续运行的外部适配器及你配置的宿主处理程序；不支持持续接收钩子的桌面聊天不会自动收到任务。授予指定公司的读写凭据，在宿主安全环境设置 `COMPANY_AGENT_TOKEN`，再启动：

```powershell
python scripts/agent-host.py --service-url http://127.0.0.1:4177 --company <公司标识> --label "我的 Agent" --handler-argv '["python","my-company-handler.py"]'
```

将参数数组替换为实际宿主处理程序；远程连接使用已授权的 HTTPS 服务地址。适配器把任务通过标准输入交给它，凭据仅通过环境提供；程序须按部署合同领取意向、保存结果并准备下一幕。连接后不必每回合人工复制，处理慢时可阅读已有场景资料。网页区分等待接收、已接收和已领取处理，接收不等于经营完成。

适配器持续续租并保留不含任务正文或凭据的本机去重记录。超时、异常或没有保存进展时停止，不自动重放；先核对已领取与保存状态再恢复。Ctrl+C 断开连接，已领取状态仍保留。详细任务合同、处理程序输入、`--once` 与恢复边界见[持续连接外部宿主](agent-interface.md#持续连接外部宿主)。

[Agent 接口](agent-interface.md) · [经营 Skills](hosted-skills.md) · [插件接口](plugins.md)

## 从源码构建

需要 Rust 1.94+、目标系统的 C 编译工具链、Node.js 24 与 npm。先构建前端，再构建嵌入页面的 Rust 程序。

Windows：

```powershell
.\scripts\build-company.ps1 -BuildProfile Release -InstallDependencies
.\target\release\company-launch.exe --lang zh-CN
```

macOS 或 Linux：

```bash
cd web
npm ci
npm run build
cd ..
cargo build --locked --release --workspace --bins
./target/release/company-launch --data-dir "$HOME/.local/share/transplane-company" --lang zh-CN
```

使用目标平台的编译器和依赖库。重新构建前清理过期构建产物，保留公司数据。

## Docker 与 PostgreSQL

SQLite：

```bash
docker compose build company
docker compose run --rm --entrypoint company company init
docker compose up -d company
```

保存初始化凭据后打开 `http://127.0.0.1:4177`，数据保存在宿主机 `company-data/`。源码容器构建要求基础镜像及依赖支持宿主机架构。

PostgreSQL 设置 `COMPANY_POSTGRES_PASSWORD` 和指向数据库的 `COMPANY_DATABASE_URL`，连接串特殊字符需 URL 编码。数据库就绪后初始化：

```bash
docker compose --profile postgres up -d postgres
docker compose --profile postgres exec postgres pg_isready -U company -d company
docker compose build company
docker compose --profile postgres run --rm --entrypoint company company init
docker compose --profile postgres up -d company
```

原生程序接受 `--database-url` 或 `COMPANY_DATABASE_URL`。迁移通过[导出与恢复](operations.md#恢复与后端迁移)完成，不能只更改连接串。


