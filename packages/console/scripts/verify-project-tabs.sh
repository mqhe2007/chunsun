#!/usr/bin/env bash
# 验收：项目列表页签与导航文案（需求 gHuSewpabCSl）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
CONSOLE="$ROOT/packages/console/src"
fail=0

ok() { echo "OK: $1"; }
bad() { echo "FAIL: $1"; fail=1; }

if rg -q 'to="/projects"' "$CONSOLE/layouts/ConsoleLayout.vue" \
  && rg -q '^\s+项目\s*$' "$CONSOLE/layouts/ConsoleLayout.vue" \
  && ! rg -q '项目管理' "$CONSOLE/layouts/ConsoleLayout.vue"; then
  ok "侧栏导航文案为「项目」"
else
  bad "侧栏导航文案应为「项目」且不含「项目管理」"
fi

if rg -q 'title: "项目"' "$CONSOLE/pages/projects/index.vue"; then
  ok "路由 meta title 为「项目」"
else
  bad "路由 meta title 应为「项目」"
fi

if rg -q 'title="项目"' "$CONSOLE/screens/projects/ProjectListView.vue" \
  && rg -q '我的项目' "$CONSOLE/screens/projects/ProjectListView.vue" \
  && rg -q '参与的项目' "$CONSOLE/screens/projects/ProjectListView.vue" \
  && rg -q 'tabs tabs-border' "$CONSOLE/screens/projects/ProjectListView.vue"; then
  ok "列表页标题与双页签"
else
  bad "列表页缺少标题或双页签"
fi

if rg -q 'aria-label="回到项目"' "$CONSOLE/layouts/ConsoleShell.vue"; then
  ok "品牌回链 aria-label 为「回到项目」"
else
  bad "品牌回链 aria-label 应为「回到项目」"
fi

cd "$ROOT/packages/console"
pnpm exec vitest run src/screens/projects/projectListPartition.test.ts

if [[ "$fail" -ne 0 ]]; then
  echo "验收失败"
  exit 1
fi
echo "验收通过：导航文案 + 页签文案 + 分类单测"
