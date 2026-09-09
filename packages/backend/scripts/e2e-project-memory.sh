#!/usr/bin/env bash
# 项目级记忆端到端验收脚本（需求 aKek1T5nctZQ：项目级记忆机制）。
#
# 覆盖验收点：
#   1. GET 无记忆 → 404 MEMORY_NOT_FOUND
#   2. PUT snapshot → GET 回读一致（roundtrip）
#   3. PUT 缺 snapshot 字段 → 400 SNAPSHOT_REQUIRED（拒绝后旧值未破坏）
#   4. PUT 超 10k 字符 → 400 MEMORY_TOO_LARGE（需求工作记忆同限 10k）
#   5. 知识索引固定包含 memory system 条目；知识列表（无策略/eager）包含项目记忆，lazy 不含
#   6. DELETE /memory → 405（无删除端点，仅可编辑不可删除）
#   7. 真实 CLI（Secret Key 通道）：chunsun memory get / put roundtrip
#   8. 需求工作记忆超 10k 同样被拒（容量统一调整）
#
# 前置：一个已启动、迁移就绪的后端实例；环境变量：
#   CHUNSUN_BASE     后端 base URL（默认 http://127.0.0.1:11111/api/v1）
#   CHUNSUN_EMAIL / CHUNSUN_PASSWORD  已有已激活账号（否则走注册，需 SMTP 配置）
#   CHUNSUN_CLI_BIN  本地编译的 CLI 二进制（默认 packages/cli/target/debug/chunsun）
#
# 用法：
#   CHUNSUN_BASE=... CHUNSUN_EMAIL=... CHUNSUN_PASSWORD=... ./scripts/e2e-project-memory.sh

set -u

BASE="${CHUNSUN_BASE:-http://127.0.0.1:11111/api/v1}"
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

post() { curl -s -X POST "$BASE$1" ${AUTH:+-H "$AUTH"} -H "$JSON" -d "$2"; }
get()  { curl -s "$BASE$1" ${AUTH:+-H "$AUTH"}; }
put()  { curl -s -X PUT "$BASE$1" ${AUTH:+-H "$AUTH"} -H "$JSON" -d "$2"; }
code() { curl -s -o /dev/null -w '%{http_code}' -X "$2" "$BASE$1" ${3:+-H "$3"} ${4:+-H "$4"} ${5:+-d "$5"}; }
jid()  { python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["id"])'; }

echo "== 0. 登录（或注册） =="
if [ -n "$EMAIL" ] && [ -n "$PASSWORD" ]; then
  LOGIN=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")
  check "登录成功" '"success":true' "$LOGIN"
else
  EMAIL="pm-e2e-$(date +%s)@test.local"
  REG=$(post "/auth/register" "{\"email\":\"$EMAIL\",\"password\":\"secret123\"}")
  check "注册成功（需 SMTP 配置）" '"success":true' "$REG"
  LOGIN=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"secret123\"}")
fi
TOKEN=$(echo "$LOGIN" | python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["token"])')
check "登录拿到 token" '^ey' "$TOKEN"
AUTH="Authorization: Bearer $TOKEN"

echo "== 1. 项目 + 需求 =="
PROJ=$(post "/projects" '{"name":"项目记忆e2e","description":"project memory e2e"}')
PID=$(echo "$PROJ" | jid)
check "项目创建" '"success":true' "$PROJ"
RID=$(post "/projects/$PID/requirements" '{"description":"项目记忆e2e需求"}' | jid)
check "需求创建" "$RID" "$RID"

echo "== 2. 无记忆时 GET → 404 MEMORY_NOT_FOUND =="
CODE_GET_MISSING=$(code "/projects/$PID/memory" GET "$AUTH")
check "GET 无记忆返回 404" "404" "$CODE_GET_MISSING"
BODY_GET_MISSING=$(get "/projects/$PID/memory")
check "错误码 MEMORY_NOT_FOUND" "MEMORY_NOT_FOUND" "$BODY_GET_MISSING"

echo "== 3. roundtrip：PUT snapshot → GET 回读一致 =="
SNAP='## 填坑记录\n- 部署前必须全量备份数据库\n\n## 经验沉淀\n- 迁移用独立文件，不改已应用迁移的哈希'
PUT_RES=$(put "/projects/$PID/memory" "{\"snapshot\":\"$SNAP\"}")
check "PUT 项目记忆成功" '"success":true' "$PUT_RES"
MEM=$(get "/projects/$PID/memory")
check "回读填坑记录" '全量备份数据库' "$MEM"
check "回读经验沉淀" '独立文件' "$MEM"
check "回读 updatedAt" '"updatedAt"' "$MEM"

echo "== 4. 硬化：缺 snapshot → 400；超 10k → 400 =="
CODE_MISSING=$(code "/projects/$PID/memory" PUT "$AUTH" "$JSON" '{}')
check "缺 snapshot 返回 400" "400" "$CODE_MISSING"
BODY_MISSING=$(put "/projects/$PID/memory" '{}')
check "错误码 SNAPSHOT_REQUIRED" "SNAPSHOT_REQUIRED" "$BODY_MISSING"
MEM_AFTER=$(get "/projects/$PID/memory")
check "拒绝后旧记忆未被破坏" '全量备份数据库' "$MEM_AFTER"
BIG=$(python3 -c 'import json;print(json.dumps({"snapshot":"x"*10001}))')
CODE_BIG=$(code "/projects/$PID/memory" PUT "$AUTH" "$JSON" "$BIG")
check "超 10k 返回 400" "400" "$CODE_BIG"
BODY_BIG=$(put "/projects/$PID/memory" "$BIG")
check "错误码 MEMORY_TOO_LARGE" "MEMORY_TOO_LARGE" "$BODY_BIG"

echo "== 5. 属于知识库：索引固定 memory 条目；列表含内容；lazy 不含 =="
IDX=$(get "/projects/$PID/knowledge/index")
check "索引含 memory 条目" '"key":"memory"' "$IDX"
check "索引 memory 为 system" '"system":true' "$IDX"
check "索引 memory 恒 eager" '"loadStrategy":"eager"' "$IDX"
LIST=$(get "/projects/$PID/knowledge/documents")
check "知识列表含项目记忆" '项目记忆' "$LIST"
check "知识列表记忆带正文" '全量备份数据库' "$LIST"
EAGER=$(get "/projects/$PID/knowledge/documents?strategy=eager")
check "eager 含项目记忆" '项目记忆' "$EAGER"
LAZY=$(get "/projects/$PID/knowledge/documents?strategy=lazy")
check_absent "lazy 不含项目记忆" '项目记忆' "$LAZY"

echo "== 6. 仅可编辑不可删除：DELETE → 405 =="
CODE_DEL=$(code "/projects/$PID/memory" DELETE "$AUTH")
check "DELETE 返回 405" "405" "$CODE_DEL"
MEM_AFTER_DEL=$(get "/projects/$PID/memory")
check "DELETE 后记忆仍在" '全量备份数据库' "$MEM_AFTER_DEL"

echo "== 7. 真实 CLI：chunsun memory get / put =="
if [ ! -x "$CLI" ]; then
  echo "  - CLI 二进制不可用（$CLI），跳过 7/8 节"; FAIL=$((FAIL + 1))
else
  SK=$(post "/projects/$PID/secret-key/generate" '{}' | python3 -c 'import sys,json;d=json.load(sys.stdin)["data"];print(d.get("secretKey") or "")' 2>/dev/null)
  check "取得项目 Secret Key" "^sk_" "$SK"

  export CHUNSUN_API_URL="$BASE"
  export CHUNSUN_SECRET_KEY="$SK"
  CLI_GET=$("$CLI" memory get 2>&1)
  check "CLI memory get 退出码 0" "0" "$?"
  check "CLI memory get 读到记忆" '全量备份数据库' "$CLI_GET"
  "$CLI" memory put --snapshot '## 填坑记录\n- 部署前必须全量备份数据库\n- 新增迁移必须独立文件\n\n## 经验沉淀\n- 容量上限统一 10k' >/dev/null 2>&1
  check "CLI memory put 退出码 0" "0" "$?"
  CLI_GET2=$("$CLI" memory get 2>&1)
  check "CLI put 后回读新条目" '容量上限统一 10k' "$CLI_GET2"
  check "CLI put 后旧条目保留（全量覆盖新文本）" '全量备份数据库' "$CLI_GET2"
  CLI_JSON=$("$CLI" memory get --json 2>&1)
  check "CLI memory get --json 输出" '"snapshot"' "$CLI_JSON"

  echo "== 8. 需求工作记忆容量同改 10k =="
  BIG_REQ=$(python3 -c 'import json;print(json.dumps({"snapshot":"y"*10001}))')
  CODE_BIG_REQ=$(code "/projects/$PID/requirements/$RID/memory" PUT "$AUTH" "$JSON" "$BIG_REQ")
  check "需求记忆超 10k 返回 400" "400" "$CODE_BIG_REQ"
  BODY_BIG_REQ=$(put "/projects/$PID/requirements/$RID/memory" "$BIG_REQ")
  check "错误码 MEMORY_TOO_LARGE" "MEMORY_TOO_LARGE" "$BODY_BIG_REQ"
  "$CLI" requirement memory put "$RID" --snapshot '## 需求边界\n项目记忆e2e需求' >/dev/null 2>&1
  check "CLI 需求记忆 put 退出码 0" "0" "$?"
  REQ_GET=$("$CLI" requirement memory get "$RID" 2>&1)
  check "CLI 需求记忆回读" '项目记忆e2e需求' "$REQ_GET"
fi

echo
echo "======== 结果: PASS=$PASS FAIL=$FAIL ========"
[ "$FAIL" -eq 0 ]
