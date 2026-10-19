# gRPC 调用接口

[English](grpc-api.md)

用 `knot.v1.ConfigurationService` 驱动商品配置流程：展示下一题、提交用户选择，获得新的配置状态、价格和视觉属性。服务无会话存储，与原生 SDK 使用同一内核。界面、会话保存和支付流程由你的应用负责。

## 先跑一个真实配置

### 带前端的 SVG 音箱定制

[启动脚本](../examples/grpc/speaker_web.py)会启动真实 gRPC 服务和本机 Go HTTP 转接服务。浏览器用底座、箱体、面板、灯环四个 SVG 图层展示音箱；价格、选项可用性、答案自动清除和图层属性全部来自 KNOT。

准备 Docker Linux 容器、Python 3.9+、Go 1.26.4，并加载内部提供的镜像。在仓库根目录运行：

```powershell
$image = Read-Host 'Local image tag with Quote support'
$out = Join-Path $env:TEMP ('knot-speaker-web-' + [guid]::NewGuid().ToString('N'))
python examples/grpc/speaker_web.py --image $image --out $out --port 8765
```

打开 `http://127.0.0.1:8765`，依次选择有源、铝合金、面板颜色、200W、LED。选择通过 Apply 立即更新 SVG，随后 Quote 返回完整配置区间及各选项的选择后整机价。上一项/下一项只改变焦点，保留选择和 revision。返回改成胡桃木时，不兼容答案被清除、灯环熄灭。刷新先 Evaluate 保存的 State，再 Quote；恢复失败不会覆盖存档。

在启动终端按 Ctrl+C 停止页面服务并移除容器，日志保留在 `$out`。每次使用新的输出目录；`$image` 必须为包含 Quote 的配套镜像，旧三方法镜像不能支持此页面。[页面和 SVG 源码](../examples/grpc/go/cmd/speaker-web/)修改后重新启动脚本。

HTTP 转接只属于这个本机示例，不是 KNOT 服务新增的公共 REST 接口。示例用 localStorage 保存配置，不包含账号、共享数据库或支付功能；请使用非敏感演示数据，实际产品需要自行实现会话和授权。

### 脚本场景

[音箱示例脚本](../examples/grpc/speaker_demo.py) 使用已有的[商品模型](../contracts/examples/speaker.json)、真实容器和[生成的 Go 客户端](../examples/grpc/go/main.go)。它运行三个购买/回改场景及一个非法选项场景，保存完整返回值供你检查。

需要：支持 Linux 容器的 Docker、Python 3.9+、Go 1.26.4（见 [go.mod](../examples/grpc/go/go.mod)）。首次构建客户端需要下载 Go 依赖，不需要 Rust、protoc 或原生 SDK。用 `docker load --input <归档文件>` 导入内部提供的镜像，镜像名以导入输出为准。

在仓库根目录运行 PowerShell：

```powershell
$image = Read-Host 'Imported local image tag'
$out = Join-Path $env:TEMP ('knot-speaker-' + [guid]::NewGuid().ToString('N'))
python examples/grpc/speaker_demo.py --image $image --out $out
Get-Content (Join-Path $out 'summary.json')
```

脚本会构建 Go 客户端，只挂载 `speaker.json`，在本机回环地址的空闲端口启动只读容器，检查健康并运行场景，结束后删除自己的容器。结果保留在 `$out`；每次使用新目录。本机演示显式开启明文连接；远程部署使用 [mTLS](deployment.md)。

| 场景 | 选择和业务行为 | 预期结果 |
|---|---|---|
| `premium`：高配有源音箱 | 有源 → 铝合金 → 蓝色面板 → 200W → LED 灯环 | ¥2,828.00，`totalMinor=282800`，价格 `final`，流程 `complete`，灯环可见 |
| `change-material`：买家回改材质 | 在上面的完整配置上，把铝合金改成胡桃木 | ¥1,599.00，`159900`，价格退回 `estimated`；自动清掉 `power` 和 `extras`，隐藏灯环，重新询问功率 |
| `passive`：另一条购买路线 | 无源 → 胡桃木 → 蓝色面板 → 香蕉插头 | ¥1,199.00，`119900`，价格 `final`，流程 `complete`；不询问功率和配件 |
| `disabled-power`：拒绝不合法组合 | 有源 → 胡桃木 → 蓝色面板 → 尝试选 200W | 返回 `INVALID_OPTION`；失败请求不提供可替换的 State |

脚本保留旧 `snapshot.price` 的累计金额语义，`estimated` 是小计，不能作为浏览器完整配置报价。脚本会断言这些小计/最终金额、revision 和流程状态。每个成功场景还会调用 Evaluate，确认 State 不变；随后重放旧事件，确认收到 `REVISION_CONFLICT`。

查看 `*.events.json` 看提交内容，`*.result.json` 看最终结果，`*.trace.jsonl` 看每一步 Initial/Apply/Evaluate 的完整返回。在 `change-material.trace.jsonl` 中，最后一个成功 Apply 的 Changes 包含自动清理记录；Evaluate 的 Changes 为空。轨迹包含完整答案，仅用于本地非敏感演示数据。

想改选项，可以编辑生成的事件文件，再对自己启动的服务运行客户端：

```powershell
& (Join-Path $out 'grpc-example.exe') --plaintext --trace `
  --endpoint 127.0.0.1:50051 `
  --digest sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541 `
  --events (Join-Path $out 'premium.events.json')
```

第二条命令需要你另行启动服务，并填入它的实际地址；示例脚本的临时服务已经停止。上面的 digest 对应未修改的音箱模型，并非 JSON 文件字节的 SHA-256。其他模型须使用配套模型清单中的编译后 digest。客户端将最终结果写到 stdout，可选的逐次 RPC 轨迹写到 stderr。它每次从新配置开始，并测试旧 revision 拒绝，不负责保存会话。

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

使用 mTLS 时，Go 客户端去掉 `--plaintext`，提供 `--ca`、`--cert`、`--key`、`--server-name`，使用真实客户端身份并校验服务端 SAN。[部署文档](deployment.md) 说明服务端证书、健康检查和退出方式。

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
