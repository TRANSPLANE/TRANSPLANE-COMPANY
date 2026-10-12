# 插件接口 v1

插件由主人安装固定版本，按当前部署的发现接口暴露贡献；不自动运行定时任务。首版提供声明式经营模板、指标、展示、数据接入及备份目的地的扩展位置，独立进程用于执行型贡献。相同版本和相同 Manifest 可以更新配置；已安装其他版本或不同内容时拒绝替换，需先处理保留数据、停用并卸载，再显式安装所选新版本。

## Manifest 与配置

Manifest 包含 `schema_version: 1`、稳定 `id`、`name`、`version`、`author`、`description`、`api_version: 1`、`permissions`、`contributions` 和数据退出说明 `data_policy`。私有配置由安装操作传入，公开目录不返回配置或执行命令。

备份例子：

- [local-backup.json](../examples/plugins/local-backup.json)：内置本地复制适配器。
- [process-backup.json](../examples/plugins/process-backup.json)：独立进程适配器。
- [backup_destination.py](../examples/plugins/backup_destination.py)：校验、写临时副本、原子替换。
- [backup-config.json](../examples/plugins/backup-config.json)：所选本地目的地，可改为绝对路径。

```powershell
.\company.exe plugin install --manifest examples/plugins/local-backup.json --config examples/plugins/backup-config.json
.\company.exe plugin enable local-backup
.\company.exe backup
.\company.exe plugin deliver-backup --plugin local-backup --snapshot <快照文件>
.\company.exe plugin disable local-backup
```

进程例子需要服务环境中可用的 Python 3。基础容器只包含公司程序；容器中使用该例子时需在自己的镜像中加入 Python 和脚本，或使用内置本地备份适配器。相对脚本路径针对项目根目录；服务若从其他工作目录启动，安装前将 `runtime.command` 的脚本参数改为绝对路径。命令是可执行文件和参数数组，平台直接启动进程，不经过 shell。使用服务操作系统账户的权限运行，安装前确认来源。

## 备份进程合同

进程合同使用 UTF-8 编码的 stdin/stdout JSON。Python 示例显式设置 `-X utf8`，避免 Windows 系统代码页改变中文或其他 Unicode 字节。内核先生成本地一致快照，再向 stdin 写一行 JSON：

```json
{"snapshot":"/absolute/snapshot.tpcompany.json","config":{"destination":"/chosen/path"}}
```

进程成功退出并在 stdout 返回一个 JSON 对象：

```json
{"status":"delivered","path":"/chosen/path/snapshot.tpcompany.json","sha256":"file-sha256"}
```

错误返回非零退出码和 `status: failed`。运行超时按失败处理；本地原快照保留。例子不会修改原文件，复制前后校验摘要，不含网络传送或任务调度。

## 经营能力与展示贡献

[业务概况 Manifest](../examples/plugins/business-summary.json) 与 [进程例子](../examples/plugins/business_summary.py) 演示只读能力：通过平台临时凭据读取公司快照，返回付费客户、现金流、日期和版本，再由声明式指标及看板呈现。安装前将脚本路径改为服务环境中的绝对路径；非默认端口的配置设置 `api_base`，或通过 `COMPANY_PUBLIC_ORIGIN` 设置所选部署入口。

执行型贡献的输入包含 `company_id`、`request_id`、`input`、`config`、`actor` 和 `api`。`api` 提供 `base_url`、公司、所授权范围和临时 `token`；凭据有效期为 120 秒，执行结束撤销。插件不能将凭据持久化或输出。

进程返回普通 JSON。可选 `operations` 为标准批次，由平台校验权限、生成稳定子请求键并原子提交，不允许管理操作、跨公司或递归调用。`metrics` 项声明 `id`、`label`、`value_path`、`format`、可选 `unit`；`views` 声明 `id`、`title` 与字段数组。格式为 `number`、`money`、`percent` 或 `text`，字段路径按点号读取实际保存结果。

## 未确认执行结果

平台保存 `pending`、`unknown`、`completed` 状态。保持原请求标识查询，不因超时换键重复执行。只有已经保证自身外部副作用按键去重的插件才声明 `runtime.idempotent: true`；只读概况例子可以安全重读。

非幂等插件的未知结果由主人核对，再调用 `POST /api/v1/admin/plugins/<id>/reconcile`，提交 `company_id`、原 `request_id`、`result: {input_hash,output}`。请求状态从 `GET /api/v1/admin/plugins/<id>/requests/<request_id>?company_id=...` 读取；CLI 提供 `plugin request` 与 `plugin reconcile`。

## 经营 Skill 文档

`contributions.skills` 可以声明只包含元数据和 Markdown 正文的经营文档，无需运行进程。每项包含 `id`、`name`、`title`、`description` 和 `body`，可附标签、经营模式和规范／经验类型。示例见[经营决策复盘插件](../examples/plugins/operating-skills.json)。插件文档不得引用私人公司记录，正文不会提供执行权限。

安装后，文档出现在公司 Skills 目录和插件页中；停用后仍可查看或复制，但 `effective=false`，Agent 不应继续当作启用规范。复制出的公司文档独立维护，正文和修订进入备份。接口及经验沉淀规则见[经营 Skills](hosted-skills.md)。

## 数据与退出

停用保留配置和已有数据。卸载前按 Manifest 的退出策略处理保留数据，清除需主人显式选择。插件可以用 `companies/<id>/extensions/<plugin-id>/` 托管公司资料，但结构化写入仍通过领域接口。备份和迁移应保留这些文件、版本及插件登记，不能依赖运行插件才能导出。

结构化插件结果退出需先停用插件、生成包含当前结果的完整备份，核对快照 SHA256，再执行主人接口 `POST /api/v1/admin/plugins/<id>/data-exit`，输入 `company_id`、`snapshot`、`expected_sha256`。CLI 对应 `plugin data-exit <id> --company <id> --snapshot <路径> --sha256 <摘要>`。平台确认当前每条保留结果都在该快照中后，才删除对应结构化结果；公司文件仍保留，外部目的地不受此操作影响。
