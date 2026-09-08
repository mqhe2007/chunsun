#!/usr/bin/env bash
# 工作记忆机制端到端验收脚本（缺陷 dutztumK-Sjz：需求的工作记忆机制没有生效）。
#
# 覆盖验收点：
#   1. PUT 全五字段 snapshot → GET 回读五字段一致（roundtrip）
#   2. PUT 缺 snapshot 字段 → 400 SNAPSHOT_REQUIRED（不再静默回旧行）
#   3. PUT 超 20k 字符 → 400 MEMORY_TOO_LARGE
#   4. 真实 CLI（Secret Key 通道）memory put 顶层 key 增量合并：两次 put 不同 key，既有 key 保留
#   5. 真实 CLI run remind：running Run 且本轮未写记忆 → 提醒；本轮写入后 → 提醒消失
#
# 前置：一个已启动、迁移就绪的后端实例；环境变量：
#   CHUNSUN_BASE     后端 base URL（默认 http://127.0.0.1:18999/api/v1）
#   CHUNSUN_TOKEN    已激活账号的 JWT（Bearer，需为所建项目 Owner）
#   CHUNSUN_CLI_BIN  本地编译的 CLI 二进制（默认 packages/cli/target/debug/chunsun）
#
# 用法：
#   CHUNSUN_BASE=... CHUNSUN_TOKEN=... ./scripts/e2e-memory.sh

set -u

BASE="${CHUNSUN_BASE:-http://127.0.0.1:18999/api/v1}"
TOKEN="${CHUNSUN_TOKEN:?CHUNSUN_TOKEN 未设置}"
CLI="${CHUNSUN_CLI_BIN:-$(dirname "$0")/../../cli/target/debug/chunsun}"
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
put()  { curl -s -X PUT "$BASE$1" -H "$AUTH" -H "$JSON" -d "$2"; }
jid()  { python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["id"])'; }

echo "== 0. 项目 + 需求 =="
PROJ=$(post "/projects" '{"name":"工作记忆e2e","description":"memory mechanism e2e"}')
PID=$(echo "$PROJ" | jid)
check "项目创建" '"success":true' "$PROJ"
RID=$(post "/projects/$PID/requirements" '{"description":"工作记忆e2e需求"}' | jid)
check "需求创建" "$RID" "$RID"

echo "== 1. roundtrip：PUT 五字段 → GET 回读一致 =="
FIVE='{"snapshot":{
  "requirementSnapshot":{"scope":"只优化记忆机制","outOfScope":"不改场景/用例表"},
  "lastRunSummary":{"did":"四端改动","decisions":"remind 软提醒而非硬门禁","next":"部署后观察"},
  "openDecisions":[{"question":"20k 上限是否可调？","raisedAt":"2026-09-08T05:00:00Z"}],
  "codeLandmarks":[{"path":"packages/backend/src/routes/harness.rs","symbol":"put_memory","note":"入参硬化"}],
  "envRefs":["DATABASE_URL","JWT_SECRET"]
}}'
PUT_RES=$(put "/projects/$PID/requirements/$RID/memory" "$FIVE")
check "PUT 五字段成功" '"success":true' "$PUT_RES"
MEM=$(get "/projects/$PID/requirements/$RID/memory")
check "回读 requirementSnapshot" '只优化记忆机制' "$MEM"
check "回读 lastRunSummary" 'remind 软提醒而非硬门禁' "$MEM"
check "回读 openDecisions" '20k 上限是否可调' "$MEM"
check "回读 codeLandmarks" 'put_memory' "$MEM"
check "回读 envRefs" 'DATABASE_URL' "$MEM"
check "回读 updatedAt" '"updatedAt"' "$MEM"

echo "== 2. 硬化：缺 snapshot → 400；超 20k → 400 =="
CODE_MISSING=$(curl -s -o /dev/null -w '%{http_code}' -X PUT "$BASE/projects/$PID/requirements/$RID/memory" -H "$AUTH" -H "$JSON" -d '{}')
BODY_MISSING=$(put "/projects/$PID/requirements/$RID/memory" '{}')
check "缺 snapshot 返回 400" "400" "$CODE_MISSING"
check "错误码 SNAPSHOT_REQUIRED" "SNAPSHOT_REQUIRED" "$BODY_MISSING"
MEM_AFTER=$(get "/projects/$PID/requirements/$RID/memory")
check "拒绝后旧记忆未被破坏" '只优化记忆机制' "$MEM_AFTER"
BIG=$(python3 -c 'import json;print(json.dumps({"snapshot":{"blob":"x"*21000}}))')
CODE_BIG=$(curl -s -o /dev/null -w '%{http_code}' -X PUT "$BASE/projects/$PID/requirements/$RID/memory" -H "$AUTH" -H "$JSON" -d "$BIG")
BODY_BIG=$(put "/projects/$PID/requirements/$RID/memory" "$BIG")
check "超 20k 返回 400" "400" "$CODE_BIG"
check "错误码 MEMORY_TOO_LARGE" "MEMORY_TOO_LARGE" "$BODY_BIG"

echo "== 3. 真实 CLI：memory put 增量合并 =="
if [ ! -x "$CLI" ]; then
  echo "  - CLI 二进制不可用（$CLI），跳过 3/4 节"; FAIL=$((FAIL + 1))
else
  SK_RES=$(get "/projects/$PID/secret-key")
  SK=$(echo "$SK_RES" | python3 -c 'import sys,json;d=json.load(sys.stdin)["data"];print(d.get("secretKey") or "")' 2>/dev/null)
  if [ -z "$SK" ]; then
    SK=$(post "/projects/$PID/secret-key/generate" '{}' | python3 -c 'import sys,json;d=json.load(sys.stdin)["data"];print(d.get("secretKey") or "")' 2>/dev/null)
  fi
  check "取得项目 Secret Key" "^sk_" "$SK"

  export CHUNSUN_API_URL="$BASE"
  export CHUNSUN_SECRET_KEY="$SK"
  "$CLI" requirement memory put "$RID" --snapshot '{"lastRunSummary":{"did":"第二次写入"}}' >/dev/null 2>&1
  check "CLI put 退出码 0" "0" "$?"
  MEM2=$(get "/projects/$PID/requirements/$RID/memory")
  check "CLI put 后新 key 生效" '第二次写入' "$MEM2"
  check "CLI 合并保留 requirementSnapshot" '只优化记忆机制' "$MEM2"
  check "CLI 合并保留 envRefs" 'DATABASE_URL' "$MEM2"

  echo "== 4. 真实 CLI：remind 本轮未写记忆提醒 =="
  RID2=$(post "/projects/$PID/requirements" '{"description":"remind验证需求"}' | jid)
  RUN_RES=$(post "/projects/$PID/requirements/$RID2/runs" '{}')
  check "开启 Run" '"success":true' "$RUN_RES"
  REMIND_BEFORE=$("$CLI" run remind "$RID2" 2>&1)
  check "未写记忆时 remind 提醒" "尚未写入工作记忆" "$REMIND_BEFORE"
  sleep 2  # 秒级判定：put 须晚于 Run 开始至少 1 秒
  "$CLI" requirement memory put "$RID2" --snapshot '{"lastRunSummary":{"did":"remind验证"}}' >/dev/null 2>&1
  REMIND_AFTER=$("$CLI" run remind "$RID2" 2>&1)
  check_absent "写入后 remind 不再提醒" "尚未写入工作记忆" "$REMIND_AFTER"
fi

echo
echo "======== 结果: PASS=$PASS FAIL=$FAIL ========"
[ "$FAIL" -eq 0 ]
