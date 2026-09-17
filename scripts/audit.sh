#!/usr/bin/env bash
# 依赖安全检查：Rust（cargo-audit，查 RustSec 公告）+ 前端（npm audit）。
# 需要联网拉取公告库，所以不放进每次提交都跑的 check.sh；升级依赖后、发布前手动运行。
# 首次使用先安装：cargo install cargo-audit --locked
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

printf '==> cargo audit\n'
cargo audit

printf '\n==> npm audit\n'
(cd app && npm audit)

printf '\nno known vulnerabilities\n'
