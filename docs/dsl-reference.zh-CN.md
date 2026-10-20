# 商品模型 DSL 1.0

[接入体系总览](integration-overview.zh-CN.md) · [gRPC 接口](grpc-api.zh-CN.md) · [完整音箱模型](../contracts/examples/speaker.json)

KNOT 的商品模型是一个 JSON 文件，描述“可以问什么、选项何时可选、下一步问什么、如何计价、如何生成视觉属性”。它不包含用户当前选择，也不包含页面组件或订单。一个模型可以服务很多用户；每个用户的选择保存在独立的 State 中。

本说明解释 DSL 1.0 的字段及执行语义。[product.schema.json](../contracts/schemas/product.schema.json) 是字段结构的精确依据；编译器还会校验引用、类型、依赖和流程环路，因此通过 JSON Schema 不等于模型可用。JSON 不允许注释、尾随逗号、重复键或未声明字段。

## 1. 先看整体结构

| 顶层字段 | 必填 | 用途 |
|---|---|---|
| `schemaVersion` | 是 | DSL 格式版本，目前固定为 `"1.0"` |
| `productId` | 是 | 商品身份，例如 `speaker-pro` |
| `productVersion` | 是 | 模型作者维护的版本字符串，例如 `"1.0.0"` |
| `questions` | 是 | 所有问题、类型及选项 |
| `flow` | 是 | 起点和有序分支，决定实际经过哪些问题 |
| `pricing` | 是 | 币种、基础价和调整项 |
| `visual` | 是 | 视觉目标的默认值及条件覆盖规则；不用视觉时数组可为空 |
| `metadata` | 否 | 有界的描述性 JSON，不执行规则，不影响界面行为契约 |

ID 区分大小写，非空、最多 128 个 UTF-8 字节且不能含控制字符。题目 ID、题内选项 ID 等各自在规定范围内唯一；引用必须准确匹配。`END` 是流程结束标记，不要作为问题 ID。

下面是一份可以单独保存的完整模型。它询问一个 1–10 的整数，价格固定为 USD 1.00；这里的数量**没有参与计价**，要按数量计价需使用后文的 `quantityFrom`。

<!-- schema: product expect: valid -->
```json
{
  "schemaVersion": "1.0",
  "productId": "example",
  "productVersion": "1",
  "questions": [
    {
      "id": "q",
      "type": "integer",
      "label": "Quantity",
      "min": 1,
      "max": 10,
      "step": 1
    }
  ],
  "flow": {
    "start": "q",
    "edges": [{"from": "q", "to": "END"}]
  },
  "pricing": {
    "currency": "USD",
    "components": [{"id": "base", "category": "base", "amountMinor": 100}]
  },
  "visual": {"targets": [], "slots": []}
}
```

`required` 未写时默认为 true。模型不会自动选中一个选项或填入最小值；Initial 的答案为空。Schema 中的 default 是注解，不能假设任意 JSON 校验器会自动补值；由 KNOT 编译器应用契约规定的默认值。

## 2. questions：题目与答案类型

所有题目都有 `id`、`type`、`label`，可选 `required`。`label` 是展示文字，引用和答案使用 ID，不使用 label。

| type | 类型专有字段 | set_answer 的 value |
|---|---|---|
| `single` | 必填 `options` | 一个已声明的选项 ID 字符串，例如 `"active"` |
| `multi` | 必填 `options` | 不重复的选项 ID 数组，例如 `["bluetooth", "led"]` |
| `integer` | 可选 `min`、`max`、`step`，均为整数 | JSON 安全整数，例如 `2` |
| `decimal` | 必填 `scale`；可选字符串 `min`、`max`、`step` | 精确十进制字符串，例如 `"1.25"`，不是 JSON 浮点数 |
| `color` | 没有额外范围字段 | `#RRGGBB` 字符串，例如 `"#315A79"` |

选项对象必填 `id`、`label`，可选 `enabledWhen`。没有 `enabledWhen` 时选项可用；条件为 false 时选项仍可在返回的选项列表中出现，但 `enabled=false`，不能提交为答案。DSL 没有“给选项添加 price 字段”的语法，价格在 `pricing.components` 中定义。

数值规则：

- integer 使用 ±9007199254740991 范围内的精确整数。
- decimal 的 `scale` 为 0–9；值先规范化，再检查小数位。禁止指数、加号、空白，不会偷偷四舍五入到 scale。
- min/max 必须兼容；step 必须为正，步进锚点为 min，未指定 min 时为 0。未提供边界不代表边界为零，仍受全局数值域约束。
- color 会规范化为大写；不接受颜色名称、透明度后缀或简写 `#FFF`。

多选答案会按选项声明顺序规范化。`required=true` 的多选不能是空数组；可选多选的 `[]` 是明确的“一个也不选”，与未回答、跳过不同。`null` 不是合法答案，清除答案使用 `clear_answer` Event。

## 3. 条件：统一用于选项、分支、价格和视觉

条件是结构化对象，不是 JavaScript、ZEN 源码或任意字符串表达式。接入者不需要学习后端规则引擎。每个条件只使用下列一种形态：

| 形态 | 语义 |
|---|---|
| `{"all": [条件, ...]}` | 非空数组，所有条件成立 |
| `{"any": [条件, ...]}` | 非空数组，至少一个成立 |
| `{"not": 条件}` | 对一个条件取反 |
| `{"field": "题目ID", "op": "运算符", "value": 常量}` | 对该题答案比较；exists 不带 value |

表中的“条件”和“常量”是结构说明，不能原样作为 JSON 发送。`field` 直接写问题 ID，不是 `answers.xxx`、JSONPath 或任意对象路径。

| op | value | 限制 |
|---|---|---|
| `eq` / `ne` | 同类型标量 | 不用来比较整个 multi 数组 |
| `gt` / `gte` / `lt` / `lte` | 数值常量 | 用于数值题，类型和表示须匹配 |
| `between` | 两个数值组成的数组 | 闭区间，包含端点 |
| `in` | 非空、同类型常量数组 | 判断一个标量答案是否在集合中 |
| `contains` | 一个选项 ID | 判断 multi 答案是否包含该选项 |
| `exists` | 不允许 value | 判断是否有答案；跳过视为没有答案 |

例：音箱模型的 200W 选项，`enabledWhen.all` 同时要求 `speaker_type eq active` 和 `material eq aluminum`。这个条件约束答案合法性，不只是前端置灰提示。

未回答时，普通比较都是 false，**包括 ne**。所以 `not(eq(未回答, x))` 可以为 true，而 `ne(未回答, x)` 为 false。需要“已回答且不等于”时应按这个语义建模，不能凭普通编程语言的 null 比较来猜测结果。

引用的题目/选项必须存在，条件与值类型必须匹配，依赖不能形成不允许的循环。条件不是读取用户资料、数据库、时钟或网络的接口；所有求值围绕当前模型和答案进行。

## 4. flow：流程与分支顺序

`flow.start` 指定起始题目。`flow.edges` 中每条边有 `from`、`to`，可选 `when`；终点可以是题目 ID 或 `END`。

离开一道已回答或合法跳过的问题时，按边的**声明顺序**选择第一条条件为 true 的出边。没有 when 的边无条件匹配。因此默认分支应放在条件分支后面；否则后面的条件分支永远轮不到。没有出边匹配时返回 `NO_BRANCH`。图不能形成循环，引用不存在的节点也会被拒绝。

音箱模型的顺序是：

```text
speaker_type → material → panel_color
                           ├─ active：power → extras → END
                           └─ 默认：cable → END
```

`questions` 的声明顺序不等于用户实际经过的路径，路径由 flow 决定。不要把“前端隐藏题目”当成模型分支；未进入的分支必须由模型规则决定。

当前未回答的问题会暂停流程。可选题不会自动跳过，用户要提交答案或 `skip_answer`。当前必选的选择题若没有可用选项，可得到 `blocked`；`in_progress` 表示未完成，`complete` 表示到达 END。`snapshot.validation.valid=true` 不代表流程已经完成。

## 5. pricing：基础价、调整项与精确金额

`currency` 使用支持的三位大写币种代码，货币精度来自服务固定表，不在运行时联网查询。金额字段是整数最小货币单位：CNY 的 `129900` 表示 ¥1,299.00，不能填 `1299.00` 后期待服务按元解释。

每个 component 有唯一 `id`，`category` 为 `base` 或 `adjustment`，可选 `when`。金额表达式二选一：

| 字段 | 计算方式 |
|---|---|
| `amountMinor` | 固定金额 |
| `unitMinor` + `quantityFrom` | 单位金额乘以指定数值题答案 |

同一个组件不能混用两种表达式。`quantityFrom` 引用 integer/decimal 题目，不是任意公式；缺少数量不当零，负数量被拒绝。乘法采用精确十进制，每个组件分别舍入到最小货币单位，恰好半单位时远离零。

得到确定价格需要恰好一个适用 base；多个基础价同时命中会冲突。调整项可叠加，按声明顺序输出明细；折扣可以使用负调整，但总价不能为负，溢出也会失败。完整配置若必要金额无法解析会报错，不产生部分价格。

音箱完成配置的价格来自：有源基础价 129900 + 铝合金 50000 + 200W 90000 + LED 12900 = 282800。50W 没有额外调整项，不需要虚构一个前端价格字段。

### Snapshot 小计与 Quote 是两种结果

| 结果 | 状态 | 应用如何使用 |
|---|---|---|
| `snapshot.price` | `unknown` / `estimated` / `final` | 解释当前已生效组件；estimated 是已回答部分的小计 |
| `Quote.price` | `range` / `final` / `no_completion` / `unavailable` | 展示保留当前选择后，所有合法完整配置的精确范围或最终整机价 |

用户还没选完时展示 Quote 范围，不把 snapshot 小计冒充最终商品价格。Quote 的候选项价格是选该项后的**整机总价**，不等于固定加价。范围上下界相等但配置未完成时仍是 range。不可报价时金额为 null，不能替换成 0 或旧小计。

规则可执行不意味着任意模型都能在有限预算内算出完整价格范围。很大的数值域或搜索空间可能使 Quote 返回 unavailable；详见 [Quote 契约](grpc-api.zh-CN.md#完整配置报价-quote)。报价不创建订单，也不授权支付。

## 6. visual：声明输出，不指定渲染器

`visual.targets` 声明目标：必填 `id`、`defaults`，可选 `order`（默认 0）。defaults 是属性集合。输出按 order，再按目标 ID 的 UTF-8 字节顺序排序。

`visual.slots` 声明一个目标属性如何变化：必填 `id`、`target`、`property`、`cases`。每个 case 有必填 when，以及二选一的 `value`（常量）或 `valueFrom`（题目答案）。引用和类型必须匹配。

valueFrom 的支持范围是：single 答案供 ID 属性使用，color 答案供颜色属性使用，integer/decimal 答案供数值属性使用。布尔和三维向量属性使用常量 value，不能引用一个不存在的布尔或向量题型。

| property | 值类型 |
|---|---|
| `visible` | boolean |
| `variant`、`assetRef`、`surface.textureRef` | 资源/变体 ID |
| `color`、`emission.color` | `#RRGGBB` |
| `opacity`、`surface.metallic`、`surface.roughness`、`emission.strength`、`light.intensity` | 0–1 的有限数值 |
| `transform.position`、`transform.scale` | 恰好三个有限数值，每项绝对值不超过 1e9 |

带点的属性名就是 JSON 键，如 `"surface.metallic"`，不要自行改成嵌套的 `surface` 对象。不能添加 Schema 未声明的属性。

视觉规则与流程分支不同：同一个 slot 若有多个 case 同时命中，返回 `VISUAL_CONFLICT`，**不是选第一条或最后一条**。多个 slot 不能写同一个 target/property，即使作者认为它们的条件互斥，也会在编译时拒绝。

没有 case 命中时恢复 defaults；该属性没有默认值则不出现在结果中。valueFrom 取不到答案会失败，因此音箱颜色规则用 `exists(panel_color)` 保护，再读取 `panel_color`。

音箱实例中，箱体材质切换 `speaker.cabinet` 的 assetRef，面板答案驱动 `speaker.front_panel.color`，extras 包含 led 时显示 `speaker.led_ring`。资源 ID 由你的渲染器映射到 SVG、图片或三维资源；服务不读 URL、不加载贴图、不画页面。Apply 成功后替换完整 Visual；失败时保留上一份显示和 State。

## 7. Product、State、Event 与响应的边界

| 对象 | 谁生成 | 内容与用途 |
|---|---|---|
| Product | 模型作者 | 上述 DSL；服务启动时编译 |
| State | Initial/Apply 成功结果 | schemaVersion、productId、modelDigest、revision、answers、skipped；由应用保存 |
| Event | 应用根据用户动作构造 | schemaVersion、type、questionId、expectedRevision；set_answer 另有 value |
| TransitionResult | Initial/Apply/Evaluate | state、snapshot、changes、diagnostics，以及 schemaVersion |
| QuoteResult | Quote | 完整商品 price、focus 候选/预览、模型身份与 revision；不替换 State |

不能直接改 answers 或 revision 来提交选择。`set_answer` 提交目标答案（multi 传完整集合）；`clear_answer` 清除有效已到达前缀中的已答题；`skip_answer` 仅用于当前未回答的可选题。重复跳过、跳过必选题、answers 与 skipped 重叠等都不合法。

当上游选择改变时，服务会重新协调后续状态：禁用答案被清除，不再可达分支的答案被清除，不会自动换成另一个选项。保存完整新 State，不要仅合并本次点击；Changes 用于解释变化，不是存档替代物。

Evaluate 用于恢复存档，不修复或迁移错误 State。revision 校验的是请求中携带的 State，不是数据库；应用仍需 CAS/事务防止并发覆盖。详细调用见 [gRPC 参考](grpc-api.zh-CN.md)和[音箱教程](grpc/speaker.zh-CN.md)。

## 8. 模型身份、部署与更新

三个版本不要混淆：`schemaVersion` 是 JSON 协议格式，`productVersion` 是作者维护的商品版本，`modelDigest` 是编译产物的语义身份。gRPC 包名 `knot.v1` 又是传输契约版本。不能拿原始文件 SHA-256 代替 modelDigest。

模型发布方负责提供 JSON 和对应编译 digest；调用方只需要服务地址、proto、证书和 digest。发布方可用同版本源码的 `model_manifest` 工具生成 manifest，见[模型身份说明](grpc/clients.zh-CN.md#模型身份)。当前运行镜像没有独立的 manifest 命令，也没有 Compile、上传或列举模型 RPC；这是现有接入边界。

将模型 JSON 挂载到模型目录，服务启动时统一编译，成功后才就绪。目录顶层不要混放 events、manifest、State 或其他 JSON。更新模型需替换部署并重启；保留仍被旧会话引用的模型版本。不要给旧 State 换一个 digest 强行继续；需要业务迁移时由应用明确处理，或告知用户重新开始。

## 9. 校验与排错顺序

1. 校验 JSON 格式、字段和 Schema：版本、必填项、未知字段、类型、数量上限。
2. 用实际服务启动编译：检查引用、重复 ID、数值域、流程环路、规则类型和依赖。Schema 通过不能跳过这一步。
3. 跑 Initial/Apply/Evaluate/Quote：既测常用路径，也测默认分支、选项禁用、可选题、回改和恢复。
4. 验证完整报价与视觉：基础价互斥、分支顺序正确、slot 的 case 互斥，无法报价时 UI 有明确处理。

| 现象 | 优先检查 |
|---|---|
| 服务启动失败 | 模型目录混入非 Product JSON；字段、引用、循环、规则类型错误 |
| INVALID_OPTION | 当前选项的 enabledWhen；该 ID 是否属于这道题 |
| NO_BRANCH | 出边是否覆盖当前答案/skip，默认边是否存在 |
| VISUAL_CONFLICT | 同一 slot 的多个 case 是否同时成立 |
| MODEL_MISMATCH | State 的 productId/modelDigest 与外层选定模型是否一致 |
| REVISION_CONFLICT | Event 是否使用最新成功 State 的 revision |
| RESOURCE_LIMIT / Quote unavailable | 输入规模、域或搜索预算；两者不是同一类结果 |

关键边界：Product 4 MiB、State 512 KiB、Event 64 KiB；最多 256 题、每题 256 选项、总计 8192 选项、2048 条边、1024 个价格组件、256 个视觉目标、2048 个 slot、总计 4096 个 case。各限额独立生效，不能推断所有上限可同时取满。完整字段上限见 Schema，Quote 另有[调用预算](grpc-api.zh-CN.md#完整配置报价-quote)。

## 10. 用音箱模型核对一次完整链路

1. questions 声明有源/无源、材质、颜色、功率、附加项和接口类型。
2. enabledWhen 让 200W 仅适用于有源且铝合金的配置。
3. flow 让有源走功率/附加项，无源走接口；条件边先于默认边。
4. pricing 用不同 base 和条件 adjustment 组成整机价。
5. visual 将材质/颜色/LED 选择转成资产、颜色与可见性。
6. 应用提交 Event，保存返回 State，渲染 Snapshot，再调用 Quote 展示完整价格。
7. 改回胡桃木后，服务清除失效的功率/附加项，视觉隐藏 LED，报价从 final 回到 range。

可运行脚本、三种语言客户端及真实预期结果见[音箱业务教程](grpc/speaker.zh-CN.md)。前端只负责交互和渲染，不再实现另一套同名规则或计价逻辑。
