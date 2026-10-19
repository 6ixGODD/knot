# 音箱配置示例

[English](README.md) · [gRPC 接口](../../docs/grpc-api.zh-CN.md)

示例连接真实 KNOT gRPC 服务。浏览器示例供前端接入参考；脚本示例记录可复现的购买、修改场景。

## 启动前端

准备 Docker Linux 容器、Python 3.9+、Go 1.26.4。先从 [v0.1.0 Release](https://github.com/6ixGODD/knot/releases/tag/v0.1.0) 下载 `knot-grpc-0.1.0.tar.gz`，在下载目录运行以下命令。GHCR 镜像为私有包，`docker login ghcr.io` 使用有仓库权限的 GitHub 用户名和具有 `read:packages` 权限的 classic PAT；不要把 token 写进源码。首次启动下载 Go 模块，不需要 Rust 或 protoc。

```powershell
docker login ghcr.io
docker pull ghcr.io/6ixgodd/knot:0.1.0
tar -xzf ./knot-grpc-0.1.0.tar.gz
$image = 'ghcr.io/6ixgodd/knot:0.1.0'
cd knot-grpc-0.1.0
$out = Join-Path $env:TEMP ('knot-speaker-web-' + [guid]::NewGuid().ToString('N'))
python examples/grpc/speaker_web.py --image $image --out $out --port 8765
```

打开 <http://127.0.0.1:8765>。使用配套、支持 Quote 的镜像。启动脚本构建 Go 转接服务、启动只读容器，两项服务都只绑定本机回环地址。在启动终端按 Ctrl+C 停止服务并移除容器。日志保留在输出目录；下次启动使用新的目录。

## 试用步骤

1. 依次选择有源 → 铝合金 → 面板颜色 → 200W → LED，完成最后一步查看已确认总价。
2. 点击步骤按钮或“上一项”返回面板颜色，调整颜色观察面板，已有选择保留。
3. 返回材质改成胡桃木，服务清除不兼容的功率、附加功能答案并隐藏灯环，再按可选项继续。
4. 刷新页面，通过 Evaluate 恢复保存的配置；点击“重新开始”重置。

完成之前，报价覆盖所有合法完整配置，包含未回答的必选成本。每个选项显示选择后的整机报价，不是加价。区间两端相等时可能只显示一个金额，标题仍标明未完成的价格范围。

价格区域始终标识最近一次已确认报价。请求未完成时保留旧值，因此它可能暂时对应上一次选择。错误单独显示，不能拿保留的旧报价授权下单。金额不做虚构中间数值的动画。

连续调色时串行调用 Apply，只保留最新的未发送颜色。Visual 跟随服务端成功响应；待修改稳定或输入暂停 120 ms 后获取 Quote。已确认的面板、灯环颜色使用 80 ms 过渡，系统减少动态效果设置会关闭过渡。网络延迟仍会影响预览，浏览器原生调色器也因平台而异。

## 源码入口

| 文件 | 职责 |
|---|---|
| [speaker_web.py](speaker_web.py) | 构建、启动、健康检查、清理 |
| [main.go](go/cmd/speaker-web/main.go) | 本机 HTTP 到 gRPC 转接与内嵌资源 |
| [session.mjs](go/cmd/speaker-web/web/session.mjs) | Apply 串行、调色合并、过期 Quote 丢弃 |
| [app.js](go/cmd/speaker-web/web/app.js) | 控件、State 保存、已确认 SVG 属性 |
| [style.css](go/cmd/speaker-web/web/style.css) | 布局、过渡效果 |
| [web/](go/cmd/speaker-web/web/) | 四层 SVG 和 HTML |
| [speaker.json](../../contracts/examples/speaker.json) | 产品选项、规则和视觉目标 |

修改内嵌资源后重启脚本。价格和答案协调留在服务端，前端将 Visual 目标映射到自己的资源。HTTP 转接是示例代码，不是公共 REST API。localStorage 按浏览器源保存配置，不同端口不共享存档。示例不提供账号、共享会话和支付功能。展开“请求与响应”可查看最近一次调用，其中包含完整演示答案。

## 排查与检查

- 端口占用：停止旧启动脚本，或修改 `--port`。
- 输出目录已存在：换用包/源码目录之外的新路径。
- 不支持 Quote 或页面陈旧：使用配套镜像，修改源码后重启并刷新。
- 恢复失败：存档保留，先检查错误和部署模型；“重新开始”会在初始化成功后替换存档。
- 构建/启动失败：检查输出目录的 `build.log`、`web.log` 及终端输出。

在包根目录执行前端队列回归检查：

```text
node --test examples/grpc/go/cmd/speaker-web/web/app.test.mjs
```

仅此测试需要 Node。脚本场景使用 `speaker_demo.py`、同一 `--image` 及另一个新的 `--out`；[接口指南](../../docs/grpc-api.zh-CN.md)介绍场景和响应文件。脚本沿用的 `snapshot.price` 小计与浏览器的完整配置 Quote 不同。
