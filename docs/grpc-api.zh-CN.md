# gRPC 接口参考

[English](grpc-api.md) · [服务接入入口](grpc/README.zh-CN.md) · [快速开始](grpc/quickstart.zh-CN.md) · [语言客户端](grpc/clients.zh-CN.md)

`knot.v1.ConfigurationService` 是无会话存储的商品配置服务。四个 RPC 均为 unary；每次传入精确模型身份和必要的完整状态。本文描述网络契约，前端演示见[单独教程](../examples/grpc/README.zh-CN.md)。

## 何时调用哪个接口

| RPC | 业务时机 | 状态影响 |
|---|---|---|
| Initial | 开始新配置 | 返回 revision 0 的空答案 State；不创建服务端会话 |
| Apply | 用户提交、清空或跳过答案 | 校验 Event，协调依赖答案，返回完整新 State 和 Snapshot |
| Evaluate | 恢复已保存配置、重新计算视图 | 校验已有 State，不修复、不增加 revision，Changes 为空 |
| Quote | 初始展示、答案提交成功后、切换问题或预览候选值 | 返回完整商品价格范围和候选整机价；不修改 State |

### Initial

输入只有 `model_digest`。成功返回 TransitionResult。应用保存其中的 `state`，用 `snapshot.flow` 和 `snapshot.visual` 渲染流程与视觉，然后对该 State 调用 Quote 得到价格区间。不要在每次点击时调用 Initial，它会从头开始。

### Apply

输入为 `model_digest`、完整 `state_json` 和一个 `event_json`。Event 的 `expectedRevision` 必须等于所传 State 的 revision。`set_answer` 写入答案；multi 类型传完整选项集合而不是追加一个选项。`clear_answer` 清除答案，`skip_answer` 跳过允许跳过的问题；字段见 [Event Schema](../contracts/schemas/event.schema.json)。

一次合法修改可能清理依赖于旧答案的后续答案，必须整体替换返回的 State，不能只把这次点击合并进本地答案。实际 no-op 不增加 revision。领域错误不提供替换状态。不要手改 revision、answers 或 skipped 来绕过 Apply。

### Evaluate

传入完整已保存 State，恢复 Snapshot；不需要 Event。它不会补齐错误状态或做模型迁移，也不会将 revision 加一。恢复失败时保留原存档并向用户报告错误，不能静默 Initial 后覆盖存档。

### Quote

外层为 `model_digest` 和 `quote_json`；内部 JSON 为 `schemaVersion`、完整 `state`，以及可选 `focusQuestionId` / `candidateValue`。未给 focus 时使用引擎当前问题；指定 focus 可查看之前的问题，不产生导航 Event。数字/颜色候选预览需显式 focus，预览不提交答案。成功响应是独立的 QuoteResult，不是 TransitionResult；完整价格语义见下文。

## 调用及其载荷

协议定义见 [configuration.proto](../contracts/grpc/knot/v1/configuration.proto)。可据此生成你的语言客户端，也可直接参考 Go 示例。所有方法均为 unary RPC，完整路径是 `/knot.v1.ConfigurationService/<方法名>`。

| 方法 | 请求字段 | 成功载荷 |
|---|---|---|
| Initial | `model_digest` | 完整初始 TransitionResult，revision 为 0 |
| Apply | `model_digest`、`state_json`、`event_json` | 完整新 State、Snapshot 和 Changes |
| Evaluate | `model_digest`、`state_json` | `result_json`：重算 Snapshot，不增加 revision |
| Quote | `model_digest`、`quote_json` | 独立 `quote_json`：完整配置区间、焦点和候选整机价 |

所有请求字段都必须存在。proto 的 `optional` 用于检测缺字段，不表示业务上可以省略。

- `model_digest`：字符串，格式为 `sha256:[0-9a-f]{64}`，必须命中启动时已加载的模型。
- `state_json`：完整 State 的 UTF-8 JSON 字节，不是整个 TransitionResult，也不是 Snapshot。
- `event_json`：完整 Event 的 UTF-8 JSON 字节。

生成客户端的 bytes 字段直接传 JSON 字节，不要额外包一层 JSON 字符串，也不要手动 Base64。调试工具若采用 [ProtoJSON](https://protobuf.dev/programming-guides/json/)，会把 bytes 显示成 Base64；这是工具的表示方式，不是 protobuf 字节内部再编码一次。

音箱的第一步选择是这个完整 Event：

<!-- schema: event expect: valid -->
```json
{"schemaVersion":"1.0","type":"set_answer","questionId":"speaker_type","value":"active","expectedRevision":0}
```

1. 调用 Initial，解析 `result_json`，保留整个 `state`，显示 `snapshot.flow.currentQuestion`。
2. 根据用户选择构造 Event，将刚收到的 State 的 `revision` 放到 `expectedRevision`。
3. 调用 Apply，传同一 digest、完整 State 和 Event。保留 JSON 数字精度，不要用会丢精度的浮点对象中转。
4. 成功后，用返回值整体替换 State 和 Snapshot，立即更新流程和 Visual，再调用 Quote 显示整机报价。Changes 解释发生了哪些变化，不能代替完整 State。
5. 恢复已保存的配置时，用原模型 digest 和完整 State 调用 Evaluate。它重算 Snapshot，但不增加 revision，也不会修复非法 State。

结果遵循 [result.schema.json](../contracts/schemas/result.schema.json)，包含 `schemaVersion`、`state`、`snapshot`、`changes`、`diagnostics`。输入参见 [state.schema.json](../contracts/schemas/state.schema.json) 和 [event.schema.json](../contracts/schemas/event.schema.json)。金额是整数最小货币单位：CNY 的 `282800` 是 ¥2,828.00。`estimated` 是已答部分的小计，不是完成配置的报价；即使是 `final` 也不代表支付授权。视觉结果只有资源引用、颜色、显隐等属性，服务不渲染图片。

## 结果判断和状态保存

响应的 `outcome` 必须恰有一个已知分支：

- `result_json`：Initial、Apply、Evaluate 业务成功，解析完整 TransitionResult。
- `quote_json`：Quote 成功，解析独立 QuoteResult。
- `error_json`：业务失败，解析完整 `{schemaVersion,error}` 信封；按 `error.code` 分支，不匹配 message 文本。结构见 [error.schema.json](../contracts/schemas/error.schema.json)。

**gRPC OK 不等于业务成功。** 缺 outcome 或 JSON 不符合契约也是失败。任何失败都保留旧 State，不保存空结果或部分结果。

| 情况 | gRPC 结果 |
|---|---|
| 缺外层字段、digest 或 protobuf 格式非法 | INVALID_ARGUMENT |
| digest 格式正确但模型未部署 | NOT_FOUND |
| 网络消息或接纳容量超限 | RESOURCE_EXHAUSTED |
| deadline 到期 | DEADLINE_EXCEEDED |
| 服务正在排空退出 | UNAVAILABLE |
| 内部故障或捕获到 panic | INTERNAL；进程退出服务 |
| 领域校验、revision、规则或内部资源错误 | OK，响应为完整 `error_json` |

只有业务成功后，才用应用数据库的 CAS/事务保存完整新 State。`expectedRevision` 只检查你传入的 State；服务没有会话数据库，无法判断数据库里是否已有别人提交的新版本。两个请求拿同一个旧 State 调用 Apply，都可能成功，最终写入冲突须由应用处理。重试必须遵循调用方策略，并保留原 digest、State、Event；不要承诺 exactly-once。

内部 JSON 上限：Product 4 MiB、State 512 KiB、Event 64 KiB。业务 protobuf 消息上限：入站 1 MiB，出站 4 MiB + 1 KiB。deadline 不会强行中断已运行的同步计算，计算槽要等工作结束才释放。

使用 mTLS 时，按[语言客户端](grpc/clients.zh-CN.md#远程连接与校验)配置身份，endpoint 主机名必须匹配服务端 SAN。[部署文档](deployment.md) 说明服务端证书、健康检查和退出方式。

标准健康服务为 `grpc.health.v1.Health`，业务服务名为 `knot.v1.ConfigurationService`。没有 reflection、REST、模型上传、模型列表或远程管理 API。


## 完整配置报价 Quote

`QuoteRequest.quote_json` 传原始 UTF-8 [请求 JSON](../contracts/schemas/quote-request.schema.json)：必填 `schemaVersion`、完整 `state`，可选 `focusQuestionId`、`candidateValue`。显式 null 非法；具体候选预览只允许显式指定的颜色/整数/小数题。独立 `QuoteResponse` 的 oneof 是 `quote_json` 或既有 `error_json`，需校验[返回 Schema](../contracts/schemas/quote-result.schema.json)，空或未知分支是协议错误。

主 `price` 固定已有答案和 skip，覆盖所有合法完整配置，包含尚未选择的必选成本：

| 状态 | 含义 |
|---|---|
| `range` | 精确可达的 `minMinor` / `maxMinor`；未完成时即使相等仍为区间 |
| `final` | 输入配置已完成，两端等于最终整机价 |
| `no_completion` | 已穷尽且没有合法完成方式；金额均为 null |
| `unavailable` | 无法在域/搜索预算内证明精确范围；金额均为 null，原因 `domain_limit` 或 `search_limit` |

不能在不可报价或请求失败时回退显示旧小计。真实模型/规则错误仍返回错误，不当作零元或跳过分支。

`focus` 返回题目、当前答案、skip、候选及可选 preview。单选候选是选它；多选候选是切换该成员后的完整集合。候选 `price` 表示**选择后整机总价**，包括协调清理与剩余必选成本，不能标成固定加价。禁用项不报价；非法候选带完整 `error` 信封；候选合法但无后继是 `no_completion`。

Quote 保留输入 revision，不代表下单授权。界面按 modelDigest、revision、焦点和请求序号匹配响应；导航只换焦点，Apply 串行，成功后立即用 Visual 更新图层，再等待报价。取消 fetch 不代表服务端计算已经停止。

上限：请求 768 KiB、内嵌 State 512 KiB、输出 4 MiB、显式候选 256、搜索节点 10,000、全请求共享 2,000,000 计算成本。数值题合法格点超过 10,000 时不可报价；颜色按可证明的条件等价类处理。剩余预算不足以完整编码响应时，整个调用返回 `RESOURCE_LIMIT`。

未修改的音箱模型：初始完整区间 ¥1,199–¥3,027；有源/胡桃木 ¥1,599–¥2,327；有源/铝合金/200W/LED 完成价 ¥2,828。这些是模型验证示例，不能写成客户端计价规则。
