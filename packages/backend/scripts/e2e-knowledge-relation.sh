#!/usr/bin/env bash
# 知识库文档关联关系（主文档 ↔ 分册）端到端验收脚本（需求 AOzsC2VvzMHL）。
#
# 覆盖验收点：
#   1. API create/update：parentId 建立/重挂/解除；详情返回 parentId/breadcrumb/children
#   2. 守卫：不存在的父 / 跨项目父 / 自引用 / 成环 → 4xx，且失败不落库
#   3. index：前序输出（父后紧跟子树）+ parentId/depth
#   4. CLI index 树形文本（└ 缩进、主文档标注「N 分册」）与 --json
#   5. CLI doc：面包屑 + 子文档清单 + 正文，--json 可用
#   6. CLI create/update --parent（含空串解除）
#   7. 删除：有子默认 400 DOC_HAS_CHILDREN；withChildren=true 递归级联（父/子/孙全清，含批注宿主）
#   8. 兼容：无父旧文档 depth=0；strategy 过滤保留绝对 depth
#
# 前置：一个已启动、迁移就绪的后端实例（本地 dev：127.0.0.1:11112）；环境变量：
#   CHUNSUN_BASE     后端 base URL（默认 http://127.0.0.1:11112/api/v1）
#   CHUNSUN_EMAIL / CHUNSUN_PASSWORD  已激活账号（否则走注册，需 SMTP 配置）
#   CHUNSUN_CLI_BIN  本地编译的 CLI 二进制（默认 packages/cli/target/debug/chunsun）
#
# 用法：
#   CHUNSUN_EMAIL=e2e-krel@test.local CHUNSUN_PASSWORD=e2e-secret-123 ./scripts/e2e-knowledge-relation.sh

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

check_exit_zero() { # name, exit_code, output
  if [ "$2" -eq 0 ]; then
    printf '  ✓ %s\n' "$1"; PASS=$((PASS + 1))
  else
    printf '  ✗ %s\n    退出码 %s\n    输出: %s\n' "$1" "$2" "$3"; FAIL=$((FAIL + 1))
  fi
}

post() { curl -s -X POST "$BASE$1" ${AUTH:+-H "$AUTH"} -H "$JSON" -d "$2"; }
get()  { curl -s "$BASE$1" ${AUTH:+-H "$AUTH"}; }
put()  { curl -s -X PUT "$BASE$1" ${AUTH:+-H "$AUTH"} -H "$JSON" -d "$2"; }
del()  { curl -s -X DELETE "$BASE$1" ${AUTH:+-H "$AUTH"}; }
jid()  { python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["id"])'; }
jval() { python3 -c "import sys,json;d=json.load(sys.stdin)['data'];print($1)" 2>/dev/null; }

echo "== 0. 登录（或注册） =="
if [ -n "$EMAIL" ] && [ -n "$PASSWORD" ]; then
  LOGIN=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")
  check "登录成功" '"success":true' "$LOGIN"
else
  EMAIL="krel-e2e-$(date +%s)@test.local"
  REG=$(post "/auth/register" "{\"email\":\"$EMAIL\",\"password\":\"secret123\"}")
  check "注册成功（需 SMTP 配置）" '"success":true' "$REG"
  LOGIN=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"secret123\"}")
fi
TOKEN=$(echo "$LOGIN" | python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["token"])')
check "登录拿到 token" '^ey' "$TOKEN"
AUTH="Authorization: Bearer $TOKEN"

echo "== 1. 项目 + Secret Key =="
PROJ=$(post "/projects" '{"name":"知识库关联 e2e","description":"knowledge doc relations e2e"}')
PID=$(echo "$PROJ" | jid)
check "项目创建" '"success":true' "$PROJ"
SK=$(post "/projects/$PID/secret-key/generate" '{}' | python3 -c 'import sys,json;d=json.load(sys.stdin)["data"];print(d.get("secretKey") or "")')
check "取得项目 Secret Key" "^sk_" "$SK"

export CHUNSUN_API_URL="$BASE"
export CHUNSUN_SECRET_KEY="$SK"

echo "== 2. API 建树：主文档 + 分册 + 三级文档 =="
M=$(post "/projects/$PID/knowledge/documents" '{"title":"应用架构设计","content":"总纲","loadStrategy":"eager"}')
MID=$(echo "$M" | jid)
check "主文档创建（根）" '"parentId":null' "$M"

V1=$(post "/projects/$PID/knowledge/documents" "{\"title\":\"账户与权限详细设计\",\"content\":\"v1 正文\",\"loadStrategy\":\"lazy\",\"parentId\":\"$MID\"}")
V1ID=$(echo "$V1" | jid)
check "分册 1 挂主文档" "\"parentId\": *\"$MID\"" "$V1"

V2=$(post "/projects/$PID/knowledge/documents" "{\"title\":\"费率与路由详细设计\",\"content\":\"v2 正文\",\"loadStrategy\":\"eager\",\"parentId\":\"$MID\"}")
V2ID=$(echo "$V2" | jid)
check "分册 2 挂主文档" "\"parentId\": *\"$MID\"" "$V2"

SUB=$(post "/projects/$PID/knowledge/documents" "{\"title\":\"三级子文档\",\"content\":\"sub 正文\",\"parentId\":\"$V1ID\"}")
SUBID=$(echo "$SUB" | jid)
check "三级文档挂分册" "\"parentId\": *\"$V1ID\"" "$SUB"

echo "== 3. 详情：parentId / breadcrumb / children =="
DETAIL=$(get "/projects/$PID/knowledge/documents/$V1ID")
check "详情回 parentId" "\"parentId\": *\"$MID\"" "$DETAIL"
BC=$(jval "d['breadcrumb'][0]['id']" <<<"$DETAIL")
check "面包屑指向主文档" "$MID" "$BC"
CH=$(jval "','.join(c['id'] for c in d['children'])" <<<"$DETAIL")
check "子文档含三级文档" "$SUBID" "$CH"
ROOT_DETAIL=$(get "/projects/$PID/knowledge/documents/$MID")
CH2=$(jval "','.join(sorted(c['id'] for c in d['children']))" <<<"$ROOT_DETAIL")
check "主文档详情含两个直接分册" "$V1ID,$V2ID" "$CH2"

echo "== 4. index：前序输出 + parentId/depth =="
IDX=$(get "/projects/$PID/knowledge/index")
ORDER=$(echo "$IDX" | python3 -c '
import sys, json
arr = json.load(sys.stdin)["data"]["index"]
keys = [i["key"] for i in arr]
depths = {i["key"]: i["depth"] for i in arr}
m, v1, sub, v2 = sys.argv[1:5]
ok = (keys.index(m) < keys.index(v1) < keys.index(sub) < keys.index(v2)
      and depths[m] == 0 and depths[v1] == 1 and depths[sub] == 2 and depths[v2] == 1)
print("PASS" if ok else f"FAIL keys={keys} depths={depths}")' "$MID" "$V1ID" "$SUBID" "$V2ID")
check "index 前序且 depth 正确" "PASS" "$ORDER"

echo "== 5. CLI index：树形文本 + --json =="
IDX_TXT=$("$CLI" knowledge index 2>&1); IDX_CODE=$?
check_exit_zero "CLI index 退出码 0" "$IDX_CODE" "$IDX_TXT"
check "树形连接符 └" "└" "$IDX_TXT"
check "主文档标注分册数" "主文档（2 分册）" "$IDX_TXT"
check "树形含 lazy 分册" "账户与权限详细设计" "$IDX_TXT"
IDX_JSON=$("$CLI" knowledge index --json 2>&1); IDXJ_CODE=$?
check_exit_zero "CLI index --json 退出码 0" "$IDXJ_CODE" "$IDX_JSON"
check "index --json 含 parentId" '"parentId"' "$IDX_JSON"

echo "== 6. CLI doc：面包屑 + 子文档 + --json =="
DOC_TXT=$("$CLI" knowledge doc "$V1ID" 2>&1); DOC_CODE=$?
check_exit_zero "CLI doc 退出码 0" "$DOC_CODE" "$DOC_TXT"
check "doc 面包屑" "所属主文档：应用架构设计" "$DOC_TXT"
check "doc 子文档清单" "子文档（1）" "$DOC_TXT"
check "doc 正文" "v1 正文" "$DOC_TXT"
DOC_JSON=$("$CLI" knowledge doc "$V1ID" --json 2>&1); DOCJ_CODE=$?
check_exit_zero "CLI doc --json 退出码 0" "$DOCJ_CODE" "$DOC_JSON"
check "doc --json 含 breadcrumb" '"breadcrumb"' "$DOC_JSON"

echo "== 7. CLI create/update --parent =="
CLI_C=$("$CLI" knowledge create --title "CLI 分册" --content "cli child" --parent "$MID" --json 2>&1)
check "CLI create --parent 回读" "\"parentId\": \"$MID\"" "$CLI_C"
CID=$(echo "$CLI_C" | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')
CLI_U=$("$CLI" knowledge update "$CID" --parent "" --json 2>&1)
check "CLI update --parent 空串解除" '"parentId": null' "$CLI_U"
CLI_U2=$("$CLI" knowledge update "$CID" --parent "$MID" --json 2>&1)
check "CLI update --parent 重挂" "\"parentId\": \"$MID\"" "$CLI_U2"

echo "== 8. 守卫：自引用 / 成环 / 父不存在 / 跨项目 =="
SELF=$(put "/projects/$PID/knowledge/documents/$MID" "{\"parentId\":\"$MID\"}")
check "自引用 → PARENT_CYCLE" 'PARENT_CYCLE' "$SELF"
CYCLE=$(put "/projects/$PID/knowledge/documents/$MID" "{\"parentId\":\"$SUBID\"}")
check "成环 → PARENT_CYCLE" 'PARENT_CYCLE' "$CYCLE"
BADP=$(put "/projects/$PID/knowledge/documents/$MID" '{"parentId":"no-such-doc"}')
check "父不存在 → INVALID_PARENT_DOC" 'INVALID_PARENT_DOC' "$BADP"
check "系统键不可作父" 'INVALID_PARENT_DOC' "$(put "/projects/$PID/knowledge/documents/$MID" '{"parentId":"constitution"}')"

PROJ2=$(post "/projects" '{"name":"知识库关联 e2e 二","description":"cross project"}')
PID2=$(echo "$PROJ2" | jid)
OTHER=$(post "/projects/$PID2/knowledge/documents" '{"title":"别的项目的文档"}')
OID=$(echo "$OTHER" | jid)
CROSS=$(put "/projects/$PID/knowledge/documents/$MID" "{\"parentId\":\"$OID\"}")
check "跨项目父 → INVALID_PARENT_DOC" 'INVALID_PARENT_DOC' "$CROSS"

STILL=$(get "/projects/$PID/knowledge/documents/$MID")
check "守卫失败不落库（仍是根）" '"parentId":null' "$STILL"

echo "== 9. 解除/重挂 + 兼容：旧文档平铺、strategy 过滤保 depth =="
DETACH=$(put "/projects/$PID/knowledge/documents/$V2ID" '{"parentId":null}')
check "显式 null 解除关联" '"parentId":null' "$DETACH"
REATTACH=$(put "/projects/$PID/knowledge/documents/$V2ID" "{\"parentId\":\"$MID\"}")
check "重挂回主文档" "\"parentId\": *\"$MID\"" "$REATTACH"

LEGACY=$(post "/projects/$PID/knowledge/documents" '{"title":"独立旧式文档","content":"no parent"}')
LID=$(echo "$LEGACY" | jid)
IDX2=$(get "/projects/$PID/knowledge/index")
LROW=$(echo "$IDX2" | python3 -c '
import sys, json
arr = json.load(sys.stdin)["data"]["index"]
row = next((i for i in arr if i["key"] == sys.argv[1]), None)
print("PASS" if row and row["parentId"] is None and row["depth"] == 0 else f"FAIL {row}")' "$LID")
check "无父文档 depth=0 平铺" "PASS" "$LROW"

LAZY=$(get "/projects/$PID/knowledge/documents?strategy=lazy")
check "lazy 过滤含 lazy 分册" '账户与权限详细设计' "$LAZY"
check_absent "lazy 过滤不含 eager 主文档" '应用架构设计' "$LAZY"
LAZY_DEPTH=$(echo "$LAZY" | python3 -c '
import sys, json
arr = json.load(sys.stdin)["data"]["contexts"]
row = next((i for i in arr if i["key"] == sys.argv[1]), None)
print("PASS" if row and row["depth"] == 1 and row["parentId"] == sys.argv[2] else f"FAIL {row}")' "$V1ID" "$MID")
check "lazy 过滤保留绝对 depth/parentId" "PASS" "$LAZY_DEPTH"

echo "== 10. 删除：默认拒绝 + 显式级联（含批注宿主） =="
ANN=$(post "/projects/$PID/knowledge/documents/$V1ID/annotations" '{"body":"级联删除前置批注"}')
check "级联前置：分册可批注" '"success":true' "$ANN"

DEL1=$(del "/projects/$PID/knowledge/documents/$MID")
check "有子文档默认拒绝" 'DOC_HAS_CHILDREN' "$DEL1"
DEL2=$(del "/projects/$PID/knowledge/documents/$MID?withChildren=true")
check "withChildren 级联成功" '"success":true' "$DEL2"
check "子文档已删" 'CONTEXT_DOC_NOT_FOUND' "$(get "/projects/$PID/knowledge/documents/$V1ID")"
check "三级文档已删" 'CONTEXT_DOC_NOT_FOUND' "$(get "/projects/$PID/knowledge/documents/$SUBID")"
check "CLI 创建的分册已删" 'CONTEXT_DOC_NOT_FOUND' "$(get "/projects/$PID/knowledge/documents/$CID")"
IDX_AFTER=$(get "/projects/$PID/knowledge/index")
check_absent "index 不再含被删主文档" '应用架构设计' "$IDX_AFTER"
check "独立旧式文档仍在" '独立旧式文档' "$IDX_AFTER"

echo
echo "======== 结果: PASS=$PASS FAIL=$FAIL ========"
[ "$FAIL" -eq 0 ]
