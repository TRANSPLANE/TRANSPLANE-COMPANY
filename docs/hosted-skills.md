# 部署内经营 Skills

Agent 可以从所选部署直接发现并读取经营 Skill，不必安装到本地。目录只返回名称、用途、来源、版本和适用模式；完整正文按当前任务读取。公司经营快照不会一次展开所有 Skill 正文。

## 读取

| 用途 | HTTP |
|---|---|
| 内置模板目录 | `GET /api/v1/skills` |
| 公司可用目录 | `GET /api/v1/companies/<company-id>/skills` |
| 完整文档与元数据 | `GET /api/v1/companies/<company-id>/skills/<skill-id>` |
| 原始 SKILL.md | `GET /api/v1/companies/<company-id>/skills/<skill-id>/document` |
| 固定版本 | 在文档请求中添加 `?revision=<n>` |
| 公司 Skill 历史 | `GET /api/v1/companies/<company-id>/skills/<skill-id>/history` |

公司目录和文档使用 `company.read` 权限。内置模板目录不包含私人公司信息。HTTP 用 `Accept-Language` 读取中文或英文的内置模板；公司和插件自己编写的内容保持原文。

公司目录添加 `?include_deleted=true` 可查看已删除文档的元数据。完整读取返回 `skill` 元数据及其 `body`，另有生成后的 `document` 字符串。

本地 CLI 提供等价读取：

```powershell
.\company.exe --data-dir C:\CompanyData skills
.\company.exe --data-dir C:\CompanyData skills --company my-company
.\company.exe --data-dir C:\CompanyData skills --company my-company --id cashflow-check --document
.\company.exe --data-dir C:\CompanyData skills --company my-company --id cashflow-check --history
```

内置文档标识使用 `builtin:<name>`；插件文档使用 `plugin:<plugin-id>:<skill-id>`。从目录返回的 `read_url` 和 `document_url` 读取，不根据标题猜测地址。四个内置模板覆盖客户访谈、现金流复盘、周期复盘和模拟公司主持。

只将 `enabled=true`、`effective=true` 且未删除的 Skill 用作当前经营规范。停用或不适用于当前公司模式的文档仍可查阅，但不能将其描述成正在生效的能力。Skill 不会运行代码，也不授予外部行动或数据访问权限。

## 维护

“插件”页中的“经营 Skills”提供新建、预览、编辑、启停、删除和恢复。内置和插件文档是只读模板，可以复制为公司自己的 Skill 后修改。修改范围限定当前公司；公司 Skill 正文与版本保存在数据库主账。

远程修改需要明确的 `skills.write`，普通 `company.write` 不包含该权限。主人可在授予 Agent 凭据时勾选“允许维护经营 Skills”。本地 CLI 依赖操作系统访问权限。

写入操作与其他经营记录共用回执和冲突检查：

| 操作 | 内容 |
|---|---|
| `skill.put` | 新建、修订，或恢复完整文档 |
| `skill.set_enabled` | 启用或停用 |
| `skill.delete` | 从正常目录删除，保留历史 |

先从能力发现读取精确合同。新建无需 `expected_revision`；修改使用该 Skill 最新记录版本。准确重试保持请求标识及输入一致，冲突后读取最新正文并合并，不覆盖别人的更新。

`skill.put` 的正文放在 `body`，不重复填写 YAML 元数据头。服务从 `name`、`description` 和 Markdown 正文生成完整 `SKILL.md`。支持标题、标签、`procedure`／`experience` 类型、真实／模拟适用范围，以及准确来源引用。

```json
{
  "request_id": "save-cashflow-check-1",
  "input": {
    "id": "cashflow-check",
    "name": "cashflow-check",
    "title": "现金流核对",
    "description": "核对每周收付款与来源，识别重复或缺失记录。",
    "body": "先读取本周收付款和对应来源，再核对日期、账户与金额。无法解释的差异保持待核对状态。",
    "enabled": true,
    "type": "procedure",
    "tags": ["现金流", "核对"],
    "modes": ["real", "simulation"]
  }
}
```

名称使用少于 64 字符的小写字母、数字和连字符，正文上限 128 KiB。目录元数据和原始文档提供内容摘要；历史读取固定所选版本。删除和停用保留正文及历史，恢复使用完整 `skill.put` 和最新版本。

### 复制模板与出处

从内置或插件模板新建公司 Skill 时，在 `skill.put` 的 `input` 中附上 `copy_from`。复制同时需要当前公司的 `company.read` 与 `skills.write`。先读取来源的完整文档，再使用读取结果的版本和摘要；不要从标题猜测来源，或自行填写旧模板的摘要。

| `copy_from` 字段 | 输入 |
|---|---|
| `id` | 来源的 `builtin:<name>` 或 `plugin:<plugin-id>:<skill-id>` |
| `revision` | 来源文档返回的版本 |
| `content_sha256` | 来源文档返回的 SHA256 摘要 |
| `language` | 本次读取的 `zh-CN` 或 `en`；内置正文按此语言核对 |

新公司 Skill 使用自己的 `id`、名称、用途和 `body`，正文可以按公司情况调整。`copy_from` 只用于新建；修订、启停和恢复已有公司文档时不重复提交。普通原创文档省略此字段。

服务在提交事务中核对当前来源的版本及完整 `SKILL.md` 摘要，变化或卸载返回 `revision_conflict`。保留未提交的草稿，重新读取模板，再明确选择合并来源变化并使用新的 `copy_from`；输入改变时换用新请求标识。也可以由用户明确选择另存为独立文档并省略 `copy_from`，不能伪造复制来源。

成功后服务生成只读 `copied_from`，包含来源 `id`、`source`、`revision`、`language`、`content_sha256` 和原始完整 `document`；插件来源另含 `plugin_version`。公司目录和历史目录只返回其轻量元数据；完整文档读取，包括 `?revision=<n>`，返回原始模板文档快照。摘要对应快照 UTF-8 字节的 SHA256，不能把公司后来修改的正文当作原始来源。写入时不接受客户端设置或改写 `copied_from`。

后续修订保留该出处，历史、删除、恢复、备份和迁移均保留原始模板快照。停用、卸载或更换来源插件不会删除公司副本或其出处，也不会自动更新副本正文；可以在插件页展开来源文档核对。

内置模板正文随产品发行更新，按完整文档的 `content_sha256` 识别变化；即使当前模板 `revision` 相同，也不能复用旧摘要新建副本。已保存的公司副本和 `copied_from.document` 保留复制当时内容，不随内置模板更新。

## 经验沉淀

用户要求总结并长期保存时，Agent 可以创建 `experience` 类型的 Skill。规范应包含使用情境、所需输入、判断方法、建议做法、不适用条件及复核触发点。已有经营证据可以通过 `source_refs` 关联 `{kind,id,revision}`，只能引用当前公司已存在的准确版本。

区分一次事件与可迁移经验。模拟结果可以支持模拟主持或实验设计经验，不能直接作为真实市场规律；客户、行业或阶段特定的结论保留适用范围。没有新增可复用判断时，保留现有规范即可。用户没有要求持久学习时，将本次结论保留在经营报告中。

访问凭据、秘密和不必要的个人信息不写入规范。Skill 不替代当前部署的操作合同或用户授权。备份、导出与恢复包含公司 Skill 正文、修订、删除状态和来源；恢复后重新授予远程凭据。从真实公司建立模拟分支时，保留原始出处说明，不建立跨公司悬空来源引用。

## 插件文档

插件可在 `contributions.skills` 声明文档，不需要执行进程。示例见 [operating-skills.json](../examples/plugins/operating-skills.json)。停用插件后文档仍可查看和复制，但不再生效；公司自己的副本独立维护，并保留复制时的原始模板快照。插件安装、启停和卸载仍由主人管理。
