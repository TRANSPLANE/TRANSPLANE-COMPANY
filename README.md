<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/media/hero-dark.svg" />
  <img src="docs/media/hero.svg" width="960" alt="TRANSPLANE-COMPANY — a company management game with your own AI strategist" />
</picture>

**Run a one-person company like a game. Start with a four-week studio, or bring your own AI strategist.**

A company management game for solo founders (OPCs), with a fixed-rule studio, Agent-led stories and real business records.

English · [简体中文](README.zh-CN.md)

[![Rust](https://img.shields.io/badge/Rust-1.94%2B-5c4638?style=flat-square&logo=rust&logoColor=ffffff)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/React-19-2d5748?style=flat-square&logo=react&logoColor=ffffff)](https://react.dev/)
[![SQLite](https://img.shields.io/badge/SQLite-default-9a6b37?style=flat-square&logo=sqlite&logoColor=ffffff)](https://sqlite.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-supported-9a6b37?style=flat-square&logo=postgresql&logoColor=ffffff)](https://www.postgresql.org/)
[![License](https://img.shields.io/badge/License-Noncommercial-2d5748?style=flat-square)](LICENSE)
[![TRANSPLANE website](https://img.shields.io/badge/TRANSPLANE-Website-32796b?style=flat-square)](https://transplane.sg/)
[![Discord community](https://img.shields.io/badge/Discord-Community-32796b?style=flat-square&logo=discord&logoColor=ffffff)](https://discord.gg/b6q452dYMT)
[![GitHub stars](https://img.shields.io/github/stars/zhefox/TRANSPLANE-COMPANY?style=flat-square&color=9a6b37)](https://github.com/zhefox/TRANSPLANE-COMPANY/stargazers)

[Quick start](#quick-start) · [How to play](#how-to-play) · [Agent interface](docs/agent-interface.en.md) · [Report an issue](https://github.com/zhefox/TRANSPLANE-COMPANY/issues)

</div>

## Watch the game

<video src="https://github.com/user-attachments/assets/4b076493-7691-4c77-b479-e1afa48ae8e2" width="100%" controls></video>

## Your company, your next move

A customer wants a custom feature. Cash is getting tight. A potential partner comes back with an offer. What do you do?

Start with a four-week studio: CNY 6,000, twenty hours a week, commissions with deadlines and your own product to build. Allocate work, preview the consequence and confirm. The system calculates cash and progress from fixed rules; no model is needed for this chapter.

For open-ended operations, your own Agent combines actual company records, material you provide and the latest public information it can retrieve to forecast the next move. It preserves sources, observation dates and assumptions, then compares forecasts with actual outcomes. Independent local reasoning remains available. The pixel interface brings resources, relationships and progress into view.

A future optional WORLD connection can add independent perspectives and interaction signals to forecasts. Compare them with local forecasts under the same conditions and against actual outcomes to assess their value. Simulated Agent reactions remain scenario assumptions. The WORLD adapter is not implemented yet; the current connection guides help your Agent read and maintain this independent company service, without a WORLD account.

**A game to explore. A workspace to operate.**

| Start with… | What happens |
|---|---|
| **The four-week studio** | Confirm the fixed opening resources, balance commissions against product work, and finish at a saved ending after four weeks. |
| **An Agent-led simulation** | Build a fictional business from scratch. Face customers, commitments and tradeoffs; follow how earlier choices shape later events. |
| **Your real business** | Work with actual cash records, customers, subscriptions, goals and reports. Ask your Agent for analysis and the next business decision. |
| **A branch of your real company** | Explore a different strategy in a separate simulation. Its outcomes stay outside your real ledger. |

Simulation assumptions and real-world evidence are saved with the analysis. Missing real business data stays unknown until supplied.

## How to play

1. **Create a company.** Choose simulation or real operations. An empty CNY simulation can start the four-week studio.
2. **Confirm your opening.** For the studio, confirm CNY 6,000, 20 weekly hours, CNY 800 weekly overhead and four weeks. For Agent-led play, confirm your business, resources and near-term goal.
3. **Make your move.** Preview and settle a weekly work allocation, or reply to your Agent's event with an option or your own plan.
4. **Read the outcome.** Studio results follow fixed capacity, payment and deadline rules. Agent-led play saves outcomes and revisits commitments and relationships. After four weeks, pause, replay in another company or choose a next direction with your Agent.

See the [studio rules and complete example](docs/studio.md). Agent-led play also works entirely in your existing conversation; ask for status, reports or menus to get game links. An optional screenshot renderer returns the actual interface. Narrative choices saved in the browser wait for your Agent; an external host adapter can receive them continuously.

| On your desk | What you can use it for |
|---|---|
| **Events & choices** | Narrative situations, saved decisions and turn results |
| **Company records** | Cash flow, MRR/ARR, products, customers and resources |
| **Relationships** | Customer, partner and company connections; revenue and cost attribution |
| **Quests & messages** | Main, side and daily tasks, opportunities, risk alerts and simulation event clues |
| **Progress** | Goal routes, achievements, levels and experience from recorded outcomes |
| **Forecasts & reviews** | Scenario assumptions, weekly/monthly reports and comparisons with results |
| **Company knowledge** | File previews, reusable business Skills and retained history |

## Quick start

[Download a release for your system](https://github.com/zhefox/TRANSPLANE-COMPANY/releases/latest). Choose the operating system and CPU architecture listed on that release, and verify its `SHA256SUMS.txt` checksum.

### Windows · double-click to play

Extract the entire ZIP into a writable folder, then double-click **`company-launch.exe`**. It starts the service and opens the game. On first launch, save the owner credential shown in the launcher and use it to sign in. Closing the launcher stops the service.

Your records live in `company-data/` next to the executable. Keep this folder when upgrading. Packaged binaries include the game interface; Rust, Node.js and Docker are not required.

### macOS & Linux · portable package

Extract the package matching your CPU, then run:

```bash
./company-launch
```

The launcher creates `company-data/` next to the executable and opens the game. For a headless host, use `./company-launch --no-browser --data-dir /your/persistent/company-data`. Save the first-launch owner credential from the output.

### Ask your Agent to deploy

Copy this into your existing Agent conversation:

```text
Deploy TRANSPLANE-COMPANY from https://github.com/zhefox/TRANSPLANE-COMPANY
on this computer. Detect its OS and CPU, choose a matching official release,
and verify SHA256SUMS.txt. Use a fixed persistent data directory and preserve
existing company records and credentials. Start company-launch, confirm the
service is reachable, and give me the game link and a safe sign-in method.
Read /api/v1/agent-entry and the relevant operation contracts. Help me choose
the four-week studio, Agent-led simulation or real operations. If I want the
studio, create or use an empty CNY simulation, ask me to confirm CNY 6,000,
20 weekly hours, CNY 800 overhead and four weeks, then read the studio
contracts and show the first work allocation choices. The studio needs no
additional model. Otherwise confirm my business, resources and goal and
present the opening event using player-output guidance. Wait for my choice.
```

For hosting on an authorized server:

```text
Host TRANSPLANE-COMPANY from https://github.com/zhefox/TRANSPLANE-COMPANY
on the server I authorize. Ask for the host and intended domain if missing.
Choose and verify a matching release, or build from its tagged source if no
binary matches. Use persistent storage, a service supervisor, an HTTPS reverse
proxy, and local-only backend access. Keep credentials outside URLs, company
documents and logs; create scoped Agent access. Verify startup, restart and
backup/restore, then give me the game URL, a safe sign-in method, and how to
stop, update and restore it. Read /api/v1/agent-entry. Offer the four-week
studio or Agent-led operations. If I want the studio, confirm CNY 6,000,
20 weekly hours, CNY 800 overhead and four weeks in an empty CNY simulation,
read the studio contracts and show its first choices. Otherwise continue
into company setup and the first event using player-output guidance.
```

[Platform requirements, source builds & hosting →](docs/getting-started.md)

### Windows · from source

Install **Rust 1.94+**, the system C compiler toolchain, **Node.js 24** and npm. In the repository directory:

```powershell
.\scripts\build-company.ps1 -BuildProfile Release -InstallDependencies
.\target\release\company.exe --data-dir .\company-data init
.\target\release\company-server.exe --data-dir .\company-data
```

Save the `owner_token` shown during initialization, then open **[localhost:4177](http://127.0.0.1:4177)** and sign in. Create a company and send its opening brief to your Agent.

### Docker · SQLite

```bash
docker compose build company
docker compose run --rm --entrypoint company company init
docker compose up -d company
```

Save the initialization token and open **[localhost:4177](http://127.0.0.1:4177)**. Persistent data lives in `company-data/` on the host.

[Packaged binaries, source builds & PostgreSQL →](docs/getting-started.md#install-a-release)

## Connect your Agent

Give your Agent the local data directory or the authorized service address. It reads the deployment's entry and discovers the current operation contracts as needed:

```text
GET /api/v1/agent-entry
GET /api/v1/discovery
GET /api/v1/agent-guide?topic=connect
```

For a local Agent:

```powershell
.\target\release\company.exe --data-dir .\company-data --lang en guide --topic connect
.\target\release\company.exe --data-dir .\company-data --lang en play --company <company-id>
```

No local Skill installation is needed. The optional [entry Skill](skills/transplane-company/SKILL.md) can retain connection instructions in your client. Company-scoped credentials authorize remote reads and writes. For studio advice, discover the `studio` service; fixed rules settle choices. For continuous narrative handoff, configure the [external host adapter](docs/agent-interface.en.md#keep-an-external-host-connected). It supports a common handler protocol without assuming an open desktop chat automatically receives tasks.

The platform stores, validates and presents company state. Your own Agent supplies reasoning, model calls, scheduling and authorized external actions.

## Built to keep your company portable

- **Your records and files.** SQLite by default, optional PostgreSQL, plus an open company directory for knowledge and original documents.
- **One set of business rules.** CLI, HTTP and file imports share validation, request deduplication and revision checks. Cash corrections retain history.
- **Reusable business knowledge.** Agents read hosted Skills on demand; owners can edit them and preserve useful operating experience.
- **Extension points.** Plugins contribute templates, metrics, views, imports and backup destinations. Executable plugins run as trusted local processes.
- **Backup and migration.** Versioned snapshots retain company records, file versions and plugin data; restore into a fresh deployment and grant new credentials.

## Explore the project

| Guide | English | 简体中文 |
|---|---|---|
| Setup & first game | [Getting started](docs/getting-started.md) | [使用指南](docs/getting-started.zh-CN.md) |
| Four-week studio rules | [Studio](docs/studio.md) | [四周工作室](docs/studio.zh-CN.md) |
| Agent integration | [Agent interface](docs/agent-interface.en.md) | [Agent 接口](docs/agent-interface.md) |
| Hosted business Skills | [Skills](docs/hosted-skills.en.md) | [经营 Skills](docs/hosted-skills.md) |
| Plugin development | [Plugins](docs/plugins.en.md) | [插件接口](docs/plugins.md) |

[Deployment & recovery](docs/operations.md) · [Third-party notices](THIRD_PARTY_NOTICES.md)

## License

[PolyForm Noncommercial 1.0.0](LICENSE). Commercial use, including use to operate a revenue-generating business, requires separate permission from the rights holder.
