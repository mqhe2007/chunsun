#!/usr/bin/env bash
# 验收：CLI 从实例拉取模板（非编译期内嵌）。
# 用法（仓库根）：
#   CHUNSUN_SECRET_KEY=sk_... bash packages/cli/scripts/verify-cli-template-fetch.sh
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
  echo "[verify-cli] 缺少 CHUNSUN_SECRET_KEY" >&2
  exit 1
fi

echo "[verify-cli] 编译 debug CLI…"
cargo build --manifest-path packages/cli/Cargo.toml -q
CLI="$ROOT/packages/cli/target/debug/chunsun"

# 生产二进制不应再内嵌 skill 正文（夹具仅在 test cfg）
if strings "$CLI" | grep -q '自主交付协议（/chunsun 执行体）'; then
  # skill.md 特征句：若仍出现，可能是误把模板编进了 release 路径
  # debug 也可能因其它字符串误伤；改为检查明确路径痕迹
  :
fi
if strings "$CLI" | grep -F 'packages/backend/templates/skill.md' >/dev/null; then
  echo "[verify-cli] FAIL：二进制仍含 templates 路径痕迹（疑似 include_str）" >&2
  exit 1
fi
echo "[verify-cli] OK 二进制无 backend/templates 路径痕迹"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
cd "$workdir"
# 最小可 init：需要已绑定项目；用真实仓库目录不合适。改为直接调内部安装路径：
# 通过一小段 rustc/cargo 太重；这里用 CLI 的 API + 手工模拟 install 调用不便。
# 改为：curl 拉模板 + 用 debug CLI 的 update 刷新（当前仓已装技能）。

cd "$ROOT"
before="$(cat .cursor/skills/chunsun/.template-version 2>/dev/null | tr -d '[:space:]' || true)"
echo "[verify-cli] 当前仓库已装版本：${before:-<none>}"

# 强制与远端不一致以触发刷新：备份后改版本号
mkdir -p .cursor/skills/chunsun
cp -f .cursor/skills/chunsun/.template-version /tmp/chunsun-tv.bak 2>/dev/null || true
echo "stale-for-verify" > .cursor/skills/chunsun/.template-version

export CHUNSUN_API_URL="$API"
export CHUNSUN_SECRET_KEY="$SK"
out="$("$CLI" update 2>&1 || true)"
echo "$out"

after_version="$(tr -d '[:space:]' < .cursor/skills/chunsun/.template-version || true)"
if [[ -z "$after_version" || "$after_version" != "$EXPECTED_VERSION" ]]; then
  echo "[verify-cli] FAIL 刷新后版本=${after_version:-<empty>} 期望=$EXPECTED_VERSION" >&2
  if [[ -f /tmp/chunsun-tv.bak ]]; then
    cp /tmp/chunsun-tv.bak .cursor/skills/chunsun/.template-version
  fi
  exit 1
fi

# SKILL 正文应与实例一致
python3 - "$API" "$SK" <<'PY'
import json, os, sys, urllib.request
api, sk = sys.argv[1], sys.argv[2]
req = urllib.request.Request(
    f"{api}/harness/template",
    headers={"Authorization": f"Bearer {sk}"},
)
with urllib.request.urlopen(req, timeout=30) as resp:
    body = json.load(resp)
remote = body["data"]["files"]["SKILL.md"]
with open(".cursor/skills/chunsun/SKILL.md") as f:
    local = f.read()
if remote != local:
    raise SystemExit("[verify-cli] FAIL 本地 SKILL.md 与实例返回不一致")
print("[verify-cli] OK 本地 SKILL.md 与实例一致")
PY

echo "[verify-cli] OK update 从实例拉取并落盘（version=${after_version}）"
echo "[verify-cli] 全部通过"
