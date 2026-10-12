#!/usr/bin/env bash
# CLI 管理面端到端验收脚本（需求 g7eg0oqNTXSH：需求删除 + 依赖边增删）。
#
# 覆盖验收点：
#   1. CLI requirement delete：pending 删除成功（含 --json）；已删再删 404 REQUIREMENT_NOT_FOUND
#   2. 状态护栏：running / completed 拒删且可读；abandoned 可删
#   3. CLI dependency add：A 阻塞 B 后 dependency blocked / list 反映阻塞
#   4. 错误面：成环 / 自环拒绝、非法节点类型、节点不存在、边不存在（均非零退出 + 可读文案）
#   5. CLI dependency remove：移除后阻塞解除
#   6. 后端真相：API 侧节点/边与 CLI 行为一致（删除级联、列表为空）
#
# 前置：一个已启动、迁移就绪的后端实例（本地 dev：127.0.0.1:11112）；环境变量：
#   CHUNSUN_BASE     后端 base URL（默认 http://127.0.0.1:11112/api/v1）
#   CHUNSUN_EMAIL / CHUNSUN_PASSWORD  已激活账号（本地 dev 别走注册，SMTP 未配置会整单回滚）
#   CHUNSUN_CLI_BIN  本地编译的 CLI 二进制（默认 packages/cli/target/debug/chunsun）
#
# 用法：
#   CHUNSUN_EMAIL=e2e-kcli@test.local CHUNSUN_PASSWORD=e2e-secret-123 ./scripts/e2e-cli-management.sh

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
jdata_field() { python3 -c "import sys,json;print(json.load(sys.stdin)['data'].get('$1') or '')"; }

if [ -z "$EMAIL" ] || [ -z "$PASSWORD" ]; then
  echo "需要 CHUNSUN_EMAIL / CHUNSUN_PASSWORD（本地 dev 账号，例如 e2e-kcli@test.local）"
  exit 1
fi

echo "== 0. 登录 =="
LOGIN=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")
check "登录成功" '"success":true' "$LOGIN"
TOKEN=$(echo "$LOGIN" | python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["token"])')
AUTH="Authorization: Bearer $TOKEN"
check "拿到 token" '^ey' "$TOKEN"

echo "== 1. 项目 + Secret Key =="
PROJ=$(post "/projects" '{"name":"CLI管理面 e2e","description":"cli management e2e"}')
PID=$(echo "$PROJ" | jid)
check "项目创建" '"success":true' "$PROJ"
SK=$(post "/projects/$PID/secret-key/generate" '{}' | jdata_field secretKey)
check "取得项目 Secret Key" "^sk_" "$SK"

export CHUNSUN_API_URL="$BASE"
export CHUNSUN_SECRET_KEY="$SK"

echo "== 2. CLI 创建需求 A / B =="
A_JSON=$("$CLI" requirement create --description "CLI 需求A" --json 2>&1)
check "create A 退出码 0" "0" "$?"
AID=$(echo "$A_JSON" | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')
check "解析 A ID" '^..*' "$AID"
B_JSON=$("$CLI" requirement create --description "CLI 需求B" --json 2>&1)
BID=$(echo "$B_JSON" | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')
check "解析 B ID" '^..*' "$BID"

echo "== 3. dependency add：A 阻塞 B（文本 + --json） =="
ADD=$("$CLI" dependency add requirement "$AID" requirement "$BID" 2>&1); ADD_CODE=$?
check "add 退出码 0" "0" "$ADD_CODE"
check "add 输出提示" '已添加依赖' "$ADD"
BLOCKED=$("$CLI" dependency blocked requirement "$BID" 2>&1)
check "B 被阻塞" '被阻塞' "$BLOCKED"
check "B 阻塞原因含 A" "$AID" "$BLOCKED"
LIST=$("$CLI" dependency list 2>&1)
check "list 含 A/B 边" "$AID" "$LIST"
check "list 含目标 B" "$BID" "$LIST"

EID=$("$CLI" requirement create --description "CLI 需求E(--json 边)" --json 2>&1 | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')
ADD_J=$("$CLI" dependency add --json requirement "$AID" requirement "$EID" 2>&1); ADD_J_CODE=$?
check "--json add 退出码 0" "0" "$ADD_J_CODE"
check "--json 输出含 sourceType" '"sourceType": "requirement"' "$ADD_J"
check "--json 输出含 targetId E" "$EID" "$ADD_J"

echo "== 4. 错误面：成环 / 自环 / 非法类型 / 不存在节点 =="
CYCLE_OUT=$("$CLI" dependency add requirement "$BID" requirement "$AID" 2>&1); CYCLE_CODE=$?
check_exit_nonzero "成环拒绝（可读）" '循环依赖' "$CYCLE_CODE" "$CYCLE_OUT"
SELF_OUT=$("$CLI" dependency add requirement "$BID" requirement "$BID" 2>&1); SELF_CODE=$?
check_exit_nonzero "自环拒绝（可读）" '循环依赖' "$SELF_CODE" "$SELF_OUT"
BADTYPE_OUT=$("$CLI" dependency add requirement "$AID" task "$BID" 2>&1); BADTYPE_CODE=$?
check_exit_nonzero "非法节点类型" 'target_type 只能是' "$BADTYPE_CODE" "$BADTYPE_OUT"
MISSING_OUT=$("$CLI" dependency add requirement no-such-node requirement "$BID" 2>&1); MISSING_CODE=$?
check_exit_nonzero "节点不存在" 'DEPENDENCY_SOURCE_NOT_FOUND' "$MISSING_CODE" "$MISSING_OUT"

echo "== 5. dependency remove：解除阻塞 =="
RM=$("$CLI" dependency remove requirement "$AID" requirement "$BID" 2>&1); RM_CODE=$?
check "remove 退出码 0" "0" "$RM_CODE"
check "remove 输出提示" '已移除依赖' "$RM"
UNB=$("$CLI" dependency blocked requirement "$BID" 2>&1)
check "B 不再被阻塞" '可执行' "$UNB"
RM2_OUT=$("$CLI" dependency remove requirement "$AID" requirement "$BID" 2>&1); RM2_CODE=$?
check_exit_nonzero "重复移除报错" '依赖边不存在' "$RM2_CODE" "$RM2_OUT"

echo "== 6. requirement delete 状态护栏 =="
DEL_OK=$("$CLI" requirement delete "$AID" 2>&1); DEL_OK_CODE=$?
check "pending 删除退出码 0" "0" "$DEL_OK_CODE"
check "pending 删除提示" '需求已删除' "$DEL_OK"
DEL404=$("$CLI" requirement delete "$AID" 2>&1); DEL404_CODE=$?
check_exit_nonzero "重复删除报 404" 'REQUIREMENT_NOT_FOUND' "$DEL404_CODE" "$DEL404"

CID=$("$CLI" requirement create --description "CLI 需求C(running→abandoned)" --json 2>&1 | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')
check "解析 C ID" '^..*' "$CID"
"$CLI" requirement update "$CID" --status running >/dev/null 2>&1
DEL_RUN=$("$CLI" requirement delete "$CID" 2>&1); DEL_RUN_CODE=$?
check_exit_nonzero "running 拒删（护栏可读）" '仅 pending / abandoned' "$DEL_RUN_CODE" "$DEL_RUN"

DID=$("$CLI" requirement create --description "CLI 需求D(completed)" --json 2>&1 | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')
"$CLI" requirement update "$DID" --status completed >/dev/null 2>&1
DEL_COMP=$("$CLI" requirement delete "$DID" 2>&1); DEL_COMP_CODE=$?
check_exit_nonzero "completed 拒删（护栏可读）" '仅 pending / abandoned' "$DEL_COMP_CODE" "$DEL_COMP"

"$CLI" requirement update "$CID" --status abandoned >/dev/null 2>&1
DEL_AB=$("$CLI" requirement delete "$CID" 2>&1); DEL_AB_CODE=$?
check "abandoned 可删" "0" "$DEL_AB_CODE"
check "abandoned 删除提示" '需求已删除' "$DEL_AB"

echo "== 7. 后端真相 =="
CHECK_C=$(get "/projects/$PID/requirements/$CID")
check "C 已从后端删除（404）" 'REQUIREMENT_NOT_FOUND' "$CHECK_C"
DEPS=$(get "/projects/$PID/dependencies")
check_absent "依赖列表已无 A" "$AID" "$DEPS"
check_absent "依赖列表已无 B" "$BID" "$DEPS"

echo
echo "======== 结果: PASS=$PASS FAIL=$FAIL ========"
[ "$FAIL" -eq 0 ]
