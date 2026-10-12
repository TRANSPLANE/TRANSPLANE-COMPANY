# Hosted business Skills

Agents discover and read business Skills from the selected deployment without installing them locally. Catalogs contain metadata only; complete guidance is loaded for the current task. Company snapshots do not eagerly expand every Skill body.

Public templates are listed at `GET /api/v1/skills`. Company-private catalogs use `GET /api/v1/companies/<company-id>/skills`. Read a document and its metadata at `.../skills/<skill-id>`, raw SKILL.md at `.../skills/<skill-id>/document`, an exact version with `?revision=<n>`, or company history at `.../skills/<skill-id>/history`. Use the catalog's returned URLs rather than deriving addresses from titles.

Catalogs return names, purposes, sources, revisions and applicable modes. Use `?include_deleted=true` on a company catalog to inspect deleted company documents. Full reads return `skill` metadata with its `body`, plus the generated `document` string.

Company reads require `company.read`. Built-in IDs use `builtin:<name>`; plugin IDs use `plugin:<plugin-id>:<skill-id>`. The four built-in templates cover customer interviews, cash flow, periodic review and simulation hosting. Accept-Language selects Chinese or English templates; company-authored and plugin-authored content remains unchanged.

The CLI provides the same read path:

```powershell
.\company.exe --data-dir C:\CompanyData skills
.\company.exe --data-dir C:\CompanyData skills --company my-company
.\company.exe --data-dir C:\CompanyData skills --company my-company --id cashflow-check --document
.\company.exe --data-dir C:\CompanyData skills --company my-company --id cashflow-check --history
```

Apply only enabled, effective, non-deleted guidance for the company mode. Inactive documents remain readable for inspection. A Skill does not execute code, replace operation contracts, or grant access or external-action permission.

## Maintenance

The Plugins page includes business Skill creation, preview, editing, toggling, deletion and restoration. Built-in and plugin documents are read-only templates; copy one into an independent company Skill to edit it. Canonical company guidance and its revisions reside in the database.

Remote writes require explicit `skills.write`; `company.write` does not include it. The owner can select “Allow business Skill maintenance” when granting credentials. Local CLI access relies on operating-system permissions.

Use `skill.put` to create, revise or restore a complete document, `skill.set_enabled` to toggle it, and `skill.delete` to hide it while retaining history. Discover the current exact contract. Updates use that Skill's latest expected_revision; exact retries retain the request ID and input. Merge conflicts rather than overwriting updates.

Put Markdown guidance in `body`, without a duplicate YAML header. The service generates complete SKILL.md from name, description and body. Fields include title, tags, procedure/experience type, real/simulation modes and exact source references. Names use lowercase letters, digits and hyphens with fewer than 64 characters; body is limited to 128 KiB. Document digests correspond to returned content. Deletion and disabling preserve content and revisions; restoration submits full content with the latest revision.

Submit writes to the company's standard operation route. For example, create a procedure with `skill.put`:

```json
{
  "request_id": "save-cashflow-check-1",
  "input": {
    "id": "cashflow-check",
    "name": "cashflow-check",
    "title": "Cash flow check",
    "description": "Reconcile weekly receipts and payments with their sources and identify missing or duplicate records.",
    "body": "Read the week's receipts, payments and supporting sources. Reconcile dates, accounts and amounts. Keep unexplained differences pending review.",
    "enabled": true,
    "type": "procedure",
    "tags": ["cash-flow", "reconciliation"],
    "modes": ["real", "simulation"]
  }
}
```

### Copying templates and preserving origin

When creating a company Skill from a built-in or plugin template, include `copy_from` in the `skill.put` input. Copying requires both `company.read` and `skills.write` for the current company. Read the complete source first and use its returned revision and digest.

| `copy_from` field | Value |
|---|---|
| `id` | Source `builtin:<name>` or `plugin:<plugin-id>:<skill-id>` |
| `revision` | Revision returned by the source read |
| `content_sha256` | SHA256 digest returned by the source read |
| `language` | `zh-CN` or `en` used for that read; built-in content is checked in this language |

The new company Skill has its own `id`, name, purpose and `body`; adapt the text to the company when needed. `copy_from` is accepted only on creation. Omit it for original documents and for subsequent edits, toggles or restoration.

The service checks the current source revision and complete SKILL.md digest in the transaction. A changed or uninstalled source returns `revision_conflict`. Preserve the draft, read the latest template and explicitly merge the change before submitting a new `copy_from`. Changed input requires a new request ID. The user can instead explicitly save an independent document without `copy_from`; do not invent an origin.

After a successful copy, the service returns read-only `copied_from`: source `id`, `source`, `revision`, `language`, `content_sha256` and the complete original `document`. Plugin origins also contain `plugin_version`. Catalogs and history lists expose origin metadata without the document. Full document reads, including `?revision=<n>`, contain the original template snapshot. Its SHA256 is calculated from that snapshot's UTF-8 bytes. Later company edits do not change the original source. Clients cannot set or rewrite `copied_from`.

Edits, history, deletion, restoration, backup and migration retain this snapshot. Disabling, uninstalling or replacing the source plugin does not remove the independent company copy or its origin, and does not automatically update its text. The Plugins page can expand the original source document for comparison.

Built-in text may change with a product release. Identify changes with the complete document's `content_sha256`; an unchanged current template `revision` does not make an old digest valid for a new copy. Saved company copies and their `copied_from.document` retain the original text independently of built-in updates.

## Durable learning

Only when the user requests persistent learning, an Agent may create or revise an experience Skill. Describe its applicable context, inputs, judgment criteria, actions, exclusions and review triggers. Supporting business records can be linked using exact same-company source_refs entries `{kind,id,revision}`.

Separate one observed event from a transferable procedure. Simulated results may inform hosting or experimental design, but do not validate real market behavior. Retain customer-, industry- or stage-specific conditions. Leave a useful procedure unchanged when no new judgment is supported; otherwise retain ordinary conclusions in a report when durable learning was not requested.

Exclude credentials, secrets and unnecessary personal information. Guidance preserves current authorization boundaries. Backups and exports retain bodies, revisions, deleted states and source evidence; remote credentials must be granted again after restoration. Simulation forks retain origin information without dangling cross-company evidence references.

Plugins can declare guidance-only `contributions.skills` without a runtime process. See [operating-skills.json](../examples/plugins/operating-skills.json). Disabling a plugin makes its guidance ineffective but keeps it readable and copyable; company copies remain independent and retain the original template snapshot. Installation, enabling and removal remain owner operations.
