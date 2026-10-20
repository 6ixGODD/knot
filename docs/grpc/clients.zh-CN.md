# 语言客户端

[English](clients.md) · [文档入口](README.zh-CN.md)

这些客户端直接连接已启动的 gRPC 服务，不启动 Docker 或前端。使用当前源码 checkout：新增客户端不在旧版 v0.1.0 协议压缩包内。只需要所选语言的运行时和 gRPC 依赖。先完成[服务启动](quickstart.zh-CN.md)。

三种语言采用相同参数。成功时完整 JSON 写入 stdout，退出码 0；领域错误完整信封写入 stderr，退出码 2；传输或协议错误退出码 1。只有成功结果可替换保存的 State。客户端设置 10 秒 deadline、4 MiB + 1 KiB 接收上限，不配置显式重试。

## Go

需要 Go 1.26.4，已包含生成代码，无需 protoc 或 cgo。从仓库根目录执行：

```sh
cd examples/grpc/go
go run ./cmd/call Initial --plaintext --digest sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541
```

[客户端源码](../../examples/grpc/go/cmd/call/main.go)使用 `NewConfigurationServiceClient`，分别调用生成的四个 RPC，通过 oneof 类型检查响应，再做 JSON Schema 校验。

## Python

需要 Python 3.10+。从仓库根目录创建环境：

```sh
python -m venv .venv
```

Linux/macOS 激活：`. .venv/bin/activate`；PowerShell 激活：`.venv/Scripts/Activate.ps1`。接着：

```sh
python -m pip install grpcio==1.84.0 grpcio-tools==1.84.0
python -m grpc_tools.protoc -I contracts/grpc --python_out=examples/grpc/python --grpc_python_out=examples/grpc/python contracts/grpc/knot/v1/configuration.proto
python examples/grpc/python/client.py Initial --plaintext --digest sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541
```

[客户端源码](../../examples/grpc/python/client.py)使用 `ConfigurationServiceStub`，直接传 UTF-8 bytes，并检查 `WhichOneof("outcome")`。生成的是网络消息代码，不是编译 KNOT 原生库。

## Node.js

需要 Node.js 20+。从仓库根目录执行：

```sh
npm install --prefix examples/grpc/node @grpc/grpc-js@1.14.6 @grpc/proto-loader@0.8.1
node examples/grpc/node/client.cjs Initial --plaintext --digest sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541
```

[客户端源码](../../examples/grpc/node/client.cjs)动态加载 proto，使用 `Buffer` 传 JSON，启用 `oneofs: true` 并检查 `outcome`。这是后端 Node.js，不是浏览器 gRPC-Web。十进制业务值保留字符串，不转浮点数。

## 调用每个接口

将 PROGRAM 换成 `go run ./cmd/call`（Go 示例目录内），或 `python examples/grpc/python/client.py` / `node examples/grpc/node/client.cjs`（仓库根目录）。DIGEST 换成上方完整 digest。文件路径相对当前工作目录。

```text
PROGRAM Initial --plaintext --digest DIGEST
PROGRAM Apply --plaintext --digest DIGEST --input state.json --event event.json
PROGRAM Evaluate --plaintext --digest DIGEST --input state.json
PROGRAM Quote --plaintext --digest DIGEST --input quote.json
```

`state.json` 只包含 Initial/Apply 返回的完整 `state`，不是整个响应。`event.json` 包含一个完整 Event。`quote.json` 包含版本和完整 state 对象，以及可选的焦点/预览字段。[音箱教程](speaker.zh-CN.md)和[可运行驱动](../../examples/grpc/walkthrough.py)会准备这些文件，并用任一语言客户端调用全部四个接口。

## 远程连接与校验

删除 `--plaintext`，提供 `--ca server-ca.pem --cert client.pem --key client-key.pem --endpoint knot.example.com:50051`。endpoint 主机名必须匹配服务证书 SAN，示例不关闭主机名校验。客户端身份由运营方提供，私钥放在源码之外。

Python/Node 精简示例检查 oneof、UTF-8、JSON 和版本，不是完整的恶意响应防护实现。正式应用保存结果前还需按 [JSON Schema](../../contracts/schemas/README.md) 校验，并确认响应的模型身份/revision 与请求对应。Go 示例已使用完整 Schema 校验。gRPC OK 不代表领域成功。

## 模型身份

服务没有 Compile、上传或列举模型 RPC。运营方提供已部署商品的编译 digest。教程固定 digest 仅适用于未修改的 `speaker.json`。自定义模型的发布方可以用同版本源码工具生成 manifest：

```sh
cargo run --release --locked -p knot-server --example model_manifest -- /absolute/path/product.json
```

这个编译步骤属于模型发布方，不是网络调用方的前置条件。目前运行镜像没有独立 manifest 命令；文件 hash 不能替代编译 digest。保存中的会话仍引用旧模型时要保留旧版本，不能修改旧 State 的 digest 强行迁移。

生成和加载方式参考官方 [Python](https://grpc.io/docs/languages/python/basics/) 和 [Node](https://grpc.io/docs/languages/node/basics/) 教程。Java、C# 等语言也可从同一 proto 生成客户端，遵循相同的 bytes、oneof、deadline 契约；这里提供 Go、Python、Node.js 三种可运行示例。
