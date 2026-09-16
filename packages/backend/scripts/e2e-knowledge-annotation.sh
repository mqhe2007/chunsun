#!/usr/bin/env bash
# 知识库文档批注端到端验收（需求 u-WPvdvYh4Fw，三步一体：文档级 / 内联 / LLM 结合）。
#
# 覆盖：
#   1. 成员读写路径：锚点批注 建 → 列 → 改 body → 删
#   2. 非作者改 / 删被拒（403 ANNOTATION_FORBIDDEN）
#   3. 宪法 / 项目记忆可批注（docRef=constitution|memory），单篇 GET 携带批注块
#   4. 锚点漂移：正文改写后 GET 列表置 stale，批注保留（绝不静默删除）
#   5. 方案 A 注入：eager 列表条目带 open 批注摘要；结案后不再注入
#   6. 选项 B 闭环：非作者（AI 角色）可结案 resolved；作者可重新打开回 open
#   7. 文档级隔离：A 文档批注不出现在 B 文档的列表 / 注入里
#
# 账号两种供给方式：
#   a. 全新注册（默认，适合 SMTP 可用的环境）：注册两个新账号自建项目；
#   b. 预置账号：CHUNSUN_EMAIL / CHUNSUN_PASSWORD（作者）+
#      CHUNSUN_EMAIL2（协作者，密码复用 CHUNSUN_PASSWORD）——适合本地无 SMTP、
#      注册被邀请码/邮箱验证挡住的环境（作者/协作者行需已存在且邮箱已验证）。
#
# 前置：本地后端（默认 127.0.0.1:11112）。JSON 解析用 node（Windows 无 python3）。

set -u

BASE="${CHUNSUN_BASE:-http://127.0.0.1:11112/api/v1}"
EMAIL="${CHUNSUN_EMAIL:-}"
PASSWORD="${CHUNSUN_PASSWORD:-}"
EMAIL2="${CHUNSUN_EMAIL2:-}"
JSON="Content-Type: application/json"

PASS=0
FAIL=0

# Windows / Git Bash 下，命令行参数会按本地代码页（GBK）转码后交给原生 curl.exe，
# UTF-8 中文 body 直接 -d 会变成非法字节。统一走临时文件（--data-binary @file）绕开，
# 文件内容不经过参数转码。用当前目录相对路径：原生 exe 解析不了 MSYS 的 /tmp。
BODY_FILE=".e2e-body.tmp"
trap 'rm -f "$BODY_FILE"' EXIT
wbody() { printf '%s' "$1" > "$BODY_FILE"; }

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

post() { wbody "$2"; curl -s -X POST "$BASE$1" ${AUTH:+-H "$AUTH"} -H "$JSON" --data-binary @"$BODY_FILE"; }
get()  { curl -s "$BASE$1" ${AUTH:+-H "$AUTH"}; }
put()  { wbody "$2"; curl -s -X PUT "$BASE$1" ${AUTH:+-H "$AUTH"} -H "$JSON" --data-binary @"$BODY_FILE"; }
patch(){ wbody "$2"; curl -s -X PATCH "$BASE$1" ${AUTH:+-H "$AUTH"} -H "$JSON" --data-binary @"$BODY_FILE"; }
del()  { curl -s -X DELETE "$BASE$1" ${AUTH:+-H "$AUTH"}; }
# 用法：jpath data.id / jpath 'data.contexts'（输出原始 JSON 片段）
jpath() { node -e "const d=JSON.parse(require('fs').readFileSync(0,'utf8'));let v=d;for(const k of '$1'.split('.')){v=v?.[k]}process.stdout.write(typeof v==='string'?v:JSON.stringify(v)??'')"; }
jlogin() { node -e "const d=JSON.parse(require('fs').readFileSync(0,'utf8'));process.stdout.write(d.data.token)"; }

# ================================================================ 0. 两个账号

echo "== 0. 作者 + 协作者 =="
if [ -n "$EMAIL" ] && [ -n "$PASSWORD" ]; then
  LOGIN1=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")
else
  TS=$(date +%s)
  EMAIL="ann-e2e-a-$TS@test.local"
  EMAIL2="ann-e2e-b-$TS@test.local"
  PASSWORD="secret123"
  post "/auth/register" "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}" >/dev/null
  post "/auth/register" "{\"email\":\"$EMAIL2\",\"password\":\"$PASSWORD\"}" >/dev/null
  LOGIN1=$(post "/auth/login" "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")
fi
check "作者登录" '"success":true' "$LOGIN1"
TOKEN1=$(echo "$LOGIN1" | jlogin)
AUTH="Authorization: Bearer $TOKEN1"
LOGIN2=$(curl -s -X POST "$BASE/auth/login" -H "$JSON" -d "{\"email\":\"${EMAIL2:-ann-e2e-b-0@test.local}\",\"password\":\"$PASSWORD\"}")
check "协作者登录" '"success":true' "$LOGIN2"
TOKEN2=$(echo "$LOGIN2" | jlogin)
AUTH2="Authorization: Bearer $TOKEN2"

# ================================================================ 1. 项目 + 两篇文档

echo "== 1. 项目 + 文档 =="
PROJ=$(post "/projects" '{"name":"知识批注 e2e","description":"annotation e2e"}')
PID=$(echo "$PROJ" | jpath data.id)
check "项目创建" '"success":true' "$PROJ"

DOC_A=$(post "/projects/$PID/knowledge/documents" '{"title":"部署手册","content":"## 部署步骤\n\n我们约定使用 **nanoid** 生成主键。\n\n第一行内容\n第二行内容","loadStrategy":"eager"}')
DOC_A_ID=$(echo "$DOC_A" | jpath data.id)
check "文档 A 创建" '"success":true' "$DOC_A"

DOC_B=$(post "/projects/$PID/knowledge/documents" '{"title":"另一篇","content":"文档 B 的原始正文","loadStrategy":"lazy"}')
DOC_B_ID=$(echo "$DOC_B" | jpath data.id)
check "文档 B 创建" '"success":true' "$DOC_B"

# 拉协作者进项目（真实成员路径，非特权直插）
INVITE=$(post "/projects/$PID/members" "{\"identifier\":\"$EMAIL2\",\"role\":\"MEMBER\"}")
check "协作者加入项目" '"success":true' "$INVITE"

# ================================================================ 2. 锚点批注 CRUD

echo "== 2. 建 → 列 → 改 → 删 =="
CREATE=$(post "/projects/$PID/knowledge/documents/$DOC_A_ID/annotations" \
  '{"body":"这里应该写清 nanoid 长度","anchorText":"使用 nanoid 生成主键","anchorPrefix":"我们约定","anchorSuffix":"。"}')
check "锚点批注创建" '"success":true' "$CREATE"
check "返回 open 状态" '"status":"open"' "$CREATE"
check "回显锚点文本" '使用 nanoid 生成主键' "$CREATE"
AN_A=$(echo "$CREATE" | jpath data.id)

# 整篇批注（无锚点）
CREATE_WHOLE=$(post "/projects/$PID/knowledge/documents/$DOC_A_ID/annotations" '{"body":"整篇缺一节错误处理示例"}')
check "整篇批注创建" '"success":true' "$CREATE_WHOLE"
AN_D=$(echo "$CREATE_WHOLE" | jpath data.id)

LIST=$(get "/projects/$PID/knowledge/documents/$DOC_A_ID/annotations")
check "列表含锚点批注" "$AN_A" "$LIST"
check "列表含整篇批注" "$AN_D" "$LIST"

PATCHED=$(patch "/projects/$PID/knowledge/annotations/$AN_A" '{"body":"已补充：nanoid(12)"}')
check "作者改 body" '已补充：nanoid(12)' "$PATCHED"

# 空锚点等价整篇：anchorText 空白串应被归一成 NULL（返回 anchorText:null）
CREATE_EMPTY_ANCHOR=$(post "/projects/$PID/knowledge/documents/$DOC_A_ID/annotations" '{"body":"空锚点归一检查","anchorText":"  "}')
check "空锚点归一为整篇" '"anchorText":null' "$CREATE_EMPTY_ANCHOR"
del "/projects/$PID/knowledge/annotations/$(echo "$CREATE_EMPTY_ANCHOR" | jpath data.id)" >/dev/null

DELETED=$(del "/projects/$PID/knowledge/annotations/$AN_D")
check "作者删除批注" '"success":true' "$DELETED"
LIST2=$(get "/projects/$PID/knowledge/documents/$DOC_A_ID/annotations")
check_absent "删除后列表不含已删项" "$AN_D" "$LIST2"

# ================================================================ 3. 非作者改 / 删被拒

echo "== 3. 非作者 403 =="
PATCH3=$(AUTH="$AUTH2"; patch "/projects/$PID/knowledge/annotations/$AN_A" '{"body":"协作者想改"}')
check "非作者改被拒" 'ANNOTATION_FORBIDDEN' "$PATCH3"
DEL3=$(AUTH="$AUTH2"; del "/projects/$PID/knowledge/annotations/$AN_A")
check "非作者删被拒" 'ANNOTATION_FORBIDDEN' "$DEL3"
# 项目成员身份不够，须到作者级：批注内容应保持作者编辑后的样子
STILL=$(get "/projects/$PID/knowledge/documents/$DOC_A_ID/annotations")
check "原批注未被改动" '已补充：nanoid(12)' "$STILL"

# ================================================================ 4. 系统文档批注 + 单篇 GET 携带

echo "== 4. 宪法 / 项目记忆批注 =="
CA=$(post "/projects/$PID/knowledge/documents/constitution/annotations" '{"body":"宪法第二条建议补示例"}')
check "宪法可批注" '"success":true' "$CA"
CA_ID=$(echo "$CA" | jpath data.id)
MA=$(post "/projects/$PID/knowledge/documents/memory/annotations" '{"body":"记忆快照建议压缩历史段"}')
check "项目记忆可批注" '"success":true' "$MA"

CON_GET=$(get "/projects/$PID/knowledge/constitution")
check "宪法 GET 携带批注块" '"annotations"' "$CON_GET"
check "宪法批注块含正文" '宪法第二条建议补示例' "$CON_GET"

# 新项目默认没有记忆行（GET /memory 404 是既有语义）；先写一份快照再读
put "/projects/$PID/memory" '{"snapshot":"## 项目记忆\n\n历史经验占位。"}' >/dev/null
MEM_GET=$(get "/projects/$PID/memory")
check "项目记忆 GET 携带批注块" '"annotations"' "$MEM_GET"

# ================================================================ 5. 锚点漂移 → stale 不删除

echo "== 5. 锚点漂移 =="
CREATE_B=$(post "/projects/$PID/knowledge/documents/$DOC_B_ID/annotations" \
  '{"body":"这句要改写","anchorText":"文档 B 的原始正文"}')
AN_B=$(echo "$CREATE_B" | jpath data.id)
check "B 批注创建" '"success":true' "$CREATE_B"

# 正文整段重写 → 锚点再也找不到
put "/projects/$PID/knowledge/documents/$DOC_B_ID" \
  '{"title":"另一篇","content":"文档 B 已被整段重写","loadStrategy":"lazy"}' >/dev/null

STALE_LIST=$(get "/projects/$PID/knowledge/documents/$DOC_B_ID/annotations")
check "列表报告漂移检测" '"staleDetected"' "$STALE_LIST"
check "漂移批注 id 在列" "$AN_B" "$STALE_LIST"
check "漂移批注置 stale" '"status":"stale"' "$STALE_LIST"
check "批注保留未删除" '"body":"这句要改写"' "$STALE_LIST"

# ================================================================ 6. 方案 A 注入（eager 摘要 + 单篇全量）

echo "== 6. eager 注入 =="
EAGER=$(get "/projects/$PID/knowledge?strategy=eager")
ITEM_A=$(echo "$EAGER" | node -e "const d=JSON.parse(require('fs').readFileSync(0,'utf8'));const c=d.data.contexts.find(x=>x.key==='$DOC_A_ID');process.stdout.write(JSON.stringify(c))")
check "eager 条目带批注摘要" '"annotations"' "$ITEM_A"
check "摘要含 open 批注正文" '已补充：nanoid(12)' "$ITEM_A"
# resolved / stale 不进 prompt：B 文档的 stale 批注不应出现在注入里
ITEM_B=$(echo "$EAGER" | node -e "const d=JSON.parse(require('fs').readFileSync(0,'utf8'));const c=d.data.contexts.find(x=>x.key==='$DOC_B_ID');process.stdout.write(JSON.stringify(c))")
check_absent "stale 批注不进 eager 注入" '这句要改写' "$ITEM_B"
check_absent "B 无批注时无 annotations 键" '"annotations"' "$ITEM_B"

DOC_GET=$(get "/projects/$PID/knowledge/documents/$DOC_A_ID")
check "单篇 GET 携带批注块" '"annotations"' "$DOC_GET"
check "单篇形态含锚点上下文" '"anchorPrefix"' "$DOC_GET"

# ================================================================ 7. 选项 B 闭环（AI 结案 → 人重开）

echo "== 7. 结案闭环 =="
# 非作者（模拟 AI / 其他成员）结案：所有成员可用，不要求作者
RESOLVE=$(AUTH="$AUTH2"; patch "/projects/$PID/knowledge/annotations/$AN_A" \
  '{"status":"resolved","outcome":"addressed","resolvedNote":"已在部署步骤补充 nanoid(12) 说明"}')
check "非作者可结案" '"status":"resolved"' "$RESOLVE"
check "结案结论 addressed" '"outcome":"addressed"' "$RESOLVE"
check "结案留痕 note" '已在部署步骤补充' "$RESOLVE"

# 结案后不再注入（resolved 不进 prompt）
EAGER2=$(get "/projects/$PID/knowledge?strategy=eager")
ITEM_A2=$(echo "$EAGER2" | node -e "const d=JSON.parse(require('fs').readFileSync(0,'utf8'));const c=d.data.contexts.find(x=>x.key==='$DOC_A_ID');process.stdout.write(JSON.stringify(c))")
check_absent "结案后 eager 不再注入" '已补充：nanoid(12)' "$ITEM_A2"
# 但阅读页列表仍可见（resolved 折叠但不消失，选项 B 的可核查兜底）
FULL_LIST=$(get "/projects/$PID/knowledge/documents/$DOC_A_ID/annotations")
check "结案后列表仍可见" '"status":"resolved"' "$FULL_LIST"

# 作者重新打开 → 清空结案痕迹
REOPEN=$(patch "/projects/$PID/knowledge/annotations/$AN_A" '{"status":"open"}')
check "作者可重新打开" '"status":"open"' "$REOPEN"
check "重开清空 outcome" '"outcome":null' "$REOPEN"
check "重开清空 note" '"resolvedNote":null' "$REOPEN"

# ================================================================ 8. 文档级隔离

echo "== 8. 隔离 =="
ISOL_LIST=$(get "/projects/$PID/knowledge/documents/$DOC_B_ID/annotations")
check_absent "A 的批注不出现在 B" "$AN_A" "$ISOL_LIST"
check_absent "宪法批注不出现在 B" "$CA_ID" "$ISOL_LIST"
EAGER3=$(get "/projects/$PID/knowledge?strategy=eager")
ITEM_B3=$(echo "$EAGER3" | node -e "const d=JSON.parse(require('fs').readFileSync(0,'utf8'));const c=d.data.contexts.find(x=>x.key==='$DOC_B_ID');process.stdout.write(JSON.stringify(c))")
check_absent "B 条目不含 A 的批注" '已补充：nanoid(12)' "$ITEM_B3"

echo "== 结果：通过 $PASS / 失败 $FAIL =="
[ "$FAIL" -eq 0 ]
