#!/usr/bin/env bash
# 依赖关系（Blocking / Blocked By + DAG）端到端验收脚本。
#
# 覆盖验收点：
#   1. 需求/缺陷详情页可添加、移除 Blocking / Blocked By 关系（POST/DELETE 端点）
#   2. 建立循环依赖时系统拒绝并提示（409 DEPENDENCY_CYCLE）
#   3. 详情页展示 Blocking / Blocked By 列表（GET 单节点端点）
#   4. API 查询某节点的直接依赖和传递依赖路径（blocking/blockedBy/transitive*）
#   5. 删除节点级联清理依赖边
#   6. PATCH blockedBy 全量替换：新增 / 清空 / 成环 409 / 同行更新与边替换同事务回滚 / 引用不存在 404（需求 + 缺陷）
#   7. 需求删除状态护栏：pending 可删；running / completed 409 REQUIREMENT_DELETE_NOT_ALLOWED；abandoned 可删
#   8. GET /dependencies 每边附 sourceDone 标注（列表「被阻塞」口径）：需求 completed / 缺陷 resolved 置 true；abandoned 不视为完成
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

check_absent() { # name, unexpected_substr, actual
  if echo "$3" | grep -q "$2"; then
    printf '  ✗ %s\n    不应含: %s\n    实际: %s\n' "$1" "$2" "$3"; FAIL=$((FAIL + 1))
  else
    printf '  ✓ %s\n' "$1"; PASS=$((PASS + 1))
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
# 注意：macOS 自带 bash 3.2 对「check 参数位置的 $(...) 内嵌套双引号 JSON」有解析缺陷
# （花括号被当作 brace expansion 拆参）；载荷统一先落变量再进 check（e2e-cli-management.sh 同规）。
EDGE_AB=$(post "/projects/$PID/dependencies" "{\"sourceType\":\"requirement\",\"sourceId\":\"$AID\",\"targetType\":\"requirement\",\"targetId\":\"$BID\"}")
check "A->B" '"success":true' "$EDGE_AB"
EDGE_BC=$(post "/projects/$PID/dependencies" "{\"sourceType\":\"requirement\",\"sourceId\":\"$BID\",\"targetType\":\"requirement\",\"targetId\":\"$CID\"}")
check "B->C" '"success":true' "$EDGE_BC"
EDGE_BD=$(post "/projects/$PID/dependencies" "{\"sourceType\":\"requirement\",\"sourceId\":\"$BID\",\"targetType\":\"defect\",\"targetId\":\"$DID\"}")
check "B->D 跨类型" '"success":true' "$EDGE_BD"

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

patch() { curl -s -X PATCH "$BASE$1" -H "$AUTH" -H "$JSON" -d "$2"; }

echo "== 7. PATCH blockedBy 全量替换（需求） =="
patch_ret=$(patch "/projects/$PID/requirements/$AID" "{\"blockedBy\":[{\"kind\":\"requirement\",\"id\":\"$CID\"}]}")
check "PATCH A blockedBy=[C]" '"success":true' "$patch_ret"
GA3=$(get "/projects/$PID/dependencies/requirement/$AID")
check "A.blockedBy 含 C（新增入边）" "$CID" "$GA3"
check "A 出边 A->B 不受替换影响" "$BID" "$GA3"

cycle_ret=$(patch "/projects/$PID/requirements/$CID" "{\"blockedBy\":[{\"kind\":\"requirement\",\"id\":\"$AID\"}]}")
check "PATCH 成环 409 DEPENDENCY_CYCLE" "DEPENDENCY_CYCLE" "$cycle_ret"

atomic_ret=$(patch "/projects/$PID/requirements/$CID" "{\"status\":\"abandoned\",\"blockedBy\":[{\"kind\":\"requirement\",\"id\":\"$AID\"}]}")
check "成环时同事务回滚（仍返回 DEPENDENCY_CYCLE）" "DEPENDENCY_CYCLE" "$atomic_ret"
CST=$(get "/projects/$PID/requirements/$CID")
check "回滚：C.status 仍为 pending（行更新未半提交）" '"status":"pending"' "$CST"

clear_ret=$(patch "/projects/$PID/requirements/$AID" '{"blockedBy":[]}')
check "PATCH 空数组清空 A.blockedBy" '"success":true' "$clear_ret"
GA4=$(get "/projects/$PID/dependencies/requirement/$AID")
check_absent "清空后 A.blockedBy 不含 C" "$CID" "$GA4"

badref=$(patch "/projects/$PID/requirements/$AID" '{"blockedBy":[{"kind":"requirement","id":"no-such-node"}]}')
check "引用不存在 404 DEPENDENCY_TARGET_NOT_FOUND" "DEPENDENCY_TARGET_NOT_FOUND" "$badref"

echo "== 8. PATCH blockedBy 全量替换（缺陷） =="
patch_d=$(patch "/projects/$PID/defects/$DID" "{\"blockedBy\":[{\"kind\":\"requirement\",\"id\":\"$CID\"}]}")
check "PATCH D blockedBy=[C]" '"success":true' "$patch_d"
GD=$(get "/projects/$PID/dependencies/defect/$DID")
check "D.blockedBy 含 C" "$CID" "$GD"
patch_d2=$(patch "/projects/$PID/defects/$DID" '{"blockedBy":[]}')
check "清空 D.blockedBy" '"success":true' "$patch_d2"
GD2=$(get "/projects/$PID/dependencies/defect/$DID")
check_absent "清空后 D.blockedBy 不含 C" "$CID" "$GD2"

echo "== 9. 需求删除状态护栏 =="
EID=$(post "/projects/$PID/requirements" '{"description":"待删需求E(pending)"}' | jid)
check "pending 可删(200)" "200" "$(curl -s -o /dev/null -w '%{http_code}' -X DELETE "$BASE/projects/$PID/requirements/$EID" -H "$AUTH")"

FID=$(post "/projects/$PID/requirements" '{"description":"待删需求F(running)"}' | jid)
patch "/projects/$PID/requirements/$FID" '{"status":"running"}' >/dev/null
GID=$(post "/projects/$PID/requirements" '{"description":"待删需求G(completed)"}' | jid)
patch "/projects/$PID/requirements/$GID" '{"status":"completed"}' >/dev/null
DEL_F_CODE=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE "$BASE/projects/$PID/requirements/$FID" -H "$AUTH")
DEL_F_BODY=$(curl -s -X DELETE "$BASE/projects/$PID/requirements/$FID" -H "$AUTH")
check "running 拒删(409)" "409" "$DEL_F_CODE"
check "running 拒删错误码" "REQUIREMENT_DELETE_NOT_ALLOWED" "$DEL_F_BODY"
DEL_G_BODY=$(del "/projects/$PID/requirements/$GID")
check "completed 拒删" "REQUIREMENT_DELETE_NOT_ALLOWED" "$DEL_G_BODY"

patch "/projects/$PID/requirements/$FID" '{"status":"abandoned"}' >/dev/null
check "abandoned 可删(200)" "200" "$(curl -s -o /dev/null -w '%{http_code}' -X DELETE "$BASE/projects/$PID/requirements/$FID" -H "$AUTH")"

echo "== 10. 删除节点级联清理 =="
if [ -n "${CHUNSUN_DB_URL:-}" ]; then
  del "/projects/$PID/requirements/$BID" >/dev/null
  CNT=$(psql "$CHUNSUN_DB_URL" -Atc "SELECT count(*) FROM dependency WHERE project_id='$PID'")
  check "删 B 后依赖边清空" "0" "$CNT"
else
  echo "  - CHUNSUN_DB_URL 未设置，跳过级联删除断言"
fi

echo "== 11. /dependencies sourceDone（列表「被阻塞」口径） =="
SID=$(post "/projects/$PID/requirements" '{"description":"阻塞源S"}' | jid)
T1ID=$(post "/projects/$PID/requirements" '{"description":"目标T1"}' | jid)
T2ID=$(post "/projects/$PID/defects" '{"description":"目标缺陷T2"}' | jid)
DSID=$(post "/projects/$PID/defects" '{"description":"缺陷源DS"}' | jid)
T3ID=$(post "/projects/$PID/requirements" '{"description":"目标T3"}' | jid)

EDGE_S1=$(post "/projects/$PID/dependencies" "{\"sourceType\":\"requirement\",\"sourceId\":\"$SID\",\"targetType\":\"requirement\",\"targetId\":\"$T1ID\"}")
check "建边 S->T1" '"success":true' "$EDGE_S1"
EDGE_S2=$(post "/projects/$PID/dependencies" "{\"sourceType\":\"requirement\",\"sourceId\":\"$SID\",\"targetType\":\"defect\",\"targetId\":\"$T2ID\"}")
check "建边 S->T2（跨类型）" '"success":true' "$EDGE_S2"
EDGE_D3=$(post "/projects/$PID/dependencies" "{\"sourceType\":\"defect\",\"sourceId\":\"$DSID\",\"targetType\":\"requirement\",\"targetId\":\"$T3ID\"}")
check "建边 DS->T3（缺陷源）" '"success":true' "$EDGE_D3"

edge_flag() { # $1=列表JSON $2=sourceId $3=targetId
  echo "$1" | python3 -c "import sys,json;d=json.load(sys.stdin)['data'];print([e['sourceDone'] for e in d if e['sourceId']=='$2' and e['targetId']=='$3'][0])"
}

LIST_1=$(get "/projects/$PID/dependencies")
check "列表边均含 sourceDone 字段" '"sourceDone"' "$LIST_1"
check "S pending：S->T1 sourceDone=false" "False" "$(edge_flag "$LIST_1" "$SID" "$T1ID")"
check "S pending：S->T2 sourceDone=false" "False" "$(edge_flag "$LIST_1" "$SID" "$T2ID")"
check "DS open：DS->T3 sourceDone=false" "False" "$(edge_flag "$LIST_1" "$DSID" "$T3ID")"

patch "/projects/$PID/requirements/$SID" '{"status":"completed"}' >/dev/null
patch "/projects/$PID/defects/$DSID" '{"status":"resolved"}' >/dev/null
LIST_2=$(get "/projects/$PID/dependencies")
check "S completed：S->T1 sourceDone=true" "True" "$(edge_flag "$LIST_2" "$SID" "$T1ID")"
check "S completed：S->T2 sourceDone=true" "True" "$(edge_flag "$LIST_2" "$SID" "$T2ID")"
check "DS resolved：DS->T3 sourceDone=true" "True" "$(edge_flag "$LIST_2" "$DSID" "$T3ID")"

SAID=$(post "/projects/$PID/requirements" '{"description":"弃用源SA"}' | jid)
T4ID=$(post "/projects/$PID/requirements" '{"description":"目标T4"}' | jid)
EDGE_SA4=$(post "/projects/$PID/dependencies" "{\"sourceType\":\"requirement\",\"sourceId\":\"$SAID\",\"targetType\":\"requirement\",\"targetId\":\"$T4ID\"}")
check "建边 SA->T4" '"success":true' "$EDGE_SA4"
patch "/projects/$PID/requirements/$SAID" '{"status":"abandoned"}' >/dev/null
LIST_3=$(get "/projects/$PID/dependencies")
check "SA abandoned：SA->T4 sourceDone=false（abandoned 不算完成）" "False" "$(edge_flag "$LIST_3" "$SAID" "$T4ID")"

echo
echo "======== 结果: PASS=$PASS FAIL=$FAIL ========"
[ "$FAIL" -eq 0 ]
