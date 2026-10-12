# Agent 接口

## 渐进式发现

| 层级 | HTTP | 本地 CLI |
|---|---|---|
| 入口 | `GET /api/v1/agent-entry` | `company discover` |
| 服务索引 | `GET /api/v1/discovery` | `company discover` |
| 服务操作 | `GET /api/v1/discovery?view=operations&service=<服务>` | `company discover --view operations --service <服务>` |
| 精确合同 | `GET /api/v1/discovery?view=contract&operation=<操作>` | `company discover --view contract --operation <操作>` |
| 任务引导 | `GET /api/v1/agent-guide?topic=<主题>` | `company guide --topic <主题>` |

从当前任务需要的合同开始读取。部署返回实际能力、权限、输入、错误与示例，详细字段以部署为准。入口 Skill 见 [SKILL.md](../skills/transplane-company/SKILL.md)。

HTTP 语言通过 `Accept-Language: zh-CN` 或 `lang=zh-CN` 选择；查询参数优先于请求头。英文使用 `en`，`en-US` 归到英文，`zh` 归到简体中文。CLI 对应全局 `--lang zh-CN` 或 `COMPANY_LANG=zh-CN`，只改变引导及错误文字，不改变经营数据。

## 部署、建档与首次游玩

部署完成后向用户提供可访问的前端入口及安全登录方式。凭据单独交付，不放入公司文档、Skills、截图或带凭据的页面链接。说明用户可以新建真实公司、从零开始模拟，或继续已有公司；没有真实业务也能从模拟开局。

读取 `connect` 与 `company` 引导，取得业务方向、可用资源、投入时间和近期目标，核对币种、时区与起始日期。模拟开局可提供少量题材与资源建议，由用户确认后保存。真实经营没有依据的余额、客户或收入保持未知。

界面新建公司后，交接内容携带公司标识。Agent 先读取这个公司，沿用其模式与基础设定，再准备资源、目标路线与首个事件，不重复创建。如果通过聊天建档，使用主人的明确建档授权及创建合同；普通公司 Agent 凭据不能创建新公司或分支。

`POST /api/v1/companies` 接收 `company.create` 请求封装，必须使用主人凭据。返回回执的 `company.id` 是后续维护对象，不能以名称猜测标识。准确重试保持 `request_id` 与整个输入不变；创建的请求标识在部署级空间中使用，另一家公司应使用新标识。`name`、`currency`、`timezone`、`opening_date` 为必填；显式选择 `mode`，可带 `description`。当前部署合同给出其余可选字段。

建档后继续建立近期可反馈的结果目标和阶段路线，并提交一个可独立理解的首事件。向用户呈现处境、方案、风险和反馈时间，说明可以说出方案文字或提出自己的计划，在需要其选择时等待；只创建公司或只返回页面链接不算完成开局。模拟主持读取 `builtin:simulation-gm`，逐步接续承诺、关系和经营后果；“继续”不能替玩家选择尚未选择的路线。

每次开局、事件、结算、继续、状态和意向交接回复前，必须读取并应用入口的 `player_output.policy` 或当前 `play` 的 `player_output_policy` 完整呈现合同。`required` 表示默认玩家呈现必须遵守，`layout`、`steps` 与 `preflight` 分别提供回复布局、表达规则和发送前检查。旧部署没有内嵌合同时读取 `/api/v1/agent-guide?topic=player-output`，CLI 对应 `company guide --topic player-output`。场景、人物台词、结果和下一选择构成主回复，省略没有内容的部分；回执、版本、原始标识和维护过程只在明确查询、调试或实际失败恢复时展开。开局说明模式，影响当前取舍的投入、期限、风险与关键未知仍自然呈现，不反复插入模拟免责声明。只有已保存并结算的结果才表述为已结算。

重要模拟客户和伙伴使用稳定自然名或有意义的角色称谓，保留身份、动机、顾虑及历史关系，用少量自然台词或具体行动形成取舍。即使旧报告、当前事件和方案仍出现 `V5`、“样本 A”等占位号，也只用作内部查找键，不能照抄到玩家叙事、在人名旁附旧号或宣布别名维护。接续旧存档先读取报告、回访和已有别名，优先将内部标识与显示称呼映射保存到获准维护的 `context/cast.md` 或等价知识并同步复用；不修改研究假设、指标、历史结果或已保存选择，不为命名新建客户。缺少姓名或保存授权时用已知角色称谓继续。自然方案按当前 `option_id` 映射提交，不改变原承诺。玩家明确选定的代号、实际型号与产品版本仍保留；真实公司只用实际或获准称呼，不虚构身份、台词和反馈。

模式在创建后固定。真实公司进入模拟由主人调用该公司下的 `simulation.fork`，使用当前已核对状态并保留来源基线，不原地更改模式，也不继承 Agent 凭据。返回真实经营时选择来源公司或新建真实公司；没有来源的纯模拟公司不能直接变成真实公司。查看与切换公司不迁移经营结果。

## 查询和提交

入口的 `player_output.policy` 与 `company.play` 的 `player_output_policy` 内嵌完整合同，内容与 `player-output` 引导来自同一定义，按当前语言返回。入口保留 `guide`、`read_before_play` 和简短 `instruction`，`play` 保留 `player_output_guide` 与 `player_output_instruction`，供旧客户端接续。Agent 每次组装玩家文字时应用完整合同，不能只保留链接，也不能把旧存档、维护指令或工具结果直接当作回复。`technical_details=on_explicit_request` 允许玩家主动展开标识、版本、来源和账目；它不改变经营记录和权限校验。

需要登录的请求使用 `Authorization: Bearer <凭据>`。主人通过本地 CLI 或前端授予、撤销按公司和能力限定的 Agent 凭据。查询 `GET /api/v1/session` 核对身份和权限，不将凭据写入公司文件。

| 用途 | HTTP |
|---|---|
| 可访问公司 | `GET /api/v1/companies` |
| 主人新建公司 | `POST /api/v1/companies` |
| 公司总览 | `GET /api/v1/companies/<id>/snapshot` |
| 历史基线 | `GET /api/v1/companies/<id>/history/<revision>` |
| 记录与历史 | `GET /api/v1/companies/<id>/records?kind=<类型>&history=true&offset=0` |
| 确定性指标 | `GET /api/v1/companies/<id>/metrics?as_of=YYYY-MM-DD` |
| 公司操作 | `POST /api/v1/companies/<id>/operations/<操作>` |
| 建档等全局操作 | `POST /api/v1/operations/<操作>` |
| 原子导入 | `POST /api/v1/companies/<id>/imports` |
| 登记文件 | `POST /api/v1/companies/<id>/files/sync` |
| 下载登记文件 | `GET /api/v1/companies/<id>/files/<文件标识>` |
| 安全文件预览 | `GET /api/v1/companies/<id>/files/<文件标识>/preview?revision=<版本>` |

单个操作的请求封装：

```json
{
  "request_id": "agent-owned-stable-request-id",
  "expected_revision": 1,
  "input": {}
}
```

首次创建无需 `expected_revision`；更新已有记录需读取当前记录版本并提交。精确重试保持整个请求一致；输入改变使用新请求标识。冲突返回错误，禁止用覆盖重试消除冲突。

记录查询每页最多 500 条，返回 `total`、`has_more` 和 `next_offset`。继续读取下一页，不把首批记录误当完整公司档案。总览的资金流水为最近一批，全部流水从记录查询逐页读取；CLI 对应 `query --offset <值>`。

文件导入使用封装或等价操作数组：

```json
{
  "operations": [
    {"operation": "account.put", "request": {"request_id": "account-create-1", "input": {}}}
  ]
}
```

`input` 使用精确合同中的字段。导入最多 1,000 个操作，失败不提交整个批次；CLI 与 API 使用相同业务路径。金额是公司币种的整数最小单位。

`opening_date` 与账户 `opening_on` 表示记录和期初余额的起点。新部署若没有完整旧账，使用已核实余额的日期建账，历史经营背景放入公司资料，避免用更早日期暗示已有完整资金记录。

## 资料与复盘

预览读取已登记的不可变内容版本，权限为 `files.read`。Markdown 和 UTF-8 文本直接返回正文；图片及 PDF 使用同一公司的准确版本内容地址，读取时仍携带凭据。Markdown 不执行原始 HTML；HTML、SVG、格式无效或超限文件保留下载。文本上限 1 MiB，图片与 PDF 上限 10 MiB。界面能直接查看当前文件和所选历史版本，读取不会创建新记录。

在 `companies/<id>/` 内编辑资料，再用相对路径显式同步。文件版本保留摘要和关联；不能读取其他公司的文件，也不能提交目录外路径。

预测记录包含基准、数据版本、假设和情景；周报/月报包含固定期间、截止点和版本。路线调整保存新版本。`real` 和 `simulation` 公司分离：模拟公司可提交回合，真实公司建立分支需要主人权限。模拟结果、资金和成果始终保存在独立公司。

## 部署内经营 Skills

本地安装是可选的。Agent 从 `/api/v1/agent-entry` 的 `skills` 字段读取目录及引导地址，先按用途选择元数据，再展开所需正文。公司目录、完整文档、历史、原始 SKILL.md 和写入字段见[经营 Skills](hosted-skills.md)。

读取公司 Skill 使用 `company.read`；修改需要明确的 `skills.write`。内置和插件文档是只读模板，可以复制为公司独立规范。规范与经验同样保留历史、幂等和版本检查；仅在用户要求沉淀经验时持久保存，不把一次模拟结果写成未经验证的真实市场规则。

Agent 对话支持自由表达，游戏主界面保存当前事件、选择和结果。经营判断依据公司资料和现实行业数据：保留来源 URL、观察日期、事实与假设，计算输入、方法与结果可核对。回合的 `research` 使用 `{title,url,observed_on,claim}` 项，`calculation` 保存 `{inputs,method,result}`；具体格式按当前部署合同读取。模拟的虚构启动资源不替代现实经营依据。

模拟主持按需读取 `builtin:simulation-gm`，根据公司阶段、资源、模拟日期、未结事项和历史选择主动准备事件。驱动可以是相关且有来源的现实变化，也可以是明确模拟的客户、竞争、机会、故障、成本、伙伴或期限；说明为何此刻发生、前次选择如何导致处境、每项方案的投入与经营取舍。按相关性和信息时效性补充现实来源，不要求每回合联网；无法取得数据时明确假设，不能伪造 URL 或把后来信息当作当时已知事实。

回合结果区分即时变化和延迟后果。待回访客户、承诺、期限、未兑现后果及来源事件或回合标识保存在公司资料或回合叙事中；文件修改后显式同步，后续事件重新读取并接续，不能只依赖当前聊天记忆。成功失败须有已说明的风险、资源约束或模拟响应依据，定量达成与保存指标一致。先结束当前事件，再准备有意义的下一取舍；不在状态查询时自动制造事件或要求玩家补写世界。

模拟日期使用 `simulation_date`，通过 `simulation.advance` 携带当前公司版本显式推进，不能倒退。回合的 `after_revision` 使用当前已提交版本，定量目标成功与该版本的进度一致。报告和预测保存固定基线的 `metrics_snapshot`，后续修改不覆盖当时依据。

## 对话游玩与前端呈现

| 用途 | HTTP | CLI |
|---|---|---|
| 当前事件、方案、意向、处理阶段、结算、资金与目标 | `GET /api/v1/companies/<id>/play` | `company play --company <id>` |
| 模块链接与截图服务配置状态 | `GET /api/v1/companies/<id>/presentation?view=<view>` | 使用 `play` 返回的链接 |
| 实际前端模块截图 | `POST /api/v1/companies/<id>/presentation/capture`，正文 `{"view":"overview"}` | 通过网络部署调用 |

以上接口使用该公司的 `company.read` 权限。`play` 为只读查询；`data_revision` 由系统生成，查询不会增加版本。`current_event` 保留决策自身版本及完整方案；`selection_commands` 可直接准备选择提交，`claim_command` 提供未领取意向的处理入口。`latest_result` 固定引用结果记录与当时资金、目标快照；当前 `state` 随公司变化更新，两者不混用。

阶段分别为 `awaiting_choice`（待选择）、`awaiting_agent`（已保存、可重选）、`agent_processing`（已领取处理）、`awaiting_event`（等待下一事件）。用户在对话中说出方案文字或标识，Agent 用 `decision.select` 保存；用户说“继续”，先读取当前阶段再给出选择、处理已选方案、查看结果或准备下一事件，不替玩家决定尚未选择的经营方向。

真实经营缺少必要依据时向用户索取实际资料或最小必要的数据来源授权与连接凭据，不能用虚构情景代替实际财务、客户或业务数据。秘密保存在 Agent 的安全凭据存储或专用配置；不得出现在公司资料、报告、导出、截图或链接。截图按请求返回，分享和发布需单独授权。

开放页面为 `overview`、`operations`、`growth`、`simulation`、`reports`、`files`、`chronicle`、`plugins`。返回链接采用 `/?company=<id>#<view>`，附到用户选择的服务地址；链接不包含凭据，打开后由用户正常登录。

截图服务是可选的可信本地进程，配置见[部署文档](operations.md#可选前端截图)。配置状态只说明渲染器已配置，不表示截图已验收。成功返回 `image.mime_type`、`image.base64`、尺寸、实际截图的 `data_revision` 和对应 `captured_state`；`changed_since_capture` 标明当前状态是否已更新。Agent 可将 PNG 展示给玩家，同时给出快捷链接。未配置返回 HTTP 503 `screenshot_unavailable`，渲染失败返回 HTTP 502 `screenshot_failed`，忙碌返回 HTTP 503 `screenshot_busy`，均保留 `play` 状态和链接；继续文字游玩，不伪造图片。

## 经营决策与交接

| 操作 | 输入用途 | 权限 |
|---|---|---|
| `decision.put` | 创建或修订未选择的事件；保存 `baseline_revision`、处境、方案及相关目标 | 已授权公司的经营写入能力 |
| `decision.select` | 用 `id` 及精确方案 `choice`（标题或标识）、`option_id`，或允许的自由方案 `text` 保存/重选意向 | 已授权公司的经营写入能力 |
| `decision.claim` | 领取最新保存意向，固定选择并记录处理者 | 已授权公司的经营写入能力 |
| `decision.resolve` | 已领取事件用 `result_refs` 和 `summary` 关联固定结果，可选固定 `company_revision` | 已授权公司的经营写入能力 |
| `decision.cancel` | 用 `id`、`reason` 取消未结束事件，保留选择和历史 | 已授权公司的经营写入能力 |

更新封装携带决策自身的 `expected_revision`，不要将其与公司 `baseline_revision` 混淆。以上写入均使用 `company.write`；只读凭据不能保存选择。每个结果引用为 `{kind,id,revision}`。方案使用 `title`，可选 `estimated_cost_minor` 表示预计投入；该数值不自动入账。详细限制和错误码以当前部署精确合同为准。

保存、重选、领取后的下一次更新复用该回执 `record.revision`；无需反复读取公司版本。推演回合填写 `decision_id` 后，服务可推导选择前基线；当前回合 `after_revision`、报告/预测数据版本及结算公司快照均可省略，服务确定并校验。需要指定历史快照时按合同显式传入。

从当前部署发现经营决策操作，再读取该公司的决策记录及相关公司知识。事件保存基线、当前处境、经营目标、自足的方案、风险和假设。每家公司最多存在一个未结束的事件。模拟公司可由 Agent 主动设计明确虚构的公司背景、产品、客户画像、事件及客户响应假设；不要求玩家先提供现实客户或每项世界细节。外部市场事实仍需来源，模拟行动、客户反馈和收益假设明确标识，不写入真实公司。

玩家提交选择时，保存方案标识和决策版本，得到可重读的选择意向；选择不修改账目、目标、模拟日期或业务记录。界面显示等待 Agent，允许用户复制意向到外部对话。复制没有送达保证，平台凭据也不授予支付、通信或其他外部行动权限。

Agent 使用当前决策版本领取，再重读实际保存意向与领取回执。随后按授权完成现实操作或明确虚构的模拟操作，发布结果，再用精确结果标识和版本结束决策。模拟公司的结果须含模拟回合，真实公司的结果可引用报告及合同允许的实际记录。结果须在选择后提交且属于同一公司；历史结果重读被引用版本，不随报告修订跳到当前版本。未领取前可用最新决策版本重选；领取后固定方案，需要改变时协调停止、取消并建立新事件。

公司相关经营资源在事件准备后改变时，按冲突提示读取最新记录并修订未选事件。普通报告及未关联资料的修改不使事件自动失效。公司版本供查询、追溯与变化检测，不要求 Agent 手动递增；事件基线、报告/预测数据版本、回合 after_revision 和结算快照省略时由服务事务确定，回合 before_revision 仅在填写本公司已选择或处理中的 decision_id 时可推导，否则必须提供；显式历史版本仍校验。更新记录使用它自身的 `expected_revision`。

结束事件后再准备下一项经营选择。公司恢复或迁移保留事件、选择和结果历史，远程授权仍须重新建立。已有 v0.1 公司没有决策记录时可以继续查看原账目、回合和报告，由 Agent 显式准备新事件。

授权、插件、备份及恢复是主人管理能力。Agent 任务调度、外部工具和现实行动由客户端负责，平台授权不自动授予这些权限。
