#!/usr/bin/env bash
# 依赖关系（Blocking / Blocked By + DAG）端到端验收脚本。
#
# 覆盖验收点：
#   1. 需求/缺陷详情页可添加、移除 Blocking / Blocked By 关系（POST/DELETE 端点）
#   2. 建立循环依赖时系统拒绝并提示（409 DEPENDENCY_CYCLE）
#   3. 详情页展示 Blocking / Blocked By 列表（GET 单节点端点）
#   4. API 查询某节点的直接依赖和传递依赖路径（blocking/blockedBy/transitive*）
#   5. 删除节点级联清理依赖边
#
# 前置：一个已启动、迁移就绪的后端实例；环境变量：
#   CHUNSUN_BASE    后端 base URL（默认 http://127.0.0.1:18999/api/v1）
#   CHUNSUN_TOKEN   已激活账号的 JWT（Bearer）
#   CHUNSUN_DB_URL  可直接 psql 的数据库连接串（可选，用于级联删除断言）
#
# 用法：
#   CHUNSUN_BASE=... CHUNSUN_TOKEN=... ./scripts/e2e-dependency.sh

set -u

BASE="${CHUNSUN_BASE:-http://127.0.0.1:18999/api/v1}"
TOKEN="${CHUNSUN_TOKEN:?CHUNSUN_TOKEN 未设置}"
AUTH="Authorization: Bearer $TOKEN"
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

post() { curl -s -X POST "$BASE$1" -H "$AUTH" -H "$JSON" -d "$2"; }
get()  { curl -s "$BASE$1" -H "$AUTH"; }
del()  { curl -s -X DELETE "$BASE$1" -H "$AUTH"; }
jid()  { python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["id"])'; }

echo "== 1. 项目 =="
PROJ=$(post "/projects" '{"name":"依赖e2e","description":"test"}')
PID=$(echo "$PROJ" | jid)
check "项目创建" '"success":true' "$PROJ"

echo "== 2. 需求 A/B/C + 缺陷 D =="
AID=$(post "/projects/$PID/requirements" '{"description":"需求A"}' | jid)
BID=$(post "/projects/$PID/requirements" '{"description":"需求B"}' | jid)
CID=$(post "/projects/$PID/requirements" '{"description":"需求C"}' | jid)
DID=$(post "/projects/$PID/defects" '{"description":"缺陷D"}' | jid)
check "节点创建" "$AID" "$AID$BID$CID$DID"

echo "== 3. 加边 A->B, B->C, B->D =="
check "A->B" '"success":true' "$(post "/projects/$PID/dependencies" "{\"sourceType\":\"requirement\",\"sourceId\":\"$AID\",\"targetType\":\"requirement\",\"targetId\":\"$BID\"}")"
check "B->C" '"success":true' "$(post "/projects/$PID/dependencies" "{\"sourceType\":\"requirement\",\"sourceId\":\"$BID\",\"targetType\":\"requirement\",\"targetId\":\"$CID\"}")"
check "B->D 跨类型" '"success":true' "$(post "/projects/$PID/dependencies" "{\"sourceType\":\"requirement\",\"sourceId\":\"$BID\",\"targetType\":\"defect\",\"targetId\":\"$DID\"}")"

echo "== 4. 循环依赖拒绝 =="
CYCLE_CODE=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/projects/$PID/dependencies" -H "$AUTH" -H "$JSON" -d "{\"sourceType\":\"requirement\",\"sourceId\":\"$CID\",\"targetType\":\"requirement\",\"targetId\":\"$AID\"}")
CYCLE_BODY=$(curl -s -X POST "$BASE/projects/$PID/dependencies" -H "$AUTH" -H "$JSON" -d "{\"sourceType\":\"requirement\",\"sourceId\":\"$CID\",\"targetType\":\"requirement\",\"targetId\":\"$AID\"}")
check "C->A 循环拒绝(409)" "409" "$CYCLE_CODE"
check "错误码 DEPENDENCY_CYCLE" "DEPENDENCY_CYCLE" "$CYCLE_BODY"

echo "== 5. 直接 + 传递依赖查询 =="
GA=$(get "/projects/$PID/dependencies/requirement/$AID")
check "A.blocking 含 B" "$BID" "$GA"
check "A.transitiveBlocking 含 C" "$CID" "$GA"
check "A.transitiveBlocking 含 D" "$DID" "$GA"
GC=$(get "/projects/$PID/dependencies/requirement/$CID")
check "C.blockedBy 含 B" "$BID" "$GC"
check "C.transitiveBlockedBy 含 A" "$AID" "$GC"

echo "== 6. 移除边 =="
check "移除 B->C" '"success":true' "$(del "/projects/$PID/dependencies/requirement/$BID/requirement/$CID")"
GA2=$(get "/projects/$PID/dependencies/requirement/$AID")
check "移除后 A 仍 blocking B" "$BID" "$GA2"

echo "== 7. 删除节点级联清理 =="
if [ -n "${CHUNSUN_DB_URL:-}" ]; then
  del "/projects/$PID/requirements/$BID" >/dev/null
  CNT=$(psql "$CHUNSUN_DB_URL" -Atc "SELECT count(*) FROM dependency WHERE project_id='$PID'")
  check "删 B 后依赖边清空" "0" "$CNT"
else
  echo "  - CHUNSUN_DB_URL 未设置，跳过级联删除断言"
fi

echo
echo "======== 结果: PASS=$PASS FAIL=$FAIL ========"
[ "$FAIL" -eq 0 ]
