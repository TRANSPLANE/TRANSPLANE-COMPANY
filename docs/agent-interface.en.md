# Agent interface

## Progressive discovery

| Level | HTTP | CLI |
|---|---|---|
| Entry | `GET /api/v1/agent-entry` | `company discover` |
| Services | `GET /api/v1/discovery` | `company discover` |
| Service operations | `GET /api/v1/discovery?view=operations&service=<id>` | `company discover --view operations --service <id>` |
| Exact contract | `GET /api/v1/discovery?view=contract&operation=<id>` | `company discover --view contract --operation <id>` |
| Task guidance | `GET /api/v1/agent-guide?topic=<id>` | `company guide --topic <id>` |

Read the selected deployment's contract rather than guessing nested input fields. Installed enabled plugins join its discovery index. Company knowledge begins with `companies/<id>/INDEX.md` and relevant linked files.

For English HTTP contracts and errors, send `Accept-Language: en` or append `lang=en`; an explicit query overrides the header. `en-US` resolves to English and `zh` to Simplified Chinese. CLI uses global `--lang en` or `COMPANY_LANG=en`. Language changes presentation and guidance, never stored company data.

## Deployment, profiling and first play

After deployment, provide an accessible frontend link and safe sign-in method. Deliver credentials separately, outside company documents, Skills, screenshots and page URLs. Explain that users can create a real company, start a simulation or continue an existing company. A real business is optional for simulation.

Read the `connect` and `company` guides, reuse known settings and bundle missing business direction, available resources, time commitment and one near-term goal into one short confirmation, also checking currency, timezone and starting date. When direction is missing, offer two or three concrete simulation settings or a coherent resource setup. Save after confirmation and do not ask again for known answers. Unsupported real balances, customers, motives and revenue remain unknown.

A frontend-created company's handoff includes its ID. Read this existing company, retain its mode and base settings, then prepare resources, the goal route and first event instead of creating a duplicate. For conversation-based profiling, use the owner's authorized creation contract. Ordinary company Agent credentials cannot create a new company or fork.

`POST /api/v1/companies` accepts a `company.create` request envelope and requires an owner credential. Use `company.id` from the receipt for maintenance, rather than guessing from the name. Exact retries keep the same `request_id` and entire input. Creation IDs share deployment-level scope; another company needs a new request ID. Required fields are `name`, `currency`, `timezone` and `opening_date`; choose `mode` explicitly and optionally supply `description`. Read the current contract for remaining optional fields.

After confirmation, continue with a goal route and self-contained opening event rather than stopping at a profile or webpage link. For simulation, read `builtin:simulation-gm` and default to one near-term goal expected to provide feedback in 3–5 turns: a specific person brings an incentive, limited resources force value tradeoffs, and costs, constraints, risks and feedback timing are visible before selection. Return feedback, revisit commitments and close the phase according to player actions rather than predetermining success. Offer two or three natural routes and a player-authored plan, waiting at their decision. “Continue” cannot select an unselected route for them.

Before replying with an opening, event, settlement, continuation, state or intent handoff, apply the complete contract at the entry's `player_output.policy` or the current `play` response's `player_output_policy`. `required` makes default presentation mandatory; `layout`, `steps` and `preflight` provide layout, expression rules and a pre-send check. Older deployments use `/api/v1/agent-guide?topic=player-output`, or `company guide --topic player-output`. Default to a brief scene, necessary state, natural dialogue or action, causal consequences and a next choice, omitting empty parts. Give short strategic advice when asked for a recommendation or reason; expand sources, calculations, receipts, versions and maintenance for explicit requests, debugging or recovery from an actual failure. Explain mode at the opening and locate costs, deadlines, risks and consequential unknowns beside their information without repeating disclaimers each turn. Only saved and settled results are settled; waiting, failure and actual unknowns stay truthful.

Important simulated characters retain stable natural names or meaningful roles, identities, concerns and historical relationships. Legacy research codes such as `V5` or “Sample A” remain internal lookup keys. Read reports, follow-ups and aliases before continuing; prefer persisting mappings in authorized `context/cast.md` or equivalent knowledge and synchronizing them, without rewriting research, metrics, historical results or choices, or creating customers merely for naming. Default replies neither announce maintenance nor append old codes. Without permission to save, continue with known role titles. Respect player names, deliberate anonymity, code-name stories, actual model numbers and versions. Real people use actual or approved names, recorded actions and feedback; quote recorded words, otherwise summarize records.

Modes are fixed after creation. The owner uses a real company's `simulation.fork` operation to copy its checked current state and retain the source baseline; it neither changes the original mode nor inherits Agent credentials. Return to the source real company or create a new real company. A pure simulation without a real source cannot directly become a real company. Viewing and switching companies never transfer outcomes.

## Authorized operations

The entry's `player_output.policy` and `company.play`'s `player_output_policy` embed the complete contract in the selected language, generated from the same definition as the `player-output` guide. The entry retains `guide`, `read_before_play` and a short `instruction`; `play` retains `player_output_guide` and `player_output_instruction` for older clients. Apply the full contract whenever composing player text rather than retaining only its link or repeating old saves, maintenance instructions or tool results as the reply. `technical_details=on_explicit_request` allows players to inspect IDs, versions, sources and accounts without changing record or permission validation.

Protected HTTP requests use `Authorization: Bearer <token>`. The owner grants and revokes company-scoped Agent credentials, with reading and writing separated. Check `GET /api/v1/session` for the current identity and scopes. Local CLI access follows operating system access; it does not need an HTTP token. Never store credentials in company documents.

| Purpose | HTTP |
|---|---|
| Accessible companies | `GET /api/v1/companies` |
| Owner company creation | `POST /api/v1/companies` |
| Current company | `GET /api/v1/companies/<id>/snapshot` |
| Historical baseline | `GET /api/v1/companies/<id>/history/<revision>` |
| Records and history | `GET /api/v1/companies/<id>/records?kind=<kind>&history=true&offset=0` |
| Metrics at a date | `GET /api/v1/companies/<id>/metrics?as_of=YYYY-MM-DD` |
| Company operation | `POST /api/v1/companies/<id>/operations/<operation>` |
| Global operation | `POST /api/v1/operations/<operation>` |
| Atomic import | `POST /api/v1/companies/<id>/imports` |
| File synchronization | `POST /api/v1/companies/<id>/files/sync` |
| Registered file | `GET /api/v1/companies/<id>/files/<file-id>` |
| Safe file preview | `GET /api/v1/companies/<id>/files/<file-id>/preview?revision=<revision>` |

Record pages contain at most 500 entries and return `total`, `has_more` and `next_offset`. Follow pagination to inspect a complete ledger. A dashboard snapshot shows a recent subset of transactions.

Write envelopes contain `request_id`, optional `expected_revision` and `input`. Read the current record revision before an update. Keep the same ID and input for an exact retry. Changed input needs a new ID. Revision conflicts require reading and merging, and must not be bypassed by overwriting.

Import uses `{"operations":[{"operation":"...","request":{"request_id":"...","input":{}}}]}` with at most 1,000 operations. The CLI and HTTP routes share validation, atomic commit and receipts. Currency amounts use integer smallest units.

## Four-week studio interface

The studio and Agent-led operations share company records but use different settlement flows. Start only in an empty CNY simulation company, after the owner explicitly confirms CNY 6,000 opening cash, 20 weekly hours, CNY 800 weekly overhead and the four-week duration. The system calculates fixed rules without another model. An Agent may advise or explain allocations, but cannot change capacity, deadlines, payments or results.

First read `GET /api/v1/discovery?view=operations&service=studio`, the selected operation's exact contract and `GET /api/v1/agent-guide?topic=studio`. Dedicated HTTP routes are under `/api/v1/companies/<id>`:

| Operation | Route and input |
|---|---|
| `studio.state` | `GET /studio` reads rules, current run revision, orders, choices and deterministic previews |
| `studio.start` | `POST /studio/start` with `{request_id,expected_company_revision}` initializes the confirmed chapter |
| `studio.preview` | `POST /studio/preview` with `{run_id,expected_run_revision,allocations}` previews without settling |
| `studio.choose` | `POST /studio/choose` with `{request_id,run_id,expected_run_revision,option_id}`, or `allocations` instead of `option_id`; supply exactly one |

`run_id` is `studio-v1`; `expected_run_revision` uses `studio.state`'s `run.revision`. Built-in choices are `deliver`, `product` and `balance`. Custom allocations are `[{target,hours}]`, targeting `product`, `atlas`, `birch` or `cedar`, with positive even hours totalling at most 20. They cannot exceed remaining work or target unavailable or closed orders. An empty array permits an idle week while still charging overhead. See [Four-week studio](studio.md) for rules and a complete example.

Reading and preview require `company.read`; starting and settling require `company.write`, `finance.write`, `reports.write` and `goals.write`. Generic operations and CLI use `OperationRequest`: starting uses the company revision as `expected_revision` and `input={}`; choosing uses the run revision, with `run_id` and exactly one `option_id` or `allocations` in `input`. Deployment contracts define the dedicated HTTP mapping.

`studio.choose` atomically saves intent, work, cash transactions, a turn, exact result references and the next week's event. It does not wait for model adjudication or run the open-ended record-and-settlement loop separately. Present saved `result` and `studio` from a successful receipt. Retry a lost response with the original request ID and identical input. Read the current run after conflicts; changed input requires a new request ID.

Translate free plans into allocations, preview and obtain player confirmation before settling. Stop rule turns after week four: `run.week=5` does not permit a fifth submission. Review the three saved goal conditions. Players may pause, create another company to replay, or explicitly choose a next direction for Agent-led operations, without automatic goals or achievements.

## Keep an external host connected

Codex, OpenClaw and remote Agents can use the same receiving protocol, while the user's own host runs the model and client. Grant existing company-scoped `company.read` and `company.write` credentials, then connect from an external process. Connecting does not create credentials, change company revisions or choose for the player. A desktop chat without a persistent receiving hook still needs the external adapter below; an open chat window alone is not a connected host.

All routes below are under `/api/v1/companies/<id>`:

| Route | Purpose |
|---|---|
| `POST /agent-link/connect` | `{label,lease_seconds:60,session_id?}`; initial connection returns `session_id`, supplied for renewal, with a maximum lease of 120 seconds |
| `GET /agent-link/pending-work` | Required `session_id`, optional `after_revision`, `after_request_key` and `wait_ms` (maximum 25000); returns current `play`, `link` and nullable `work` |
| `POST /agent-link/ack` | `{session_id,request_key,receipt_id}` explicitly confirms receipt of the currently issued work |
| `POST /agent-link/disconnect` | `{session_id}` disconnects without cancelling claimed decisions or external actions |
| `GET /agent-link` | Read connection, delivery and current claim state; requires only `company.read` |

`work.kind` is `process_selection` or `prepare_event`, with stable `request_key` and the issued `receipt_id`. Reading work does not confirm delivery: without a host it requires manual handoff; unreceived work with a connection is `queued`, explicit ACK makes it `delivered`, and business `decision.claim` makes it `processing`. ACK neither claims, spends nor settles. An old receipt cannot acknowledge reselected intent. Connections and ACKs are runtime state outside the company ledger; reconnect after service restart or lease expiry.

The repository and release's `scripts/agent-host.py` is a Python-standard-library external adapter, listening continuously by default. `--once` checks once and handles at most one current task. Supply `COMPANY_AGENT_TOKEN` through the host's secure environment; command arguments contain only service, company, display name and handler:

```bash
python scripts/agent-host.py --service-url http://127.0.0.1:4177 --company <company-id> --label "My Agent" --handler-argv '["python","my-company-handler.py"]'
```

Replace `--handler-argv` with your configured host handler's argument array. The adapter starts it with `shell=False`, delivering only `{service_url,company_id,kind,request_key,play,policy}` on standard input. Credentials reach the handler only through its environment, outside URLs, task text and logs. Windows `.cmd` handlers need explicit `cmd.exe` adapter arguments; never concatenate task text into commands. Handler output is not forwarded directly. The adapter displays only connection, receipt and completion states supported by service evidence.

The handler uses authoritative `play` and current operation contracts to read saved intent, claim it, perform authorized actions, save results and settle with exact references. It may atomically import results and the next event. Studio events use the `studio.choose` flow above instead of duplicating manual adjudication. `prepare_event` uses known material to prepare a scene or direction choice; a closed phase cannot automatically assign a new player goal or create achievements. Exit zero alone is insufficient: processing requires this decision's settled exact results in the service, and preparation requires a new authoritative event.

Independent renewal keeps the connection alive during slow model work. Local receipts store only request-key hashes and status, by default in the current user's `.transplane-company/agent-host/`, configurable with `--state-file`. Continuous polling and process restarts never automatically replay received work. Timeout, failure, revoked authorization or no verifiable progress stops the adapter. The owner first checks actual claims and saved state before deciding how to recover; do not delete receipts merely to retry.

While waiting, view known scene material, simulation settings or summaries of actual originals, or prepare unpublished material and drafts. Do not present unwritten customer responses as facts. Local pause affects reading only: it neither cancels claimed host actions, sends character messages nor changes the ledger. Ctrl+C disconnects the adapter while preserving claims for review. The host owner still manages authorization for actual payments, communication and other external actions.

## Knowledge, forecasts and turns

File preview uses `files.read` and reads registered immutable content versions. Markdown and UTF-8 text return their content directly; images and PDFs use the same company's exact-version content URL, with authorization still required. Markdown does not execute raw HTML. HTML, SVG, invalid formats and files above the preview limits remain downloadable. Text is limited to 1 MiB; images and PDFs to 10 MiB. The frontend can view both the current file and a selected historical version. Reading creates no new records.

File paths stay inside the selected company. File edits require explicit synchronization, preserving bytes, checksums and revisions. Use a verified opening balance date rather than implying missing historical cash records are complete.

The local Agent forecasts from actual company records, user material and current public information it is authorized to retrieve, independently of WORLD. Predictions or linked material retain sources, observation dates, baseline snapshots, data versions, assumptions and scenarios. Request actual data or minimum source access when necessary evidence is missing; reasoning may continue from known material and explicit assumptions without inventing actual records. Weekly and monthly reports retain periods, cutoff and versions, and route changes create new versions. Real and simulation companies remain separate, keeping simulated outcomes, funds and achievements in their own company.

A future WORLD adapter can provide independent Agent perspectives and interaction signals. Only optional integration boundaries exist today; online connection is not implemented, and current entry points, contracts and guides serve the independent company service. Label authorized signals with their sources and observation dates. Simulated Agent responses remain scenario assumptions rather than actual customer feedback. Evaluate added value by retaining local and supplemented forecasts with the same company snapshot, forecast window and metric definitions, then comparing both against actual outcomes at the same cutoff. Conclude improved prediction quality only when supported by that real-world comparison.

## Hosted business Skills

Local installation is optional. Start with the `skills` field in `/api/v1/agent-entry`, read its catalog and guidance URLs, select metadata by purpose, then load only the documents needed for the task. Company catalogs, full documents, raw SKILL.md, history and write fields are described in [Hosted business Skills](hosted-skills.en.md).

Company reads use `company.read`; remote maintenance requires explicit `skills.write`. Ordinary `company.write` does not include it. Built-in and plugin documents are read-only templates that can be copied into independent company guidelines. Copies preserve the source document snapshot after the source plugin is removed. Procedures and experience retain history, idempotency and revision checks. Persist experience only when the user requests it, and do not turn one simulated outcome into an unverified real-market rule. Guidance does not expand permissions or replace current operation contracts.

The Agent conversation supports freely proposed ideas, while the game screen saves the event, choice and result. Retain company evidence, real source URLs, observation dates, facts and assumptions; amounts have checkable inputs, methods and results. Turn `research` uses `{title,url,observed_on,claim}` and `calculation` retains `{inputs,method,result}`; the current contract determines exact formats. Refresh real sources when relevance or age warrants it without requiring research every turn. Keep observation and simulation dates separate: later information cannot become a fact known earlier. Simulated customer responses and revenue use explicit assumptions rather than posing as market evidence.

Check `mode` before writing. Real companies preserve actual records. Simulations use a separate identity, ledger and operating date. Use `simulation.advance` with the current company revision to advance `simulation_date` explicitly; dates cannot move backward. A simulation turn's `after_revision` must be the currently committed company revision. Quantitative success must agree with computed goal progress. Reports and forecasts retain a fixed `metrics_snapshot`, which later edits do not replace. Forking does not inherit Agent credentials, and simulated results do not update the real ledger.

For simulation adjudication, fix constraints, response assumptions and methods before selection, retaining investigable suspense. Apply the actual action afterward rather than changing costs, limits or success conditions retroactively. Distinguish immediate changes, open commitments and delayed effects, saving them in the narrative structure below or authorized company knowledge and synchronizing edited files. Success and failure both carry existing assets, relationships and evidence forward. Phase closure offers another goal, a pivot or an exit; random punishment or unsupported rewards cannot replace business causality.

## Conversation play and frontend presentation

| Purpose | HTTP | CLI |
|---|---|---|
| Current event, full options, intent, phase, settlement and state | `GET /api/v1/companies/<id>/play` | `company play --company <id>` |
| Module links and renderer configuration status | `GET /api/v1/companies/<id>/presentation?view=<view>` | Links returned by `play` |
| Actual frontend module screenshot | `POST /api/v1/companies/<id>/presentation/capture`, body `{"view":"overview"}` | Call the network deployment |

These routes require the company's `company.read` capability. Reading does not increment data revisions. `current_event` retains the decision's revision, full options, selection templates and a claim template when awaiting an Agent. `latest_result` fixes exact result versions and historical metrics, separately from current `state`.

Phases are `awaiting_choice`, `awaiting_agent` (editable saved intent), `agent_processing` (claimed) and `awaiting_event`. Users name an option or propose a plan in conversation; the Agent saves it through the same decision operation as the frontend. For “continue”, read the phase before offering choices, processing a saved choice, showing settlement or preparing the next event. Do not treat it as permission to choose a new direction.

For missing real-business evidence, request actual data or the minimum source access and connection credentials needed to obtain it. Do not substitute fiction for actual financial, customer or business data. Keep secrets in the Agent's secure credential storage or dedicated configuration, outside company documents, reports, exports, screenshots and links. Return screenshots on request; sharing and publication require separate authorization.

Allowed views: `overview`, `operations`, `growth`, `simulation`, `reports`, `files`, `chronicle`, `plugins`. Links use `/?company=<id>#<view>` under the selected deployment and contain no credentials. The user signs in normally.

Rendering is optional; see [setup](operations.md#可选前端截图). “Configured” does not assert a verified renderer. Successful captures return actual PNG `image.base64`, MIME type, dimensions, the rendered `data_revision` and historical `captured_state`. `changed_since_capture` identifies later changes. Attach the image and quick link. HTTP 503 `screenshot_unavailable`, HTTP 502 `screenshot_failed`, and HTTP 503 `screenshot_busy` retain `play` state and links. Continue using text when captures are unavailable; never fabricate an image.

## Business decisions and handoff

| Operation | Input purpose | Permission |
|---|---|---|
| `decision.put` | Create or revise an unselected event with its `baseline_revision`, situation, options and linked goals | Authorized company business writes |
| `decision.select` | Save or replace intent with `id` and exact title/ID `choice`, `option_id`, or permitted freeform `text` | Authorized company business writes |
| `decision.claim` | Claim the latest saved intent and fix its choice and processing actor | Authorized company business writes |
| `decision.resolve` | Link a claimed event to fixed results with `result_refs` and `summary`; optionally fix `company_revision` | Authorized company business writes |
| `decision.cancel` | Cancel an unfinished event using `id` and `reason`, retaining its choice and history | Authorized company business writes |

The update envelope carries the decision's own `expected_revision`, which differs from company snapshot revisions. These writes use `company.write`; read-only credentials cannot choose. Each result reference is `{kind,id,revision}`. Options use `title`; optional `estimated_cost_minor` describes a proposed commitment and never creates a cash record. Read current contracts for detailed constraints and errors.

Reuse the latest receipt's `record.revision` for the next selection, reselection, claim or settlement update. With `decision_id`, a turn derives its selection baseline; omitted `after_revision`, report/forecast data versions and settlement company snapshots resolve in the server transaction. Explicit historical versions remain validated.

### Event experience, aftermath and story context

`decision.put` accepts optional `experience`. Legacy events need no backfill. When a new event uses it, `options` has 2–4 entries, each requiring `benefit` and `tradeoff`, with optional `commitment` and `feedback_on`. These describe benefits, opportunity costs, promises and feedback dates, without replacing `risk`, `assumptions` or actual business records. Normally enable `allow_freeform`, map natural wording to the current `option_id`, and save and adjudicate player-authored plans under the same constraints.

| `experience` field | Content |
|---|---|
| `scene` | The concrete present scene and its experiential theme |
| `why_now` | Why this happens now and its causal connection to earlier choices |
| `stakes` | Present gains and losses, limited resources and incompatible values |
| `cast` | `[{id,name,role,concern?,line?}]`: stable character IDs, natural names and identities, with optional concerns and dialogue |
| `callbacks` | Optional `[{decision_id,summary}]`, referencing only settled historical decisions in this company |
| `chapter` | Optional `{id,title,turn,planned_turns}`: arc ID, title, current scene and planned turn count, usually 3–5 turns and adjustable through choices |

A simulation turn or report may save optional `aftermath`, requiring `decision_id` to link a selected, processing or settled decision in the same company. Simulated results are saved with turns; real results with reports and actual records. Dialogue, relationship changes and promise statuses need supporting evidence; real mode does not generate them from a simulation script.

| `aftermath` field | Content |
|---|---|
| `immediate` | Optional `[string]`, immediate consequences that have occurred |
| `reactions` | Optional `[{cast_id,line}]`, character reactions |
| `relationships` | Optional `[{cast_id,change}]`, relationship changes supported by actions, without automatically changing cash, customers or metrics |
| `commitments` | Optional `[{id,title,cast_id?,due_on?,status,source_decision_id}]`, new or changed promises; `status` is `pending`, `fulfilled`, `broken` or `cancelled` |
| `chapter` | Optional `{id,status,summary?,takeaway?}`; `status` is `ongoing` or `closed`, with ending `summary` and the tradeoff's `takeaway` submitted at closure |

Commitment `id` stays stable across turns. Submit updates incrementally rather than repeating the full list; `source_decision_id` always points to the originating decision rather than changing to the current follow-up. Fulfilment, breach and cancellation require existing actions or results. A due date prompts a check and does not itself prove an occurrence. A `closed` arc does not mean a goal succeeded: saved metrics and completion evidence determine that result. `takeaway` is not an unverified real-market rule.

`company.play` returns `story_context`: `recent_choices`, `relationships` and `open_commitments` derive from exact result references of settled decisions; `cast` and `chapter` also continue the current event's saved settings. `source_ref` preserves source versions for revisiting characters, promises and the next scene. `presentation` supplies the current event experience and options; `cadence` supplies its arc and next beat. Legacy records without narrative structure return empty collections or `null`. Authorized company knowledge may supply known context; real unknowns remain unknown. These are story projections, granting no payment, communication or other external-action permissions. Fields serve maintenance and frontend presentation; default player replies use dialogue, business causality and a next choice.

Discover decision operations, then query the company's decisions and relevant knowledge. Events retain their baseline, situation, business objectives, self-contained options, risks and assumptions. Each company has at most one unfinished decision. In simulation, the GM may establish clearly fictional company backgrounds, products, customer personas, events and response assumptions without requiring real contacts or world-building answers every turn. External factual market claims still need evidence. Simulated actions, responses and revenue assumptions remain identified and never update real company records.

A player's selection saves an option identity and decision revision as a readable intent. It does not update the ledger, goals, simulation date or business records. The game shows that it is waiting for the Agent and allows the user to copy the intent into an external conversation. Copying has no delivery guarantee, and deployment credentials grant no payment, communication or other external permissions.

Claim the current decision version and reread the actual saved selection and receipt. Complete authorized real operations or explicit simulated actions, publish their results, then resolve with exact record identities and revisions. Simulation results include a turn; real results use reports or actual records allowed by the contract. Results belong to the same company and were committed after selection. Historical outcomes use the referenced version. A saved choice may change before claim; after claim, coordinate stopping, cancel and prepare a new event to change plans.

If relevant business resources change, read current records and revise an open event as the conflict requires. Ordinary reports and unrelated knowledge changes do not invalidate an event. Data revisions are system-maintained history and change references; Agents never increment them. Omitted event baselines, report/forecast versions, turn after_revision and settlement snapshots resolve inside the transaction. A turn derives before_revision only from a same-company selected/processing decision_id; otherwise supply it. Explicit historical versions are validated. Updates retain record-level revision checks.

Prepare the next event after closing the current one. Restore and migration retain events, choices and result history, but require new remote credentials. Existing v0.1 companies without decisions keep their accounts, turns and reports; an Agent prepares the first new event explicitly.

Scheduling, model calls and external business actions belong to the user's Agent client. Owner authorization for this deployment does not expand external permissions.

For an existing free-text studio selection, preview the structured allocation and obtain confirmation, then submit `studio.choose` with `allocations` and the exact saved `selection_text`. This retains the original intent while binding its translation; another processing actor or a changed selection conflicts.

`forecast.put` accepts dated `context_sources` (`company`, `public`, `world`, `assumption`), `analysis_method` and `limitations`. Each source includes a title, reference, observation date and retrieval date; optional expiry is retained even when stale. Observation ≤ retrieval ≤ forecast `as_of`. Read `/api/v1/agent-guide?topic=prediction` for source and comparison guidance.
