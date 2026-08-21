#!/usr/bin/env bash
# 验收 GET /api/v1/harness/template：合法密钥 200 + 版本一致；无/无效密钥 401。
# 用法（仓库根）：
#   CHUNSUN_SECRET_KEY=sk_... [CHUNSUN_API_URL=https://host/api/v1] \
#     bash packages/backend/scripts/verify-harness-template.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

API="${CHUNSUN_API_URL:-https://chunsun.example.com/api/v1}"
API="${API%/}"
SK="${CHUNSUN_SECRET_KEY:-}"
EXPECTED_VERSION="$(tr -d '[:space:]' < packages/backend/templates/VERSION)"

if [[ -z "$SK" ]]; then
  echo "[verify] 缺少 CHUNSUN_SECRET_KEY" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

echo "[verify] API=$API expectedVersion=$EXPECTED_VERSION"

# --- 401: 无 Authorization ---
code_no_auth="$(curl -sS -o "$tmpdir/no_auth.json" -w '%{http_code}' "$API/harness/template" || true)"
if [[ "$code_no_auth" != "401" ]]; then
  echo "[verify] FAIL 无密钥期望 401，实际 $code_no_auth" >&2
  cat "$tmpdir/no_auth.json" >&2 || true
  exit 1
fi
echo "[verify] OK 无密钥 → 401"

# --- 401: 无效密钥 ---
code_bad="$(curl -sS -o "$tmpdir/bad.json" -w '%{http_code}' \
  -H "Authorization: Bearer sk_invalid_for_verify_harness_template" \
  "$API/harness/template" || true)"
if [[ "$code_bad" != "401" ]]; then
  echo "[verify] FAIL 无效密钥期望 401，实际 $code_bad" >&2
  cat "$tmpdir/bad.json" >&2 || true
  exit 1
fi
echo "[verify] OK 无效密钥 → 401"

# --- 200: 合法密钥 ---
code_ok="$(curl -sS -o "$tmpdir/ok.json" -w '%{http_code}' \
  -H "Authorization: Bearer $SK" \
  "$API/harness/template")"
if [[ "$code_ok" != "200" ]]; then
  echo "[verify] FAIL 合法密钥期望 200，实际 $code_ok" >&2
  cat "$tmpdir/ok.json" >&2 || true
  exit 1
fi

python3 - "$tmpdir/ok.json" "$EXPECTED_VERSION" <<'PY'
import json, sys
path, expected = sys.argv[1], sys.argv[2]
with open(path) as f:
    body = json.load(f)
assert body.get("success") is True, body
data = body.get("data") or {}
version = data.get("templateVersion")
files = data.get("files") or {}
required = [
    "SKILL.md",
    "loop-rules.md",
    "commands.md",
    "slash/chunsun.md",
    "slash/chunsun-fix.md",
]
missing = [k for k in required if not isinstance(files.get(k), str) or not files[k].strip()]
if missing:
    raise SystemExit(f"missing/empty files: {missing}")
if version != expected:
    raise SystemExit(f"templateVersion={version!r} != expected={expected!r}")
print(f"[verify] OK 合法密钥 → 200 templateVersion={version} files={len(files)}")
PY

echo "[verify] 全部通过"
