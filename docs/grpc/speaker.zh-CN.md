# 实战：音箱定制流程

[English](speaker.md) · [文档入口](README.zh-CN.md)

场景：客户定制有源音箱，选择箱体、颜色、功率和 LED，然后回头修改箱体。应用要在未选完时给完整商品价格范围，回改后清除不兼容选择，并正确保存、恢复配置。

使用未修改的[音箱模型](../../contracts/examples/speaker.json)、[本地服务](quickstart.zh-CN.md)和一种[语言客户端](clients.zh-CN.md)。这个流程不依赖前端；商城、销售配置工具或后端系统都可以按同样方式接入。

## 运行完整流程

安装客户端依赖后，在仓库根目录选择一种命令。每次使用新的输出目录。Bash 和 PowerShell 都能执行；Unix 可将 python 换成 python3。

```sh
python examples/grpc/walkthrough.py --out ../speaker-python-run -- python examples/grpc/python/client.py
python examples/grpc/walkthrough.py --out ../speaker-node-run -- node examples/grpc/node/client.cjs
```

Go 先构建客户端，保证退出码 2 不被 `go run` 转成 1：

```sh
go -C examples/grpc/go build -o ../../../knot-call ./cmd/call
python examples/grpc/walkthrough.py --out ../speaker-go-run -- ./knot-call
```

Windows 将输出路径改为 `../../../knot-call.exe`，驱动命令末尾改为 `-- ./knot-call.exe`。这只构建 Go 网络客户端，不构建 Rust 服务或原生库。

驱动默认连接 localhost:50051；在 `--` 前增加 `--endpoint host:port` 可换地址。它只用于本地明文教程，通过子进程调用所选客户端，保存每次请求 JSON、完整 stdout/stderr 和退出码。共检查 20 次调用，覆盖四个 RPC、正确报价、领域错误及未知模型的传输错误。

## 1. 开始配置，显示完整价格范围

用模型 digest 调 Initial，保存 revision 0 的完整 State。再调用 Quote，内部请求包含 `schemaVersion="1.0"` 和该 State。初始范围为 CNY 1,199–3,027，即 `minMinor=119900`、`maxMinor=302700`、`currency="CNY"`、`status="range"`。

金额是最小货币单位的整数，不是浮点元数。范围已包含未回答的必选项。配置未完成时，不能把旧接口的 `snapshot.price.totalMinor` 累计小计当成完整商品报价。

## 2. 提交选择，渲染确认后的结果

依次提交 `speaker_type=active`、`material=aluminum`、`panel_color="#315A79"`、`power=200w`、`extras=["led"]`。每次都是 `set_answer` Event，`expectedRevision` 来自最近成功的 State。驱动会构造请求，可直接查看编号的 `.event.json` 文件。

每次 Apply 成功后整体替换 State，用 `snapshot.visual` 渲染，再对新状态调用 Quote。选完后价格为 CNY 2,828（`282800`），Quote 状态 `final`，上下界相等，LED 视觉目标可见。

候选项报价是**选该项之后的整机价格**，不能标成固定加价。多选题的 Quote candidate 表示在当前集合中切换该选项；Apply 仍需传完整目标集合。

## 3. 返回上一项并修改

想重新查看箱体时，Quote 指定 `focusQuestionId="material"`，State 不变。导航不修改答案或 revision；客户真正改选时才提交 Apply。

把箱体改成 walnut 后，服务清除不兼容的 power/extras。整体保存返回状态，按 Visual 更新箱体并隐藏 LED。Quote 变为 CNY 1,599–2,327 的 `range`，不能继续显示旧配置的最终价。此时尝试 `power=200w` 会返回 `INVALID_OPTION`；保留最近成功状态。

## 4. 恢复、并发和保存

对存档调用 Evaluate 来恢复视图；State 和 revision 不变，再调用 Quote。Evaluate 失败不能覆盖原存档。

KNOT 没有服务端会话：拿同一份旧 State 发两个 Apply，两次都可能成功。你的应用必须用旧 revision 加自己的会话/版本标识，对数据库执行 CAS 或事务更新；保存成功后才能确认修改完成。将旧 `expectedRevision` 的 Event 用于新 State 则返回 `REVISION_CONFLICT`，驱动也检查了这一点。

UI 应串行提交 Apply；Quote 响应按模型 digest、revision、焦点和本地请求序号去除过时结果。请求未完成时明确显示最近已确认价格，不能让旧颜色响应覆盖新选择。Quote 不授权付款；订单创建要确认目标模型/配置，并应用你的结算规则。

## 最后再加前端

[SVG 演示](../../examples/grpc/README.zh-CN.md)使用 Go HTTP 转接层，因为运行服务提供普通 gRPC，没有 REST 或 gRPC-Web。HTTP 路由属于示例，不是 KNOT 公共接口。你自己的后端可以用任一语言客户端，再提供适合浏览器的 API。
