#!/usr/bin/env bash
# e2e: SMTP 可用时，对真实未验证用户重发验证邮件，email_log 应为 sent。
# 可选环境变量 CHUNSUN_E2E_EMAIL；未设则取生产库中最新未验证用户。
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

if [[ -n "${CHUNSUN_E2E_EMAIL:-}" ]]; then
  TEST_EMAIL="$CHUNSUN_E2E_EMAIL"
else
  TEST_EMAIL=$(ssh -i "$KEY" -o BatchMode=yes "ubuntu@${HOST}" \
    "psql \"$URL\" -tAc \"SELECT email FROM \\\"user\\\" WHERE email_verified = false ORDER BY created_at DESC LIMIT 1\"" | tr -d '[:space:]')
fi

if [[ -z "$TEST_EMAIL" ]]; then
  echo "FAIL: no unverified user and CHUNSUN_E2E_EMAIL unset" >&2
  exit 1
fi

echo "resend-verification → $TEST_EMAIL"
RESP=$(curl -sS -w "\n%{http_code}" -X POST "$ORIGIN/api/v1/auth/resend-verification" \
  -H 'Content-Type: application/json' \
  -d "{\"email\":\"$TEST_EMAIL\"}")
BODY=$(echo "$RESP" | sed '$d')
CODE=$(echo "$RESP" | tail -n1)

echo "HTTP $CODE"
echo "$BODY"
test "$CODE" = "200"

# 等日志落库
sleep 1
STATUS=$(ssh -i "$KEY" -o BatchMode=yes "ubuntu@${HOST}" \
  "psql \"$URL\" -tAc \"SELECT status FROM email_log WHERE \\\"to\\\" = '$TEST_EMAIL' AND template = 'verification' ORDER BY created_at DESC LIMIT 1\"" | tr -d '[:space:]')

echo "email_log.status=$STATUS"
test "$STATUS" = "sent"

echo "OK: verification email_log=sent"
