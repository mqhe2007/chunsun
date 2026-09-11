#!/usr/bin/env bash
# 知识库分享 + 系统文档拒绝分享端到端验收（需求 FQj4N2yeSXNY）。
#
# 覆盖：
#   1. 自定义文档可 POST 生成分享链接；公开 GET 可读回 title/content
#   2. 停用后公开 GET → SHARE_INVALID
#   3. 轮换后旧 token 失效
#   4. constitution / memory 分享 → SHARE_SYSTEM_DOC_FORBIDDEN
#   5. 公开响应不含成员/环境变量等字段
#
# 前置：本地后端（默认 127.0.0.1:11112）+ 已激活账号
#   CHUNSUN_EMAIL=e2e-kcli@test.local CHUNSUN_PASSWORD=e2e-secret-123 \
#     ./scripts/e2e-knowledge-share.sh

set -u

BASE="${CHUNSUN_BASE:-http://127.0.0.1:11112/api/v1}"
EMAIL="${CHUNSUN_EMAIL:-}"
PASSWORD="${CHUNSUN_PASSWORD:-}"
JSON="Content-Type: application/json"

PASS=0
FAIL=0

check() {
  if echo "$3" | grep -q "$2"; then
    printf '  ✓ %s\n' "$1"; PASS=$((PASS + 1))
  else
    printf '  ✗ %s\n    期望含: %s\n    实际: %s\n' "$1" "$2" "$3"; FAIL=$((FAIL + 1))
  fi
}

check_absent() {
  if echo "$3" | grep -q "$2"; then
    printf '  ✗ %s\n    不应含: %s\n    实际: %s\n' "$1" "$2" "$3"; FAIL=$((FAIL + 1))
  else
    printf '  ✓ %s\n' "$1"; PASS=$((PASS + 1))
  fi
}

post() { curl -s -X POST "$BASE$1" ${AUTH:+-H "$AUTH"} -H "$JSON" -d "$2"; }
get()  { curl -s "$BASE$1" ${AUTH:+-H "$AUTH"}; }
patch(){ curl -s -X PATCH "$BASE$1" ${AUTH:+-H "$AUTH"} -H "$JSON" -d "$2"; }
del()  { curl -s -X DELETE "$BASE$1" ${AUTH:+-H "$AUTH"}; }
jid()  { python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["id"])'; }

echo "== 0. 登录 =="
if [ -n "$EMAIL" ] && [ -n "$PASSWORD" ]; then
  LOGIN=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")
else
  EMAIL="kshare-e2e-$(date +%s)@test.local"
  post "/auth/register" "{\"email\":\"$EMAIL\",\"password\":\"secret123\"}" >/dev/null
  LOGIN=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"secret123\"}")
fi
check "登录成功" '"success":true' "$LOGIN"
TOKEN=$(echo "$LOGIN" | python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["token"])')
AUTH="Authorization: Bearer $TOKEN"

echo "== 1. 项目 + 自定义文档 =="
PROJ=$(post "/projects" '{"name":"知识分享 e2e","description":"share e2e"}')
PID=$(echo "$PROJ" | jid)
check "项目创建" '"success":true' "$PROJ"
DOC=$(post "/projects/$PID/knowledge/documents" '{"title":"对外手册","content":"# 标题一\n\n正文\n\n## 小节\n\n细节","loadStrategy":"lazy"}')
DOC_ID=$(echo "$DOC" | jid)
check "文档创建" '"success":true' "$DOC"

echo "== 2. 生成分享 =="
SHARE=$(post "/projects/$PID/knowledge/documents/$DOC_ID/share" '{}')
check "分享创建成功" '"success":true' "$SHARE"
check "返回 token" '"token":' "$SHARE"
check "返回 url" '"url":' "$SHARE"
STOKEN=$(echo "$SHARE" | python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["token"])')

echo "== 3. 公开读取 =="
PUB=$(curl -s "$BASE/public/knowledge/shares/$STOKEN")
check "公开可读" '"success":true' "$PUB"
check "公开含标题" '对外手册' "$PUB"
check "公开含正文" '标题一' "$PUB"
check_absent "公开不含 envVars" 'envVar' "$PUB"
check_absent "公开不含 members" 'member' "$PUB"

echo "== 4. 停用 =="
DEL=$(del "/projects/$PID/knowledge/documents/$DOC_ID/share")
check "停用成功" '"success":true' "$DEL"
PUB2=$(curl -s "$BASE/public/knowledge/shares/$STOKEN")
check "停用后公开失败" 'SHARE_INVALID' "$PUB2"

echo "== 5. 重新生成并使旧 token 失效 =="
SHARE2=$(post "/projects/$PID/knowledge/documents/$DOC_ID/share" '{}')
STOKEN2=$(echo "$SHARE2" | python3 -c 'import sys,json;print(json.load(sys.stdin)["data"]["token"])')
PUB_OLD=$(curl -s "$BASE/public/knowledge/shares/$STOKEN")
PUB_NEW=$(curl -s "$BASE/public/knowledge/shares/$STOKEN2")
check "旧 token 失效" 'SHARE_INVALID' "$PUB_OLD"
check "新 token 可读" '"success":true' "$PUB_NEW"

echo "== 6. 系统文档拒绝分享 =="
SYS1=$(post "/projects/$PID/knowledge/documents/constitution/share" '{}')
SYS2=$(post "/projects/$PID/knowledge/documents/memory/share" '{}')
check "宪法不可分享" 'SHARE_SYSTEM_DOC_FORBIDDEN' "$SYS1"
check "记忆不可分享" 'SHARE_SYSTEM_DOC_FORBIDDEN' "$SYS2"

echo "== 结果：通过 $PASS / 失败 $FAIL =="
[ "$FAIL" -eq 0 ]
