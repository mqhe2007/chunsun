#!/usr/bin/env bash
# 知识库 CLI 创建/更新能力端到端验收脚本（需求 QmITvkJ0Nosm）。
#
# 覆盖验收点：
#   1. CLI create：POST 知识文档成功，返回文档 ID；index / 单条 doc / 按策略过滤均可读回
#   2. CLI create 支持 --strategy lazy；lazy 文档不出现在 eager 过滤里
#   3. CLI update：改 title/content/strategy/sortOrder 成功，回读一致
#   4. CLI update 至少提供一个字段：空更新报错非零退出
#   5. CLI create --strategy 非法值：报错非零退出
#   6. CLI update 不存在的文档：报 CONTEXT_DOC_NOT_FOUND
#   7. 保持不支持删除：CLI 无 delete 子命令（help 不含 delete；调用报未知子命令）
#   8. 后端真相校验：API 列表含 CLI 创建的文档
#
# 前置：一个已启动、迁移就绪的后端实例（本地 dev：127.0.0.1:11112）；环境变量：
#   CHUNSUN_BASE     后端 base URL（默认 http://127.0.0.1:11112/api/v1）
#   CHUNSUN_EMAIL / CHUNSUN_PASSWORD  已激活账号（否则走注册，需 SMTP 配置）
#   CHUNSUN_CLI_BIN  本地编译的 CLI 二进制（默认 packages/cli/target/debug/chunsun）
#
# 用法：
#   CHUNSUN_EMAIL=e2e-kcli@test.local CHUNSUN_PASSWORD=e2e-secret-123 ./scripts/e2e-knowledge-cli.sh

set -u

BASE="${CHUNSUN_BASE:-http://127.0.0.1:11112/api/v1}"
CLI="${CHUNSUN_CLI_BIN:-$(dirname "$0")/../../cli/target/debug/chunsun}"
EMAIL="${CHUNSUN_EMAIL:-}"
PASSWORD="${CHUNSUN_PASSWORD:-}"
JSON="Content-Type: application/json"

PASS=0
FAIL=0

check() { # name, expected_substr, actual
  if echo "$3" | grep -q "$2"; then
    printf '  ✓ %s\n' "$1"; PASS=$((PASS + 1))
  else
    printf '  ✗ %s\n    期望含: %s\n    实际: %s\n' "$1" "$2" "$3"; FAIL=$((FAIL + 1))
  fi
}

check_absent() { # name, unexpected_substr, actual
  if echo "$3" | grep -q "$2"; then
    printf '  ✗ %s\n    不应含: %s\n    实际: %s\n' "$1" "$2" "$3"; FAIL=$((FAIL + 1))
  else
    printf '  ✓ %s\n' "$1"; PASS=$((PASS + 1))
  fi
}

check_exit_nonzero() { # name, expected_substr, exit_code, output
  if [ "$3" -ne 0 ]; then
    if echo "$4" | grep -q "$2"; then
      printf '  ✓ %s（退出码 %s）\n' "$1" "$3"; PASS=$((PASS + 1))
    else
      printf '  ✗ %s\n    退出码非 0 但输出不含: %s\n    实际: %s\n' "$1" "$2" "$4"; FAIL=$((FAIL + 1))
    fi
  else
    printf '  ✗ %s\n    期望非零退出，实际退出码 0\n    输出: %s\n' "$1" "$4"; FAIL=$((FAIL + 1))
  fi
}

post() { curl -s -X POST "$BASE$1" ${AUTH:+-H "$AUTH"} -H "$JSON" -d "$2"; }
get()  { curl -s "$BASE$1" ${AUTH:+-H "$AUTH"}; }
jid()  { python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["id"])'; }

echo "== 0. 登录（或注册） =="
if [ -n "$EMAIL" ] && [ -n "$PASSWORD" ]; then
  LOGIN=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")
  check "登录成功" '"success":true' "$LOGIN"
else
  EMAIL="kcli-e2e-$(date +%s)@test.local"
  REG=$(post "/auth/register" "{\"email\":\"$EMAIL\",\"password\":\"secret123\"}")
  check "注册成功（需 SMTP 配置）" '"success":true' "$REG"
  LOGIN=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"secret123\"}")
fi
TOKEN=$(echo "$LOGIN" | python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["token"])')
check "登录拿到 token" '^ey' "$TOKEN"
AUTH="Authorization: Bearer $TOKEN"

echo "== 1. 项目 + Secret Key =="
PROJ=$(post "/projects" '{"name":"知识库CLI e2e","description":"knowledge cli create/update e2e"}')
PID=$(echo "$PROJ" | jid)
check "项目创建" '"success":true' "$PROJ"
SK=$(post "/projects/$PID/secret-key/generate" '{}' | python3 -c 'import sys,json;d=json.load(sys.stdin)["data"];print(d.get("secretKey") or "")')
check "取得项目 Secret Key" "^sk_" "$SK"

export CHUNSUN_API_URL="$BASE"
export CHUNSUN_SECRET_KEY="$SK"

echo "== 2. CLI create：创建 eager 文档并读回 =="
CREATE1=$("$CLI" knowledge create --title "CLI 部署手册" --content "第一步备份数据库" --json 2>&1)
check "create 退出码 0" "0" "$?"
check "create 返回文档 ID" '"id": "' "$CREATE1"
check "create 返回标题" 'CLI 部署手册' "$CREATE1"
check "create 默认 eager" '"loadStrategy": "eager"' "$CREATE1"
DOC1=$(echo "$CREATE1" | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')
check "解析到文档 ID" "$DOC1" "$DOC1"

DOC_GET=$("$CLI" knowledge doc "$DOC1" 2>&1)
check "doc 回读标题" 'CLI 部署手册' "$DOC_GET"
check "doc 回读正文" '第一步备份数据库' "$DOC_GET"

IDX=$("$CLI" knowledge index 2>&1)
check "index 含新文档" 'CLI 部署手册' "$IDX"

echo "== 3. CLI create --strategy lazy：过滤互斥 =="
CREATE2=$("$CLI" knowledge create --title "CLI 参考手册" --content "仅按需加载" --strategy lazy --json 2>&1)
check "create lazy 退出码 0" "0" "$?"
check "create lazy 生效" '"loadStrategy": "lazy"' "$CREATE2"
DOC2=$(echo "$CREATE2" | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')

LAZY_LIST=$("$CLI" knowledge --strategy lazy 2>&1)
check "lazy 过滤退出码 0" "0" "$?"
check "lazy 过滤含 lazy 文档" 'CLI 参考手册' "$LAZY_LIST"
EAGER_LIST=$("$CLI" knowledge --strategy eager 2>&1)
check "eager 过滤退出码 0" "0" "$?"
check_absent "eager 过滤不含 lazy 文档" 'CLI 参考手册' "$EAGER_LIST"

echo "== 4. CLI update：改 title/content/strategy/sortOrder =="
UPD=$("$CLI" knowledge update "$DOC2" --title "CLI 参考手册 v2" --content "更新后的正文" --strategy eager --sort-order 42 --json 2>&1)
check "update 退出码 0" "0" "$?"
check "update 返回新标题" 'CLI 参考手册 v2' "$UPD"
check "update strategy 生效" '"loadStrategy": "eager"' "$UPD"
check "update sortOrder 生效" '"sortOrder": 42' "$UPD"

DOC_GET2=$("$CLI" knowledge doc "$DOC2" 2>&1)
check "update 后回读标题" 'CLI 参考手册 v2' "$DOC_GET2"
check "update 后回读正文" '更新后的正文' "$DOC_GET2"

IDX2=$("$CLI" knowledge index 2>&1)
check "index 反映新标题" 'CLI 参考手册 v2' "$IDX2"
check "index 反映新策略 eager" 'strategy=eager' "$IDX2"

echo "== 5. 硬化：空更新 / 非法策略 / 不存在的文档 =="
EMPTY_OUT=$("$CLI" knowledge update "$DOC2" 2>&1); EMPTY_CODE=$?
check_exit_nonzero "空更新报错" '至少一个要更新的字段' "$EMPTY_CODE" "$EMPTY_OUT"

BAD_STRAT=$("$CLI" knowledge create --title x --strategy bogus 2>&1); BAD_CODE=$?
check_exit_nonzero "非法 --strategy 报错" '只能是 eager 或 lazy' "$BAD_CODE" "$BAD_STRAT"

MISSING=$("$CLI" knowledge update "no-such-doc" --title x 2>&1); MISS_CODE=$?
check_exit_nonzero "更新不存在文档报错" 'CONTEXT_DOC_NOT_FOUND' "$MISS_CODE" "$MISSING"

echo "== 6. 保持不支持删除：CLI 无 delete =="
HELP=$("$CLI" knowledge --help 2>&1)
check_absent "knowledge help 不含 delete" 'delete' "$HELP"
DEL_OUT=$("$CLI" knowledge delete "$DOC1" 2>&1); DEL_CODE=$?
check_exit_nonzero "delete 子命令不存在" 'unrecognized subcommand' "$DEL_CODE" "$DEL_OUT"

echo "== 7. 后端真相：API 列表包含 CLI 创建的文档 =="
LIST=$(get "/projects/$PID/knowledge/documents")
check "API 列表含 create 文档" 'CLI 部署手册' "$LIST"
check "API 列表含 update 后文档" 'CLI 参考手册 v2' "$LIST"
check "API 列表文档带正文" '更新后的正文' "$LIST"

echo
echo "======== 结果: PASS=$PASS FAIL=$FAIL ========"
[ "$FAIL" -eq 0 ]
