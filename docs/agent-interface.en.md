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

Read the `connect` and `company` guides. Collect the business direction, available resources, time commitment and near-term goal; confirm currency, timezone and starting date. Offer a few simulation settings or resource suggestions where useful and confirm before saving. Real balances, customers and revenue without evidence stay unknown.

A frontend-created company's handoff includes its ID. Read this existing company, retain its mode and base settings, then prepare resources, the goal route and first event instead of creating a duplicate. For conversation-based profiling, use the owner's authorized creation contract. Ordinary company Agent credentials cannot create a new company or fork.

`POST /api/v1/companies` accepts a `company.create` request envelope and requires an owner credential. Use `company.id` from the receipt for maintenance, rather than guessing from the name. Exact retries keep the same `request_id` and entire input. Creation IDs share deployment-level scope; another company needs a new request ID. Required fields are `name`, `currency`, `timezone` and `opening_date`; choose `mode` explicitly and optionally supply `description`. Read the current contract for remaining optional fields.

After profiling, continue with a near-term result goal, phase route and self-contained opening event. Present its situation, options, risks and feedback timing; explain that the player can name an option or propose a plan, then wait when their choice is needed. Creating a profile or returning a page link alone does not finish the opening. For simulation, read `builtin:simulation-gm` and carry commitments, relationships and consequences forward. “Continue” must not select a direction for an unselected event.

Before presenting openings, events or settlements, read `/api/v1/agent-guide?topic=player-output`, or `company guide --topic player-output` through the CLI. Lead player-facing replies with scene narrative, business consequences, goal feedback and a next choice. Reserve receipts, versions, raw IDs and maintenance narration for evidence requests, debugging or failure recovery. Explain mode at the opening; keep costs, deadlines, risks and consequential assumptions visible without repeating simulation disclaimers or isolation checks. Only successfully saved and settled outcomes are settled; storytelling must not conceal pending work, actual failures or unknown real-business state.

Important simulated customers and partners normally have stable names or meaningful role titles, with identities, motives, concerns and relationships carried forward. Use brief natural dialogue or concrete actions to establish tradeoffs. Research placeholders such as `V5` or “Sample A” stay in internal evidence rather than default character names. For legacy saves, read reports, follow-ups and existing aliases first. Prefer persisting internal-ID/display-name mappings in authorized `context/cast.md` or equivalent company knowledge, then synchronize and reuse them. Preserve research assumptions, metrics, historical outcomes and saved choices; naming does not require new customer records. Do not announce alias maintenance or append legacy codes in default replies. Without permission to persist aliases, continue with known role titles and authorized working memory. If player-facing wording differs from saved option titles, map it to the current event's `option_id` before submitting. Respect player names, deliberate anonymity or code-name stories. Actual model numbers and version names remain allowed; real companies use actual or approved display names without invented real identities or feedback.

Modes are fixed after creation. The owner uses a real company's `simulation.fork` operation to copy its checked current state and retain the source baseline; it neither changes the original mode nor inherits Agent credentials. Return to the source real company or create a new real company. A pure simulation without a real source cannot directly become a real company. Viewing and switching companies never transfer outcomes.

## Authorized operations

The entry's `player_output` contains `guide`, `read_before_play` and a short `instruction`. `company.play` returns `player_output_guide` and `player_output_instruction`. Read the full guidance before narrating play; do not repeat maintenance instructions or receipts in the default player reply.

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

## Knowledge, forecasts and turns

File preview uses `files.read` and reads registered immutable content versions. Markdown and UTF-8 text return their content directly; images and PDFs use the same company's exact-version content URL, with authorization still required. Markdown does not execute raw HTML. HTML, SVG, invalid formats and files above the preview limits remain downloadable. Text is limited to 1 MiB; images and PDFs to 10 MiB. The frontend can view both the current file and a selected historical version. Reading creates no new records.

File paths stay inside the selected company. File edits require explicit synchronization, preserving bytes, checksums and revisions. Use a verified opening balance date rather than implying missing historical cash records are complete.

Predictions retain their date, baseline revision, assumptions and scenarios. Weekly and monthly reports retain their periods, cutoff and baseline metrics snapshot. Route changes create new versions.

## Hosted business Skills

Local installation is optional. Start with the `skills` field in `/api/v1/agent-entry`, read its catalog and guidance URLs, select metadata by purpose, then load only the documents needed for the task. Company catalogs, full documents, raw SKILL.md, history and write fields are described in [Hosted business Skills](hosted-skills.en.md).

Company reads use `company.read`; remote maintenance requires explicit `skills.write`. Ordinary `company.write` does not include it. Built-in and plugin documents are read-only templates that can be copied into independent company guidelines. Copies preserve the source document snapshot after the source plugin is removed. Procedures and experience retain history, idempotency and revision checks. Persist experience only when the user requests it, and do not turn one simulated outcome into an unverified real-market rule. Guidance does not expand permissions or replace current operation contracts.

The Agent conversation supports freely proposed ideas, while the game screen saves the current event, choice and result. Ground business judgments in company knowledge and relevant real-world facts, retaining source URL, observation date, claim and assumptions. Turn `research` entries use `{title,url,observed_on,claim}`; `calculation` retains `{inputs,method,result}`. Read the current contract for exact formats. Fictional starting resources do not justify fabricated market demand or unmarked forecast assumptions.

Check `mode` before writing. Real companies preserve actual records. Simulations use a separate identity, ledger and operating date. Use `simulation.advance` with the current company revision to advance `simulation_date` explicitly; dates cannot move backward. A simulation turn's `after_revision` must be the currently committed company revision. Quantitative success must agree with computed goal progress. Reports and forecasts retain a fixed `metrics_snapshot`, which later edits do not replace. Forking does not inherit Agent credentials, and simulated results do not update the real ledger.

For simulation hosting, read `builtin:simulation-gm` as needed and proactively prepare events from the company's stage, resources, simulation date, open commitments and past choices. Drivers may be relevant, sourced real changes or explicitly simulated customers, competition, opportunities, failures, costs, partnerships or deadlines. Explain why the event happens now, how earlier choices led to it, and the commitments and business tradeoffs in each option. Refresh real sources when their relevance or age warrants it rather than requiring research every turn. When data is unavailable, identify assumptions; never invent URLs or use later information as a fact known earlier in the story.

Turn results distinguish immediate changes and delayed effects. Persist customer follow-ups, promises, deadlines, unrealized consequences and originating event or turn IDs in company knowledge or turn narratives. Explicitly synchronize edited files and reread these threads for later events rather than relying only on chat memory. Ground success and failure in stated risks, resource constraints or simulated responses; quantitative achievement agrees with saved metrics. Close the current event before preparing the next meaningful tradeoff. Viewing state does not generate an event, and players need not write the world themselves.

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

Discover decision operations, then query the company's decisions and relevant knowledge. Events retain their baseline, situation, business objectives, self-contained options, risks and assumptions. Each company has at most one unfinished decision. In simulation, the GM may establish clearly fictional company backgrounds, products, customer personas, events and response assumptions without requiring real contacts or world-building answers every turn. External factual market claims still need evidence. Simulated actions, responses and revenue assumptions remain identified and never update real company records.

A player's selection saves an option identity and decision revision as a readable intent. It does not update the ledger, goals, simulation date or business records. The game shows that it is waiting for the Agent and allows the user to copy the intent into an external conversation. Copying has no delivery guarantee, and deployment credentials grant no payment, communication or other external permissions.

Claim the current decision version and reread the actual saved selection and receipt. Complete authorized real operations or explicit simulated actions, publish their results, then resolve with exact record identities and revisions. Simulation results include a turn; real results use reports or actual records allowed by the contract. Results belong to the same company and were committed after selection. Historical outcomes use the referenced version. A saved choice may change before claim; after claim, coordinate stopping, cancel and prepare a new event to change plans.

If relevant business resources change, read current records and revise an open event as the conflict requires. Ordinary reports and unrelated knowledge changes do not invalidate an event. Data revisions are system-maintained history and change references; Agents never increment them. Omitted event baselines, report/forecast versions, turn after_revision and settlement snapshots resolve inside the transaction. A turn derives before_revision only from a same-company selected/processing decision_id; otherwise supply it. Explicit historical versions are validated. Updates retain record-level revision checks.

Prepare the next event after closing the current one. Restore and migration retain events, choices and result history, but require new remote credentials. Existing v0.1 companies without decisions keep their accounts, turns and reports; an Agent prepares the first new event explicitly.

Scheduling, model calls and external business actions belong to the user's Agent client. Owner authorization for this deployment does not expand external permissions.
