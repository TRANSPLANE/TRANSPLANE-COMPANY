# Plugin interface v1

The owner installs a fixed version. Enabled contributions appear in the deployment's discovery index. Executable plugins run as trusted local processes under the service's operating system account; the platform does not provide an operating system sandbox or a scheduler.

## Manifest and installation

A manifest contains `schema_version: 1`, `api_version: 1`, a stable `id`, `name`, `version`, `author`, `description`, `permissions`, `contributions` and a `data_policy`. Private configuration is passed separately. Public catalog output excludes configuration and execution commands.

The same version and identical manifest can update configuration. A different installed version or changed manifest is rejected; export retained data, disable and uninstall before explicitly installing its replacement.

Examples:

- [Local backup destination](../examples/plugins/local-backup.json), using the built-in adapter.
- [Process backup destination](../examples/plugins/process-backup.json) and [Python implementation](../examples/plugins/backup_destination.py).
- [Business summary](../examples/plugins/business-summary.json) and [Python implementation](../examples/plugins/business_summary.py), using a temporary scoped callback credential and saved output for metrics and a view.

```bash
./company plugin install --manifest examples/plugins/local-backup.json --config examples/plugins/backup-config.json
./company plugin enable local-backup
./company backup
./company plugin deliver-backup --plugin local-backup --snapshot <snapshot-file>
```

Execution commands are an executable-and-arguments array, started directly without a shell. Set the example script argument to an absolute path when the service starts from another directory. Python examples explicitly use `-X utf8`. The base company image contains no Python; add your chosen runtime and script to your own image, or use the built-in local backup adapter.

## Process contracts

Stdin and stdout use UTF-8 JSON. A backup destination receives one JSON line containing `snapshot` (an absolute path) and `config`. Success exits with code zero and returns `{"status":"delivered","path":"...","sha256":"..."}`. Failures and timeouts retain the original local snapshot. The sample copies through a temporary file and verifies its checksum before replacement.

An operation receives `company_id`, `request_id`, `input`, `config`, `actor` and `api`. The `api` object supplies `base_url`, `company_id`, `scopes` and a temporary `token`. The token expires after 120 seconds and is revoked after execution; never persist or print it. Configure `api_base`, or set `COMPANY_PUBLIC_ORIGIN`, for a non-default deployment address.

Output is a JSON object. Optional `operations` use the standard batch format; the platform validates scope, assigns stable child request IDs and commits atomically. Management, cross-company and recursive plugin operations are forbidden.

Metrics declare `id`, `label`, a dotted `value_path`, `format` and optional `unit`. Views declare `id`, `title` and field declarations. Formats are `number`, `money`, `percent` or `text`. They display saved operation output, rather than executing arbitrary frontend code.

## Business Skill documents

`contributions.skills` declares guidance-only documents without starting a runtime process. Each item contains `id`, `name`, `title`, `description` and Markdown `body`; optional fields include `enabled`, `tags`, `modes` (`real` and/or `simulation`) and `type` (`procedure` or `experience`). See the [operating decision review example](../examples/plugins/operating-skills.json). Plugin guidance must not reference private company records. Its text grants no execution or data-access permissions.

After installation, documents appear in company Skill catalogs and on the Plugins page. Disabled plugin documents remain readable and copyable but have `effective=false`; Agents must not treat them as active guidance. Copies become independently maintained company documents, with their text and revisions included in backups. Copy creation checks the source revision and digest and stores read-only `copied_from`, including the complete original template and plugin version. That snapshot remains available after the source plugin is uninstalled or replaced. See [Hosted business Skills](hosted-skills.en.md) for read, maintenance and experience-capture rules.

## Unknown results and data exit

Request states are `pending`, `unknown` and `completed`. Keep the original ID. Declare `runtime.idempotent: true` only when the plugin safely deduplicates its external effects by that ID; the read-only summary sample is safe to repeat.

The owner queries `GET /api/v1/admin/plugins/<id>/requests/<request-id>?company_id=...`. After independently confirming an uncertain non-idempotent outcome, reconcile with `POST /api/v1/admin/plugins/<id>/reconcile`, passing `company_id`, `request_id` and `result: {input_hash,output}`. The CLI provides `plugin request` and `plugin reconcile` equivalents.

Disabling retains data. To remove stored plugin results, disable the plugin, create a complete backup containing the current results and verify its SHA256. Then use `POST /api/v1/admin/plugins/<id>/data-exit` with `company_id`, `snapshot`, `expected_sha256`, or `company plugin data-exit <id> --company <id> --snapshot <path> --sha256 <checksum>`. The platform compares current rows against the snapshot before removing them. Company extension files and external destinations are preserved; handle them through an explicit separate exit plan.
