# 部署、备份与恢复

## 独立部署

默认 SQLite 保存在所选数据目录，页面已嵌入程序。Windows 双击 `company-launch.exe` 会初始化所需凭据、启动服务并打开浏览器；第一次保存窗口中的主人凭据。关闭窗口会停止该服务。macOS/Linux 在终端运行 `company-launch`，首次凭据在终端显示，按 Ctrl+C 停止。`--no-browser` 不打开浏览器或原生窗口，适合 Agent 管理进程。

长期托管使用 `company-server`：先用同一固定目录运行 `company init`，保存首次显示的主人凭据，再通过主机服务管理器启动服务。`company-server` 不创建凭据。默认监听 `127.0.0.1:4177`；远程入口由自己管理的 HTTPS 反向代理提供。

[English setup and Agent deployment prompts](getting-started.md#copy-a-deployment-request-to-your-agent) · [中文部署提示词](getting-started.zh-CN.md#复制给-agent-的部署提示词)

| 选项或环境变量 | 用途 |
|---|---|
| `--data-dir` / `COMPANY_DATA_DIR` | 固定运行目录；启动器默认程序旁的 `company-data`，CLI/服务器默认工作目录中的 `company-data` |
| `--database-url` / `COMPANY_DATABASE_URL` | 可选 PostgreSQL 连接串；不设置时使用 SQLite |
| `--bind` / `COMPANY_BIND` | HTTP 监听地址，默认 `127.0.0.1:4177` |
| `--web-dir` / `COMPANY_WEB_DIR` | 可选页面目录；不设置时使用编译时嵌入的页面 |
| 启动器 `--lang en` / `--lang zh-CN` | Windows 服务窗口语言；网页可单独切换语言 |
| 启动器 `--no-browser` | 保持服务运行，不打开浏览器或原生窗口 |

原生托管示例：

```bash
./company --data-dir /srv/transplane-company/data init
./company-server --data-dir /srv/transplane-company/data --bind 127.0.0.1:4177
```

将数据目录放在版本程序目录之外，让服务管理器始终传入该绝对路径。Windows 同样可使用 `C:\CompanyData`。升级前备份，停止原服务后替换程序，再以原目录启动；保留 `company-data` 和原凭据。遗失主人凭据时，可在该主机使用 `company --data-dir <目录> token grant --owner` 创建新凭据，并在管理页撤销不用的凭据。

启动器使用系统文件锁核对同一数据目录。重复启动会打开已经核验的本部署；端口被其他服务占用时退出，不自动改端口或打开那个服务。多个部署使用不同数据目录和端口。

容器与原生程序使用同一格式。数据库和公司文件共同组成部署数据；不要只复制 SQLite 主文件或只备份数据库。外部 Agent 负责定时调用，平台本身不唤醒 Agent。

## 可选前端截图

文字游玩和快捷链接不依赖截图服务。需要由 Agent 返回实际前端模块截图时，部署主人可安装独立渲染器：Node.js 24、固定版本 Playwright 与 Chromium。发行包包含渲染器源码和锁定依赖文件，不包含 Node.js、依赖目录或浏览器。

在发行目录运行：

```powershell
cd scripts/renderer
npm.cmd ci --ignore-scripts
npx.cmd playwright install chromium
cd ../..
```

Linux 使用 `npm` 和 `npx`；缺少浏览器所需的系统库时按 Playwright 安装诊断补齐。已有源码环境可复用 `web/node_modules` 的同版本 Playwright；独立发行包使用 `scripts/renderer/node_modules`。

在所选数据目录创建 `config/presentation.json`，将路径和地址替换为本机所选部署：

```json
{
  "command": ["C:/Program Files/nodejs/node.exe", "C:/TransPlaneCompany/scripts/capture-company.mjs"],
  "base_url": "http://127.0.0.1:4177/",
  "timeout_ms": 40000
}
```

`command` 是可信本地可执行程序及固定参数，不通过 shell 拼接。`base_url` 只允许 HTTP/HTTPS 的根地址，不接受用户名、密码、查询或片段；应指向此部署自己的前端。远程 HTTPS 入口也应只服务该部署。删除配置即可停用，每次截图读取最新配置，无须重启。

Agent 先查询 `company.presentation`，再按 `company.capture` 请求已开放模块。渲染器只收到该公司的临时 `company.read` 凭据，60 秒到期并在执行后撤销；请求通过标准输入传递，不放入链接、参数或日志。渲染器运行于可信本地进程，不提供系统沙箱。每次执行最多 45 秒，最多两个同时执行的截图，PNG 限 8 MiB、2048×8192 像素。页面数据、字体和场景图片加载后才截取模块，前后版本不同会有界重试。

配置状态表示程序已配置，不能代替一次实际成功截图。返回截图包含渲染时版本及当前状态；如之后已有变化，`changed_since_capture` 为真。未配置、程序缺失、浏览器不可用、超时或输出校验失败时，Agent 仍可读取结构化状态和快捷链接。渲染器配置不随经营备份自动恢复，迁移后由主人重新选择本机程序和部署地址。

## 日常备份

`company backup` 创建完整的版本化逻辑快照，默认保存到 `backups/`；`company export --output <路径>` 指定输出文件。可用 `--company <标识>` 导出一家公司。备份包括经营记录历史、公司文件、路线与报告版本、插件数据和必要引用，输出包含摘要信息。

目的地插件处理传送，备份内核处理快照一致性。先创建快照，再调用插件；失败时保留本地快照并返回失败。日志或存在输出文件都不能代替成功回执。

本地开放文件由 Agent 管理：修改后执行 `company sync` 登记版本。备份保留已登记的不可变文件版本，并另存公司目录中的当前工作文件；未同步草稿会保持原状态，不能视为已登记的经营依据。导入目录和未完成临时文件不包含在备份中。

## 恢复与后端迁移

1. 使用独立的空数据目录及空目标数据库。
2. 执行 `company restore --snapshot <快照>`；目标 PostgreSQL 通过 `--database-url` 指定。
3. 核对公司数量、稳定标识、当前及历史记录、金额、文件摘要和插件数据。
4. 执行 `company init` 并为 Agent 重新授权。
5. 在所选端口启动目标服务，验证经营台与客户端，再显式切换入口。

旧远程凭据不随恢复生效。原部署在目标核对完成前保留，避免恢复中断造成唯一数据来源消失。不要同时让两个部署接受同一套真实经营写入。

## 插件管理

插件安装和升级采用明确版本；停用保留数据。执行型插件以本地进程运行，权限受操作系统和部署凭据约束，首版不提供系统级沙箱。来源、兼容性、数据退出策略及过程接口见 [插件接口](plugins.md)。

## 故障核对

部署后先访问 `/api/v1/health`，再核对登录和公司页面；健康响应表示服务正在运行，不能代替登录及业务检查。Agent 从 `/api/v1/agent-entry` 读取接入规则，再按任务读取能力合同与公司资料。

发生超时后，先以原 `request_id` 核对或重试。提交结果未知时不要换标识重新入账。版本冲突先读取当前版本，再由 Agent 合并；无效导入保留原文件便于修正。

SQLite 写入在取得写事务时执行有界锁等待和重试。持续繁忙返回 `storage_busy`，远程接口对应 HTTP 503；客户端保留原请求标识，稍后原样重试。版本冲突和输入错误不能按存储繁忙自动重试。

磁盘不足、备份传送失败或进程中断后，检查明确回执、当前记录和备份摘要。快照传送失败时，本地快照仍可使用，修正问题后再传送。
