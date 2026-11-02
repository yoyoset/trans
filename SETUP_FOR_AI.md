# 给 AI 的安装清单（trans）

> 本仓库**只提供框架（源码）**：不含模型、不含 llama-server 二进制。需要的东西由使用者或它的 AI 按下面的清单去下载。
> 数字取自 2026-11-02 的实测；URL 当天 HTTP 200。

## 0. 先决条件

- Windows 10/11、Rust 1.92+、Node 24+、git bash（质量门禁脚本需要）

## 1. 构建与运行

```bash
cd app
npm install
npx tauri dev
```

首次运行后在「设置」里指定 llama-server 程序和 GGUF 模型文件（本地引擎默认纯 CPU 小模型）。在线校对是可选项，需要自己的 API 密钥（只进 Windows 凭据管理器，不进仓库）。

## 2. 需要下载的外部资产

| 资产 | 来源 | 大小 | sha256 | 许可 |
| :--- | :--- | :--- | :--- | :--- |
| llama-server（llama.cpp；CPU 即可，有 N 卡可选 CUDA 包） | https://github.com/ggml-org/llama.cpp/releases ，例：`llama-b10079-bin-win-cuda-12.4-x64.zip`；CPU 版选同 tag 的 `win-cpu-x64` | CUDA 例 249,081,777 B | 例（CUDA 包）：`3f94750a057e22e970e5fc6e4960dddd018baf4ed69829c84d1a2f09b5894bdd` | MIT |
| 本地翻译模型 Qwen3-4B-Instruct-2507 Q4_K_M | https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/main/Qwen3-4B-Instruct-2507-Q4_K_M.gguf | 2,497,281,120 B | `3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597`（= HF LFS oid） | Apache-2.0 |

模型可替换：任意指令微调 GGUF 都行；`docs/m0/02-translate.md` 有 4B 本地模型的实测结论（**挡不住提示注入、回译不可用**，别拿它当可信翻译）。

## 3. 验证

```bash
scripts/check.sh      # cargo fmt --check + clippy -D warnings + cargo test --workspace + vitest + tsc/vite build
```

需要真实环境的测试默认忽略：`cargo test -p trans-providers --test local_engine_smoke -- --ignored --nocapture`，先设环境变量 `TRANS_LLAMA_SERVER`（llama-server.exe 路径）和 `TRANS_LOCAL_MODEL`（GGUF 路径）。

## 4. 已知陷阱

- 本地小模型延迟（Qwen3-4B Q4，纯 CPU，直连）实测首字约 0.4s、总耗时约 3–4s，详见 `docs/m0/02-translate.md`。
- 用 pi/opencode 这类转发层时，自定义模型声明 `contextWindow: 4096`/`maxTokens: 256` 会导致每次只生成 1 个 token，去掉即恢复（同文档）。
