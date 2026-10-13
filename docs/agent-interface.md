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

读取 `connect` 与 `company` 引导，复用已有设定，将缺少的业务方向、可用资源、投入时间和一个近期目标合成一次简短确认，一并核对币种、时区与起始日期。没有方向时提供两三个具体模拟题材或成套资源建议，用户确认后保存，不重复询问已有答案。真实经营没有依据的余额、客户、动机或收入保持未知。

界面新建公司后，交接内容携带公司标识。Agent 先读取这个公司，沿用其模式与基础设定，再准备资源、目标路线与首个事件，不重复创建。如果通过聊天建档，使用主人的明确建档授权及创建合同；普通公司 Agent 凭据不能创建新公司或分支。

`POST /api/v1/companies` 接收 `company.create` 请求封装，必须使用主人凭据。返回回执的 `company.id` 是后续维护对象，不能以名称猜测标识。准确重试保持 `request_id` 与整个输入不变；创建的请求标识在部署级空间中使用，另一家公司应使用新标识。`name`、`currency`、`timezone`、`opening_date` 为必填；显式选择 `mode`，可带 `description`。当前部署合同给出其余可选字段。

确认后继续建立目标路线和自足首事件，不停在公司档案或网页链接。模拟主持读取 `builtin:simulation-gm`，默认以预计 3—5 回合得到反馈的一个近期目标开局：具体人物带来诱因，有限资源迫使价值取舍，选择前说明投入、约束、风险及反馈时间。后续按玩家行动返回反馈、回访承诺并阶段收束，不预定胜负。提供两三条自然路线和自由计划入口，在玩家决定处等待；“继续”不能替其选择尚未选择的路线。

每次开局、事件、结算、继续、状态和意向交接回复前，应用入口的 `player_output.policy` 或当前 `play` 的 `player_output_policy` 完整呈现合同。`required` 表示默认呈现必须遵守，`layout`、`steps` 与 `preflight` 提供布局、表达规则和发送前检查。旧部署读取 `/api/v1/agent-guide?topic=player-output`，CLI 对应 `company guide --topic player-output`。默认以短场景、必要状态、自然台词或行动、因果后果和下一选择回复，省略空段；玩家询问建议或理由时给简短军师分析，详细来源、计算、回执、版本和维护过程按查询、调试或实际失败恢复展开。开局说明模式，投入、期限、风险和关键未知放在对应信息处，不每回合重复免责声明。只有已保存并结算的结果才称已结算，等待、失败和真实未知如实保留。

重要模拟人物使用稳定自然名或有意义的角色称谓，保留身份、顾虑与历史关系。旧研究占位号如 `V5`、“样本 A”只作内部查找键；接续前读取报告、回访及已有别名，优先在获准 `context/cast.md` 或等价知识保存映射并同步，不改写研究、指标、历史结果或选择，也不为命名新建客户。默认不宣告别名维护或附旧号；无保存授权时用已知角色称谓继续。尊重玩家命名、刻意匿名、代号剧情及实际型号和版本。真实人物只使用实际或获准称呼、记录中的行动与反馈；台词引用原话，没有原话则概述记录。

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

## 四周工作室接口

四周工作室与 Agent 自由经营共用公司档案，但使用不同结算流程。它只在以 CNY 记账的空白模拟公司开局；主人先明确确认 ¥6,000 启动现金、每周 20 小时、每周 ¥800 开销和四周周期。规则由系统计算，不需要额外模型，Agent 可以提供建议或解释分配，不能修改产能、期限、付款或结果。

先读取 `GET /api/v1/discovery?view=operations&service=studio`、所选操作的精确合同及 `GET /api/v1/agent-guide?topic=studio`。HTTP 专用地址位于 `/api/v1/companies/<id>`：

| 操作 | 地址与输入 |
|---|---|
| `studio.state` | `GET /studio`；读取规则、当前篇章版本、订单、选项及确定预览 |
| `studio.start` | `POST /studio/start`，`{request_id,expected_company_revision}`；确认后初始化篇章 |
| `studio.preview` | `POST /studio/preview`，`{run_id,expected_run_revision,allocations}`；只读预览，不结算 |
| `studio.choose` | `POST /studio/choose`，`{request_id,run_id,expected_run_revision,option_id}` 或用 `allocations` 代替 `option_id`；两者只选一个 |

`run_id` 为 `studio-v1`，`expected_run_revision` 使用 `studio.state` 的 `run.revision`。内置方案标识是 `deliver`、`product`、`balance`；自由分配为 `[{target,hours}]`，对象是 `product`、`atlas`、`birch`、`cedar`，工时为正偶数，合计不超过 20，不能超过剩余工作量，也不能分给未开放或已关闭订单。空数组允许整周闲置，仍扣开销。完整规则与四周示例见[四周工作室](studio.zh-CN.md)。

读取与预览使用 `company.read`；开局和结算需要 `company.write`、`finance.write`、`reports.write`、`goals.write`。普通操作入口与 CLI 使用 `OperationRequest`：开局 `expected_revision` 为公司版本且 `input={}`；结算为篇章版本，`input` 含 `run_id` 及唯一的 `option_id` 或 `allocations`。专用 HTTP 的字段映射由部署合同给出。

`studio.choose` 原子保存选择、工时、资金流水、回合、精确结果引用和下一周事件，不等待模型裁定，不另行执行自由经营的记录及结算循环。重读成功回执中的 `result` 与 `studio` 呈现结果；响应丢失时以原请求标识和完全相同输入重试。版本冲突先读取当前篇章，改变输入则换请求标识。

自由计划先转为分配并预览，得到玩家确认再结算。第四周结束后停止规则回合，`run.week=5` 不代表可提交第五周；总结以保存的三项目标判断为准。玩家可以暂停、新建公司重玩，或明确选择 Agent 自由经营的下一方向，不能自动设立目标或增加成果。

## 持续连接外部宿主

Codex、OpenClaw 或远程 Agent 可以使用同一接收协议，具体模型与客户端仍由用户自己的宿主运行。先授予已有的公司范围 `company.read` 与 `company.write` 凭据，再由外部进程连接。连接不会创建凭据、改变公司版本或代替用户选择；没有持续接收钩子的桌面聊天仍需下面的外部适配器，不能仅凭聊天窗口开启就认为已经连接。

以下地址均位于 `/api/v1/companies/<id>`：

| 地址 | 用途 |
|---|---|
| `POST /agent-link/connect` | `{label,lease_seconds:60,session_id?}`；首次连接返回 `session_id`，携带该标识续租，时限最大 120 秒 |
| `GET /agent-link/pending-work` | `session_id` 必填，可带 `after_revision`、`after_request_key` 与 `wait_ms`（最大 25000）；返回当前 `play`、`link` 与可为空的 `work` |
| `POST /agent-link/ack` | `{session_id,request_key,receipt_id}`；明确确认接收当前已发出的待办 |
| `POST /agent-link/disconnect` | `{session_id}`；断开连接，不取消已领取决策或外部行动 |
| `GET /agent-link` | 查询连接、接收与当前领取状态；只需 `company.read` |

`work.kind` 为 `process_selection` 或 `prepare_event`，并携带稳定 `request_key` 和本次发出的 `receipt_id`。仅查询待办还未确认送达：没有宿主时需人工交接，有连接的未接收待办为 `queued`，显式 ACK 后为 `delivered`，业务 `decision.claim` 后才是 `processing`。ACK 不领取决策、不扣款、不结算；改选后旧回执不能确认新意向。连接和 ACK 是运行时状态，不写公司主账；服务重启或租约到期需重新连接。

仓库与发行包的 `scripts/agent-host.py` 是只用 Python 标准库的外部适配器，默认持续监听；`--once` 检查一次并至多处理一份当前任务。用户在宿主安全环境提供 `COMPANY_AGENT_TOKEN`，命令只指定服务、公司、显示名称和处理程序：

```powershell
python scripts/agent-host.py --service-url http://127.0.0.1:4177 --company <company-id> --label "My Agent" --handler-argv '["python","my-company-handler.py"]'
```

将 `--handler-argv` 换为已配置的宿主处理程序参数数组。适配器以 `shell=False` 启动它，任务只通过标准输入传递：`{service_url,company_id,kind,request_key,play,policy}`；凭据只通过环境交给处理程序，不进入 URL、任务正文或日志。Windows `.cmd` 处理程序需要显式的 `cmd.exe` 适配参数，不能把任务内容拼成命令。处理程序输出不会直接转发，适配器只显示连接、接收及有服务状态依据的完成状态。

处理程序按权威 `play` 与当前操作合同读取已保存意向，领取、执行获准行动、提交结果并用精确引用结算；可以用原子导入批量提交结果与下一事件。工作室事件采用上一节的 `studio.choose` 流程，不重复手工裁定。`prepare_event` 根据已有资料准备下一幕或方向选择，阶段已闭合时不能自动替玩家设定新目标或创造成果。退出码为零不足以证明完成：处理选择须在服务中找到该决策已结算的精确结果，准备事件须看到新的权威事件。

适配器独立续租，慢模型处理期间仍保持连接；本地接收记录只保存任务标识哈希与状态，默认位于当前用户的 `.transplane-company/agent-host/`，可用 `--state-file` 指定。连续轮询与进程重启均不自动重放已接收任务。超时、异常、授权撤销或没有可验证进展时停止，主人先核对实际领取与保存状态，再决定如何恢复；不要为重试直接删除接收记录。

等待期间可查看已知场景资料、模拟设定或实际原件概述，也可准备尚未发布的素材或草稿；不能预写未发生的客户回应作为事实。本地暂停仅暂停阅读，不取消已领取的宿主行动，不向人物发送消息，也不改主账。按 Ctrl+C 断开适配器，已领取状态保留供核对；真实付款、通信和其他外部行动授权仍由宿主主人管理。

## 资料与复盘

预览读取已登记的不可变内容版本，权限为 `files.read`。Markdown 和 UTF-8 文本直接返回正文；图片及 PDF 使用同一公司的准确版本内容地址，读取时仍携带凭据。Markdown 不执行原始 HTML；HTML、SVG、格式无效或超限文件保留下载。文本上限 1 MiB，图片与 PDF 上限 10 MiB。界面能直接查看当前文件和所选历史版本，读取不会创建新记录。

在 `companies/<id>/` 内编辑资料，再用相对路径显式同步。文件版本保留摘要和关联；不能读取其他公司的文件，也不能提交目录外路径。

经营预测由本地 Agent 依据公司实际记录、用户资料及获准检索的最新公共信息形成，不依赖 WORLD。预测或关联资料保留来源、观察时间、基准快照、数据版本、假设和情景；缺少必要资料时索取实际数据或最小来源授权，也可在已有资料与明确假设下继续推演，不补造实际记录。周报/月报固定期间、截止点和版本，路线调整保存新版本。`real` 和 `simulation` 公司分离，模拟结果、资金和成果保存在独立公司。

未来 WORLD 适配器可提供独立 Agent 视角与互动信号，当前只保留可选集成边界，尚未实现在线连接；现有入口、合同和引导用于独立公司服务。取得获准信号后标明其来源与观察时间，模拟 Agent 反应仍作为情景假设，不作为现实客户反馈。评估增益时以相同公司快照、预测窗口和指标口径分别保留本地与附加信号预测，再对照同一截止点的实际结果；只有现实对照证据支持时才判断预测质量提高。

## 部署内经营 Skills

本地安装是可选的。Agent 从 `/api/v1/agent-entry` 的 `skills` 字段读取目录及引导地址，先按用途选择元数据，再展开所需正文。公司目录、完整文档、历史、原始 SKILL.md 和写入字段见[经营 Skills](hosted-skills.md)。

读取公司 Skill 使用 `company.read`；修改需要明确的 `skills.write`。内置和插件文档是只读模板，可以复制为公司独立规范。规范与经验同样保留历史、幂等和版本检查；仅在用户要求沉淀经验时持久保存，不把一次模拟结果写成未经验证的真实市场规则。

Agent 对话支持自由表达，游戏主界面保存当前事件、选择和结果。经营判断保留公司资料、现实来源 URL、观察日期、事实与假设，金额有可核对的输入、方法和结果。回合 `research` 使用 `{title,url,observed_on,claim}`，`calculation` 保存 `{inputs,method,result}`；具体格式以当前合同为准。现实来源按相关性和时效补充，不要求每轮联网；观察日期与模拟日期分开，后来信息不能成为当时已知事实。模拟客户响应和收益采用明确假设，不冒充市场证据。

模拟裁定在选择前固定约束、响应假设和方法，保留可调查的悬念，选择后按实际行动判定，不事后改成本、限制或胜负规则。后果区分即时变化、未结承诺和延迟影响，保存到下述叙事结构或获准公司知识，文件修改后同步。成功和失败均接续已有资产、关系及证据，阶段收束提供下一目标、转向或退出选择，不用随机惩罚或无依据奖励替代经营因果。

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

### 事件体验、后果与故事上下文

`decision.put` 可选提交 `experience`。旧事件无需补写；新事件使用它时，`options` 为 2—4 项，每项必须有 `benefit` 与 `tradeoff`，可带 `commitment` 和 `feedback_on`。这些字段说明收益、机会成本、承诺及反馈日期，不替代 `risk`、`assumptions` 或实际经营记录。通常启用 `allow_freeform`，自然回复映射到当前 `option_id`，自由计划按同一约束保存与裁定。

| `experience` 字段 | 内容 |
|---|---|
| `scene` | 当前具体场景及本幕体验主题 |
| `why_now` | 此刻发生的原因及与先前选择的因果 |
| `stakes` | 当前得失、有限资源与无法兼得的价值 |
| `cast` | `[{id,name,role,concern?,line?}]`；稳定人物标识、自然称呼、身份，可带顾虑和台词 |
| `callbacks` | 可选 `[{decision_id,summary}]`；只回调本公司已结算的历史决策 |
| `chapter` | 可选 `{id,title,turn,planned_turns}`；短篇标识、标题、当前幕和计划回合数，通常 3—5 回合，可随选择调整 |

模拟回合或报告可选保存 `aftermath`，此时必须用 `decision_id` 关联本公司已选择、处理中或已结算的决策。模拟结果随回合保存，真实结果随报告及实际记录保存。人物台词、关系变化和承诺状态有相应依据，真实模式不从模拟剧本生成它们。

| `aftermath` 字段 | 内容 |
|---|---|
| `immediate` | 可选 `[string]`，已经发生的即时后果 |
| `reactions` | 可选 `[{cast_id,line}]`，人物反应 |
| `relationships` | 可选 `[{cast_id,change}]`，有行动依据的关系变化，不自动改变资金、客户或指标 |
| `commitments` | 可选 `[{id,title,cast_id?,due_on?,status,source_decision_id}]`，新增或变化的承诺；`status` 为 `pending`、`fulfilled`、`broken` 或 `cancelled` |
| `chapter` | 可选 `{id,status,summary?,takeaway?}`；`status` 为 `ongoing` 或 `closed`，收束时提交结局 `summary` 与本次取舍的 `takeaway` |

承诺 `id` 跨回合保持稳定，按增量提交，不必重复全部清单；`source_decision_id` 始终指向最初来源，不能在回访时改成当前事件。兑现、破约与取消依据既有行动或结果，到期只触发核对，不能直接当作已发生。短篇 `closed` 不等于目标成功，目标结果仍按保存指标和完成依据核对；`takeaway` 不是未经验证的真实市场规律。

`company.play` 返回 `story_context`：`recent_choices`、`relationships` 和 `open_commitments` 来自已结算决策的精确结果引用；`cast` 与 `chapter` 同时接续当前事件的已保存设定。`source_ref` 保留来源版本，供 Agent 回访人物、承诺与下一幕。`presentation` 提供当前事件体验与方案，`cadence` 提供短篇及下一节点。旧记录没有叙事结构时返回空集合或 `null`，可从获准公司知识补充已知背景，真实未知仍保持未知。它们是故事投影，不授予支付、联络或其他外部行动权限。字段用于维护和前端呈现，默认玩家回复使用人物台词、经营因果和下一选择。

从当前部署发现经营决策操作，再读取该公司的决策记录及相关公司知识。事件保存基线、当前处境、经营目标、自足的方案、风险和假设。每家公司最多存在一个未结束的事件。模拟公司可由 Agent 主动设计明确虚构的公司背景、产品、客户画像、事件及客户响应假设；不要求玩家先提供现实客户或每项世界细节。外部市场事实仍需来源，模拟行动、客户反馈和收益假设明确标识，不写入真实公司。

玩家提交选择时，保存方案标识和决策版本，得到可重读的选择意向；选择不修改账目、目标、模拟日期或业务记录。界面显示等待 Agent，允许用户复制意向到外部对话。复制没有送达保证，平台凭据也不授予支付、通信或其他外部行动权限。

Agent 使用当前决策版本领取，再重读实际保存意向与领取回执。随后按授权完成现实操作或明确虚构的模拟操作，发布结果，再用精确结果标识和版本结束决策。模拟公司的结果须含模拟回合，真实公司的结果可引用报告及合同允许的实际记录。结果须在选择后提交且属于同一公司；历史结果重读被引用版本，不随报告修订跳到当前版本。未领取前可用最新决策版本重选；领取后固定方案，需要改变时协调停止、取消并建立新事件。

公司相关经营资源在事件准备后改变时，按冲突提示读取最新记录并修订未选事件。普通报告及未关联资料的修改不使事件自动失效。公司版本供查询、追溯与变化检测，不要求 Agent 手动递增；事件基线、报告/预测数据版本、回合 after_revision 和结算快照省略时由服务事务确定，回合 before_revision 仅在填写本公司已选择或处理中的 decision_id 时可推导，否则必须提供；显式历史版本仍校验。更新记录使用它自身的 `expected_revision`。

结束事件后再准备下一项经营选择。公司恢复或迁移保留事件、选择和结果历史，远程授权仍须重新建立。已有 v0.1 公司没有决策记录时可以继续查看原账目、回合和报告，由 Agent 显式准备新事件。

授权、插件、备份及恢复是主人管理能力。Agent 任务调度、外部工具和现实行动由客户端负责，平台授权不自动授予这些权限。

工作室已有自然语言自由选择时，先预览结构化分配并取得确认，再以 `allocations` 和准确的已保存 `selection_text` 提交 `studio.choose`。这样保留原意并关联转译方案；选择变化或由其他主体领取时返回冲突。

`forecast.put` 支持带日期的 `context_sources`（公司资料、公共信息、WORLD 互动、模拟假设），以及 `analysis_method` 和 `limitations`。来源保存标题、引用、观察日期、获取日期及可选失效日期；过期资料保留原状态。观察日期≤获取日期≤预测 `as_of`。来源与结果对照规范通过 `/api/v1/agent-guide?topic=prediction` 按需读取。
