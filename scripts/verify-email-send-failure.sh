#!/usr/bin/env bash
# e2e: 临时清空 smtpHost 后注册，必须返回 EMAIL_SEND_FAILED 且用户不落库，再恢复配置。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
# shellcheck disable=SC1091
set -a; source .env; set +a

ORIGIN="${PUBLIC_ORIGIN:?PUBLIC_ORIGIN required}"
KEY="${PROD_DEPLOY_CREDENTIAL:?PROD_DEPLOY_CREDENTIAL required}"
HOST="${PROD_DEPLOY_HOST:?PROD_DEPLOY_HOST required}"

psql_url() {
  ssh -i "$KEY" -o BatchMode=yes "ubuntu@${HOST}" sudo python3 - <<'PY'
import json
from urllib.parse import urlparse, urlunparse, parse_qsl, urlencode
u = json.load(open("/var/www/chunsun/bin/chunsun.json"))["databaseUrl"]
p = urlparse(u)
q = [(k, v) for k, v in parse_qsl(p.query) if k.lower() != "schema"]
print(urlunparse(p._replace(query=urlencode(q))))
PY
}

URL=$(psql_url)
TEST_EMAIL="chunsun-fail-ux-$(date +%s)@example.invalid"
ORIG_HOST=$(ssh -i "$KEY" -o BatchMode=yes "ubuntu@${HOST}" \
  "psql \"$URL\" -tAc \"SELECT value FROM system_setting WHERE key='email.smtpHost'\"" | tr -d '[:space:]')

cleanup() {
  ssh -i "$KEY" -o BatchMode=yes "ubuntu@${HOST}" \
    "psql \"$URL\" -c \"UPDATE system_setting SET value = '${ORIG_HOST}' WHERE key = 'email.smtpHost'\"" >/dev/null
}
trap cleanup EXIT

ssh -i "$KEY" -o BatchMode=yes "ubuntu@${HOST}" \
  "psql \"$URL\" -c \"UPDATE system_setting SET value = '' WHERE key = 'email.smtpHost'\"" >/dev/null

RESP=$(curl -sS -w "\n%{http_code}" -X POST "$ORIGIN/api/v1/auth/register" \
  -H 'Content-Type: application/json' \
  -d "{\"email\":\"$TEST_EMAIL\",\"password\":\"TestPass123!\",\"nickname\":\"fail-ux\"}")
BODY=$(echo "$RESP" | sed '$d')
CODE=$(echo "$RESP" | tail -n1)

echo "HTTP $CODE"
echo "$BODY"
echo "$BODY" | grep -q '"error":"EMAIL_SEND_FAILED"'
test "$CODE" = "400"

ROWS=$(ssh -i "$KEY" -o BatchMode=yes "ubuntu@${HOST}" \
  "psql \"$URL\" -tAc \"SELECT count(*) FROM \\\"user\\\" WHERE email = '$TEST_EMAIL'\"" | tr -d '[:space:]')
test "$ROWS" = "0"

echo "OK: EMAIL_SEND_FAILED + no user row"
