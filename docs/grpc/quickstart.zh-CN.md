# 启动本地 gRPC 服务

[English](quickstart.md) · [文档入口](README.zh-CN.md)

需要 Docker（支持 Linux amd64 容器）和 curl。命令采用单行，避免不同 shell 的续行符造成混淆。在一个新的工作目录中执行。

## 1. 拉取镜像和一个商品模型

```sh
docker pull ghcr.io/6ixgodd/knot:0.1.0
mkdir models
curl -fL https://raw.githubusercontent.com/6ixGODD/knot/v0.1.0/contracts/examples/speaker.json -o models/speaker.json
```

PowerShell 中下载命令用 `curl.exe`。这里只下载示例商品，不需要 SDK、前端或协议压缩包。固定部署版本时，可以改用 Release 的 `release.json` 中记录的镜像 digest。

目录中只放商品 JSON。不要直接挂载压缩包整个 `models/`：其中的 manifest 和 events 不是商品。服务会编译目录顶层所有 `.json`，任何一个无效都会使启动失败。

## 2. 启动容器

Linux/macOS shell（镜像为 Linux amd64，Docker 需要支持该平台；这不代表原生 macOS SDK 已验证）：

```sh
docker run --detach --name knot-grpc --read-only --cap-drop ALL --security-opt no-new-privileges --memory 512m --cpus 2 --publish 127.0.0.1:50051:50051 --mount "type=bind,src=$(pwd)/models,dst=/models,readonly" --env KNOT_HEALTH_ENDPOINT=http://127.0.0.1:50051 ghcr.io/6ixgodd/knot:0.1.0 serve --transport plaintext
```

PowerShell：

```powershell
$models = (Resolve-Path ./models).Path
docker run --detach --name knot-grpc --read-only --cap-drop ALL --security-opt no-new-privileges --memory 512m --cpus 2 --publish 127.0.0.1:50051:50051 --mount "type=bind,src=$models,dst=/models,readonly" --env KNOT_HEALTH_ENDPOINT=http://127.0.0.1:50051 ghcr.io/6ixgodd/knot:0.1.0 serve --transport plaintext
```

本教程明确开启明文，仅映射本机回环地址。远程部署使用 [mTLS](../deployment.md)。容器用户为 UID 10001，模型文件必须对它可读。

## 3. 确认就绪

```sh
docker exec knot-grpc /usr/local/bin/knot-server healthcheck --endpoint http://127.0.0.1:50051
docker logs knot-grpc
```

健康检查必须输出 `SERVING` 且退出码为 0。初次编译模型需要时间；未就绪先看日志，端口能连接不代表模型加载成功。上面的环境变量也让镜像自带健康检查使用正确的本地明文地址。

## 4. 第一次调用

未修改的音箱模型，其编译 digest 为：

```text
sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541
```

它不是 JSON 文件的 SHA-256。教程中不要修改模型；自定义模型的 digest 由服务运营方提供，见[模型身份](clients.zh-CN.md#模型身份)。

按[语言客户端](clients.zh-CN.md)安装所选语言依赖，然后调用 `Initial`。例如，在 Go 示例目录中：

```sh
go run ./cmd/call Initial --plaintext --endpoint 127.0.0.1:50051 --digest sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541
```

响应有 `state`、`snapshot`、`changes`、`diagnostics`。后续请求需要的是完整 `state`，不是整个响应。初始 revision 为 0，snapshot 提供流程和视觉状态。完整商品价格区间要调用 `Quote`，不要用未完成配置的 `snapshot.price` 代替。

下一步：[音箱业务教程](speaker.zh-CN.md)。结束时：

```sh
docker stop --time 15 knot-grpc
docker rm knot-grpc
```

启动失败时检查：Docker 是否运行 Linux 容器、50051 是否被占用、挂载是否为存在的绝对路径、目录中是否混入非商品 JSON。RPC 的 `NOT_FOUND` 表示 digest 没有加载；`UNAVAILABLE` 则先排查地址、就绪状态、TLS 和日志。
