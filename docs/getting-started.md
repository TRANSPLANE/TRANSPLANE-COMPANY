# Getting started

[简体中文](getting-started.zh-CN.md) · [Project home](../README.md) · [Latest release](https://github.com/zhefox/TRANSPLANE-COMPANY/releases/latest)

Start a company, discuss its next move with your own AI Agent, and see the outcome in the game. Choose a fictional simulation, your real business, or a separate simulation branch of a real company.

## Install a release

Download the package for your operating system and processor from the [latest release](https://github.com/zhefox/TRANSPLANE-COMPANY/releases/latest), together with `SHA256SUMS.txt`. Verify its SHA-256 against the matching entry before extracting it. Available prebuilt targets are determined by the Release's actual attachments.

| Operating system | Package suffix |
|---|---|
| Windows, Intel/AMD 64-bit | `windows-x86_64.zip` |
| Windows on ARM | `windows-aarch64.zip` |
| macOS, Intel | `macos-x86_64.tar.gz` |
| macOS, Apple Silicon | `macos-aarch64.tar.gz` |
| Linux, Intel/AMD 64-bit | `linux-x86_64.tar.gz` or `linux-x86_64-musl.tar.gz` |
| Linux, ARM 64-bit | `linux-aarch64.tar.gz` or `linux-aarch64-musl.tar.gz` |
| Linux, ARMv7 hard-float | `linux-armv7.tar.gz` |
| Linux, Intel/AMD 32-bit | `linux-i686.tar.gz` |
| Linux, RISC-V 64-bit | `linux-riscv64.tar.gz` |

GNU Linux packages require glibc 2.35 or newer. Choose a musl package for Alpine Linux on its listed processor. Other OS/processor combinations require a compatible source build or container host; one package does not run on every system.

Checksum examples:

```powershell
Get-FileHash .\TRANSPLANE-COMPANY-*.zip -Algorithm SHA256
```

```bash
sha256sum TRANSPLANE-COMPANY-*.tar.gz
# macOS:
shasum -a 256 TRANSPLANE-COMPANY-*.tar.gz
```

Packaged programs include the interface; Rust, Node.js and Docker are not required to run them.

### Windows: double-click to play

1. Extract the ZIP into a writable folder.
2. Double-click **`company-launch.exe`**. It starts the service and opens the game in your browser.
3. On the first launch, save the owner token shown in the service window. Use **Copy token**, then paste it into the game's sign-in form.
4. Keep the service window open while playing. **Stop service** or closing the window stops this service.

The launcher's default data folder is `company-data` beside the executable, independent of the working directory. Keep that folder when upgrading. Reopening an existing deployment does not replace or redisplay its owner token.

To select another persistent folder or port:

```powershell
.\company-launch.exe --data-dir C:\CompanyData --bind 127.0.0.1:4177 --lang en
```

### macOS and Linux: start from a terminal

Extract the package, enter its directory, then run:

```bash
./Start-Company.sh --data-dir "$HOME/.local/share/transplane-company" --lang en
```

Or start the program directly:

```bash
chmod +x company company-server company-launch
./company-launch --data-dir "$HOME/.local/share/transplane-company" --lang en
```

The launcher prints the address and, on the first launch only, the owner token. Save it and sign in. It opens a browser through macOS `open` or Linux `xdg-open`; the terminal process keeps the service running. Press **Ctrl+C** to stop it. On a machine without a desktop browser, use `--no-browser` and access the service through a configured HTTPS endpoint or SSH tunnel.

### Agent-managed or hosted service

Initialize once and run `company-server` with the same fixed data path:

```bash
./company --data-dir /srv/transplane-company/data init
./company-server --data-dir /srv/transplane-company/data --bind 127.0.0.1:4177
```

`company init` displays a newly created owner token once. `company-server` does not create tokens or open a browser. Run it under your host's service manager and put a managed HTTPS reverse proxy in front of it for remote access. See [deployment, backups and recovery](operations.md).

If another service occupies the address, choose another `--bind` port. The launcher does not open that other service or silently change deployments.

## Copy a deployment request to your Agent

### On this computer

```text
Deploy TRANSPLANE-COMPANY on this computer and help me start playing.

Use https://github.com/zhefox/TRANSPLANE-COMPANY/releases/latest. Detect this computer's OS and processor, select a matching published package, and verify it against SHA256SUMS.txt. If there is no matching package, explain the source-build or container option before installing dependencies.

Choose a writable, fixed data directory that survives upgrades. Reuse an existing deployment if present; preserve its companies, files and credentials. Bind to localhost. On Windows use company-launch.exe to open the service window and browser; for Agent-managed startup use --no-browser or company-server under a service manager.

Keep owner and Agent tokens in secure local credential storage. Do not put them in URLs, company documents, screenshots or the repository. Give me the game URL and a safe way to sign in.

Read the running service's /api/v1/agent-entry and relevant discovery contracts and guides. Continue into company setup: let me choose simulation or real operations, confirm my resources and goal, then prepare the first event using the service's player-output rules. Do not duplicate an existing company. Keep missing real data unknown.
```

### On my server

```text
Deploy TRANSPLANE-COMPANY on a server I have authorized you to manage, then help me start playing. Use the host, domain and access method I provide; ask for missing deployment details. Do not create paid resources or use another account without my authorization.

Select a matching OS/processor package from https://github.com/zhefox/TRANSPLANE-COMPANY/releases/latest and verify SHA256SUMS.txt. Use a fixed persistent data directory outside the versioned program folder. Preserve existing data and credentials; back up before upgrading.

Initialize only if needed. Run company-server under the host's service manager, bind to localhost behind a managed HTTPS reverse proxy, and configure restart and backup for this authorized host. If containers are preferable, use persistent volumes and an image compatible with the host. Do not expose a remote sign-in page over plain HTTP.

Store secrets in secure host or Agent credential storage; keep them out of URLs, company files, screenshots and Git. Give me the HTTPS game URL, a safe sign-in method, and start/stop and backup instructions. Grant each remote Agent only the company and capabilities it needs.

Check the health endpoint and owner sign-in, then read /api/v1/agent-entry and task-specific contracts and guides. Continue into setup or the existing game, following player-output narration. Confirm the mode, initial resources and goal with me before saving an opening event. Real business figures need actual sources; simulation assumptions remain explicit.
```

## Start playing

After signing in, create a company, choose its mode and confirm currency, timezone and starting date. Send its opening brief to your Agent and describe your resources, time and near-term goal.

| Mode | Starting point |
|---|---|
| Simulation | Confirm fictional resources and a setting with your Agent; no real business is required |
| Real operations | Provide actual business records; missing funds and customer data remain unknown |
| Simulate a real company | Create a separate branch to explore choices without changing the real ledger |

Say “I choose…”, propose your plan, ask for status, or select an option in the page. A browser choice saves intent and waits for your Agent; it does not spend money or execute outside actions. You can change it until the Agent claims it. If manual handoff is needed, copy the saved intent into the Agent conversation.

After results are saved, say “continue” to face the next situation. If the event still needs a choice, “continue” presents that choice. The game retains commitments, relationships and later consequences.

The relationship view connects customers, partners, suppliers and resources. The task desk shows goals, tasks, opportunities and risks. Experience comes from recorded outcomes and reviews; missing cost evidence leaves attributed profit unknown.

## Connect your Agent

Give your Agent the fixed data directory or authorized service address. No local Skill installation is required. Start with:

```text
GET /api/v1/agent-entry
GET /api/v1/discovery
GET /api/v1/agent-guide?topic=connect
```

Protected operations use `Authorization: Bearer <token>`. Use the owner session to grant remote Agents company-scoped credentials. Input formats and retry rules come from the current deployment's contracts.

Local CLI examples, using the same data directory as the server:

```bash
./company --data-dir /srv/transplane-company/data --lang en discover
./company --data-dir /srv/transplane-company/data --lang en guide --topic connect
./company --data-dir /srv/transplane-company/data --lang en play --company <company-id>
```

Agents read hosted business Skills as needed and follow the service's narration rules. Manage Skills in Plugins, preview documents in Files, and ask for status, reports or module links. Actual screenshots require the [optional renderer](operations.md#可选前端截图).

[Agent interface](agent-interface.en.md) · [Hosted Skills](hosted-skills.en.md) · [Plugins](plugins.en.md)

## Build from source

Requirements: Rust 1.94+, a C compiler/toolchain for the target, Node.js 24 and npm. Build the frontend before Rust because it is embedded in the programs.

Windows:

```powershell
.\scripts\build-company.ps1 -BuildProfile Release -InstallDependencies
.\target\release\company-launch.exe
```

macOS or Linux:

```bash
cd web
npm ci
npm run build
cd ..
cargo build --locked --release --workspace --bins
./target/release/company-launch --data-dir "$HOME/.local/share/transplane-company"
```

Use the target platform's compiler and libraries. Remove obsolete generated outputs before rebuilding, preserving company data.

## Docker and PostgreSQL

For SQLite:

```bash
docker compose build company
docker compose run --rm --entrypoint company company init
docker compose up -d company
```

Save the initialization token and open `http://127.0.0.1:4177`. Host data persists in `company-data/`. The container build requires base images and dependencies compatible with the host architecture.

For PostgreSQL, set `COMPANY_POSTGRES_PASSWORD` and a URL-encoded `COMPANY_DATABASE_URL` pointing to your database. Wait for database readiness before initialization:

```bash
docker compose --profile postgres up -d postgres
docker compose --profile postgres exec postgres pg_isready -U company -d company
docker compose build company
docker compose --profile postgres run --rm --entrypoint company company init
docker compose --profile postgres up -d company
```

Native programs accept `--database-url` or `COMPANY_DATABASE_URL`. Changing a connection string does not migrate an existing company; use [export and restore](operations.md#恢复与后端迁移).


