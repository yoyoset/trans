#!/usr/bin/env bash
# 提交前的质量门：任何一步失败立即退出并返回非零，只有全部通过才打印 "all checks passed"。
# 用法（git bash）：scripts/check.sh
# 提交时用 `scripts/check.sh && git commit ...`，避免在测试失败时照样提交。
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

step() { printf '\n==> %s\n' "$1"; }

step "cargo fmt --check"
cargo fmt --all --check

step "cargo clippy (warnings are errors)"
cargo clippy --workspace --all-targets -- -D warnings

step "cargo test"
cargo test --workspace

step "frontend tests"
(cd app && npm test)

step "frontend typecheck + build"
(cd app && npm run build)

printf '\nall checks passed\n'
