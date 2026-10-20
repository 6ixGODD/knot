# 通过 gRPC 接入 KNOT

[English](README.md)

你的应用把用户选择发给 KNOT，得到一致的配置状态、可选项、视觉属性和完整商品报价。服务运行在 Docker 中，客户端通过普通 gRPC 调用；接入方不需要 KNOT 原生库、C ABI、cgo 或 Rust 编译器。

| 阅读顺序 | 解决的问题 |
|---|---|
| [完整接入体系](../integration-overview.zh-CN.md) | 模型、状态、服务、应用和渲染分别负责什么 |
| [商品模型 DSL 1.0](../dsl-reference.zh-CN.md) | 定义自己的题目、选项、条件、分支、价格和视觉规则 |
| [快速开始](quickstart.zh-CN.md) | 拉取镜像、挂载商品模型、检查服务、发出第一次请求 |
| [接口参考](../grpc-api.zh-CN.md) | 四个 RPC 的输入、输出、语义和错误 |
| [Go、Python、Node.js 客户端](clients.zh-CN.md) | 安装各语言依赖，直接调用每个接口 |
| [音箱定制业务教程](speaker.zh-CN.md) | 从选配、报价到回改、保存和恢复 |
| [部署](../deployment.md) | mTLS、限额、模型更新和停机 |
| [可选的前端演示](../../examples/grpc/README.zh-CN.md) | 通过本地 HTTP 转接层把服务结果画成 SVG |

镜像包含服务，不内置你的商品目录。商品由运营方部署为 JSON 模型；每个请求通过精确的 `model_digest` 选择编译后的模型版本。用户鉴权、会话存储和订单属于你的应用，KNOT 不替你保存会话或下单。

Release 压缩包只是 `.proto`、JSON Schema、模型和示例的合集。拉取镜像不依赖它；已有服务的调用方，只需取得地址、证书、proto 和模型 digest，也不需要前端或整套压缩包。前端和 Go 转接层只是可选消费者。
