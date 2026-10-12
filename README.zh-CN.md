<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/media/hero-dark.svg" />
  <img src="docs/media/hero.svg" width="960" alt="TRANSPLANE-COMPANY — 与自己的 AI 军师一起经营公司" />
</picture>

**像玩游戏一样，一人经营一家公司。带上你的 AI 军师。**

面向 OPC 超级个体的叙事经营游戏，也能成为真实业务的经营工具。

[English](README.md) · 简体中文

[![Rust](https://img.shields.io/badge/Rust-1.94%2B-5c4638?style=flat-square&logo=rust&logoColor=ffffff)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/React-19-2d5748?style=flat-square&logo=react&logoColor=ffffff)](https://react.dev/)
[![SQLite](https://img.shields.io/badge/SQLite-default-9a6b37?style=flat-square&logo=sqlite&logoColor=ffffff)](https://sqlite.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-supported-9a6b37?style=flat-square&logo=postgresql&logoColor=ffffff)](https://www.postgresql.org/)
[![License](https://img.shields.io/badge/License-Noncommercial-2d5748?style=flat-square)](LICENSE)
[![TRANSPLANE 官网](https://img.shields.io/badge/TRANSPLANE-官网-32796b?style=flat-square)](https://transplane.sg/)
[![Discord 社区](https://img.shields.io/badge/Discord-交流反馈-32796b?style=flat-square&logo=discord&logoColor=ffffff)](https://discord.gg/b6q452dYMT)
[![GitHub stars](https://img.shields.io/github/stars/zhefox/TRANSPLANE-COMPANY?style=flat-square&color=9a6b37)](https://github.com/zhefox/TRANSPLANE-COMPANY/stargazers)

[快速开始](#快速开始) · [怎么玩](#怎么玩) · [Agent 接口](docs/agent-interface.md) · [反馈问题](https://github.com/zhefox/TRANSPLANE-COMPANY/issues)

</div>

## 看看怎么玩

<video src="https://github.com/user-attachments/assets/6c7f0b05-7954-4266-b7df-fab68b3d03db" width="100%" controls></video>

## 公司下一步，听你的

客户想要一个定制功能，账上的钱开始吃紧，潜在伙伴带着新条件回来。你怎么选？

和 Agent 商量，作出选择，再看看会发生什么。Agent 同时担任主持人和军师：读取公司资料、研究相关市场信息、推演不同方案，并维护经营档案。像素游戏界面把剧情、资源、人物关系和成长进度摆到眼前。

**既是游戏，也是生产力。**

| 从这里开局 | 你能做什么 |
|---|---|
| **模拟公司** | 从零开一家虚构公司。面对客户、承诺与经营取舍，让之前的选择推动后续剧情。 |
| **真实业务** | 使用实际资金、客户、订阅、目标和报告，与 Agent 分析处境，决定下一步。 |
| **真实公司的推演分支** | 在独立模拟中尝试另一条路线，结果保留在模拟档案中。 |

情景假设与现实依据随分析保存。真实业务缺少的数据保持未知，按需补齐。

## 怎么玩

1. **创建公司。** 选择模拟或真实经营，描述业务，与 Agent 确认初始资源和目标。
2. **进入剧情。** Agent 准备当前处境和具有经营意义的选择。
3. **作出决定。** 直接用文字回复、提出自己的方案，或在游戏页面选择。
4. **查看结果。** Agent 保存经营结果，人物关系、未兑现承诺与延迟后果延续到下一事件。

全程在现有 Agent 对话中也能游玩。想看状态、报告或菜单时，可以获得对应页面链接；配置截图服务后，还能收到实际界面截图。在网页保存的选择会等待 Agent 处理。

| 经营桌上的内容 | 用途 |
|---|---|
| **事件与选择** | 当前剧情、已保存决定与回合结算 |
| **经营档案** | 现金流、MRR／ARR、产品、客户与资源 |
| **关系网** | 客户、伙伴和企业关系，以及收入与成本归属 |
| **任务与信笺** | 主线、支线、日常任务、机会、风险预警与模拟事件线索 |
| **成长进度** | 目标路线、成果，以及依据经营记录计算的等级和经验 |
| **推演与复盘** | 情景假设、周报／月报和结果对照 |
| **公司知识** | 文件预览、可复用经营 Skills 与历史记录 |

## 快速开始

[下载对应系统的发行版](https://github.com/zhefox/TRANSPLANE-COMPANY/releases/latest)。按该版本列出的操作系统与 CPU 架构选择安装包，并核对 `SHA256SUMS.txt` 中的摘要。

### Windows · 双击开始

将整个 ZIP 解压到可写目录，双击 **`company-launch.exe`**，服务启动后会打开游戏。首次启动时保存启动器显示的主人凭据，用它登录。关闭启动器窗口会停止服务。

经营档案保存在程序旁的 `company-data/`，升级时保留这个目录。发行包已包含游戏界面，无需安装 Rust、Node.js 或 Docker。

### macOS 与 Linux · 解压启动

解压与 CPU 对应的安装包后运行：

```bash
./company-launch
```

启动器在程序旁建立 `company-data/` 并打开游戏。无桌面的主机使用 `./company-launch --no-browser --data-dir /你的持久目录/company-data`，保存首次输出的主人凭据。

### 让 Agent 帮你部署

复制到你正在使用的 Agent 对话：

```text
请将 https://github.com/zhefox/TRANSPLANE-COMPANY 部署到这台电脑。
识别操作系统与 CPU 架构，下载匹配的官方发行包并校验 SHA256SUMS.txt。
使用固定的持久数据目录，保留已有公司档案与凭据。启动 company-launch，
确认服务可访问，给我游戏链接和安全的登录方式。
读取 /api/v1/agent-entry 与相关操作合同，然后引导我选择模拟或真实经营，
确认业务、资源和目标，创建或继续已有公司。遵循部署的 player-output
呈现规范，给出第一个经营事件，等待我的选择。
```

需要服务器托管时：

```text
请将 https://github.com/zhefox/TRANSPLANE-COMPANY 托管到我授权的服务器。
缺少主机或域名时先向我索取。选择并校验匹配的发行包；没有对应程序时，
从该版本的源码构建。配置持久目录、进程管理、HTTPS 反向代理，后端仅本机
访问。凭据不进入链接、公司文件和日志，为 Agent 建立按公司授权的访问。
验证启动、重启与备份恢复，给我游戏地址、安全登录方式及停止、更新、恢复
的方法。读取 /api/v1/agent-entry，接续公司建档与首个事件，并遵循部署的
player-output 呈现规范。
```

[系统要求、源码构建与托管步骤 →](docs/getting-started.zh-CN.md)

### Windows · 从源码启动

安装 **Rust 1.94+**、系统 C 编译工具链、**Node.js 24** 和 npm。在仓库目录运行：

```powershell
.\scripts\build-company.ps1 -BuildProfile Release -InstallDependencies
.\target\release\company.exe --data-dir .\company-data init
.\target\release\company-server.exe --data-dir .\company-data
```

保存初始化显示的 `owner_token`，打开 **[localhost:4177](http://127.0.0.1:4177)** 登录。创建公司后，把开场交接内容发给自己的 Agent。

### Docker · SQLite

```bash
docker compose build company
docker compose run --rm --entrypoint company company init
docker compose up -d company
```

保存初始化凭据，打开 **[localhost:4177](http://127.0.0.1:4177)**。经营数据保存在宿主机的 `company-data/`。

[可执行程序、源码构建与 PostgreSQL →](docs/getting-started.zh-CN.md#安装发行版)

## 接入自己的 Agent

把本地数据目录或已授权的服务地址告诉 Agent，它从当前部署读取入口，按任务发现所需操作合同：

```text
GET /api/v1/agent-entry
GET /api/v1/discovery
GET /api/v1/agent-guide?topic=connect
```

本地 Agent 可以使用：

```powershell
.\target\release\company.exe --data-dir .\company-data --lang zh-CN guide --topic connect
.\target\release\company.exe --data-dir .\company-data --lang zh-CN play --company <company-id>
```

无需本地安装 Skill，也可以选择用[入口 Skill](skills/transplane-company/SKILL.md)在客户端保留接入方法。远程查询与修改使用按公司授权的凭据。

平台保存、校验并呈现经营状态；推理、模型调用、定时任务和获授权的外部行动由自己的 Agent 负责。独立部署无需 TransPlane 世界账号。

## 公司档案，掌握在自己手里

- **记录与文件共存。** 默认 SQLite，可选 PostgreSQL；开放公司目录保存知识与原件。
- **一致的经营规则。** CLI、HTTP 和文件导入共用校验、请求去重与版本检查，现金更正保留历史。
- **可复用的经营经验。** Agent 按需读取托管 Skills，主人可以编辑规范、沉淀长期经验。
- **可以扩展的能力。** 插件贡献模板、指标、界面、导入与备份目的地；执行型插件作为可信本地进程运行。
- **完整备份与迁移。** 版本化快照保留经营记录、文件版本与插件数据，恢复后重新建立授权。

## 项目文档

| 内容 | 简体中文 | English |
|---|---|---|
| 部署与第一次游玩 | [使用指南](docs/getting-started.zh-CN.md) | [Getting started](docs/getting-started.md) |
| Agent 接入 | [Agent 接口](docs/agent-interface.md) | [Agent interface](docs/agent-interface.en.md) |
| 托管经营 Skills | [经营 Skills](docs/hosted-skills.md) | [Skills](docs/hosted-skills.en.md) |
| 插件开发 | [插件接口](docs/plugins.md) | [Plugins](docs/plugins.en.md) |

[部署与恢复](docs/operations.md) · [第三方许可](THIRD_PARTY_NOTICES.md)

## 许可

采用 [PolyForm Noncommercial 1.0.0](LICENSE)。商业使用，包括用来经营产生收入的业务，须另行获得权利人许可。
