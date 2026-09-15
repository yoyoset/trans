# Trans 聊天翻译

在任意聊天场景（网页聊天、客服窗口、游戏聊天框）里：读取对方的外语消息并翻译；用中文写回复，得到外语译文。

- **本地优先**：默认用本机 CPU 上的小模型翻译，结果立即出现，不花钱、不依赖网络。
- **在线校对**：觉得译文不对时点一下，由在线大模型结合最近几轮对话修正。
- **模型可替换**：本地模型、在线服务都只是设置项，出了更好的模型直接换。

设计与实测记录见 [docs/architecture.md](docs/architecture.md) 和 [docs/m0/](docs/m0/)。

## 目录结构

```
crates/
  trans-domain        纯逻辑：提示词、校验、区域锚点……（无 IO）
  trans-application   端口定义、引擎路由、翻译/校对用例、设置模型
  trans-providers     OpenAI 兼容客户端、llama-server 管理、本地模型发现
  trans-presets       区域预设（框选区域、解析方式、术语表）
  trans-win           Windows 截图 / OCR / 凭据管理器
app/
  src-tauri           Tauri 壳：组合根与命令转发
  src                 React 前端
spikes/               M0 一次性验证代码（不属于产品）
```

依赖只能向内：`domain ← application ← providers / win ← app`。

## 开发

需要：Rust 1.92+、Node 24+、Windows 10/11。

```bash
cd app
npm install
npx tauri dev
```

首次运行后在「设置」里指定 llama-server 程序和 GGUF 模型文件。

## 质量门禁

提交前必须全部通过：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd app && npm run build
```

需要真实环境的测试默认忽略，按需手动运行：

```bash
cargo test -p trans-win -- --ignored
cargo test -p trans-providers --test local_engine_smoke -- --ignored --nocapture
```

后者需要先设置环境变量 `TRANS_LLAMA_SERVER` 和 `TRANS_LOCAL_MODEL`。

## 许可证

MIT OR Apache-2.0，任选其一。
