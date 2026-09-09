#!/usr/bin/env python3
"""将 requirement_memory.snapshot 从 JSON 五字段转为 Markdown 文本。

用法:
  python3 migrate_memory_to_markdown.py <database_url>

先执行 schema migration（JSONB→TEXT），再运行本脚本。
"""
import json
import sys
import subprocess


def json_to_markdown(snapshot_json: str) -> str:
    """把 JSON snapshot 转为 Markdown 文本。"""
    try:
        data = json.loads(snapshot_json)
    except (json.JSONDecodeError, TypeError):
        # 已经是 Markdown 或无法解析，原样返回
        return snapshot_json if isinstance(snapshot_json, str) else ""

    if not isinstance(data, dict):
        return str(data)

    sections = []

    # 需求边界
    rs = data.get("requirementSnapshot")
    if rs:
        sections.append("## 需求边界")
        if isinstance(rs, dict):
            for k, v in rs.items():
                if isinstance(v, (dict, list)):
                    sections.append(f"- **{k}**: {json.dumps(v, ensure_ascii=False)}")
                else:
                    sections.append(f"- **{k}**: {v}")
        elif isinstance(rs, list):
            for item in rs:
                sections.append(f"- {item}")
        else:
            sections.append(str(rs))
        sections.append("")

    # 待决策
    od = data.get("openDecisions")
    if od and isinstance(od, list) and len(od) > 0:
        sections.append("## 待决策")
        for d in od:
            if isinstance(d, dict):
                text = (
                    d.get("question")
                    or d.get("text")
                    or d.get("content")
                    or d.get("decision")
                    or d.get("title")
                    or d.get("summary")
                    or str(d)
                )
                sections.append(f"- [ ] {text}")
            else:
                sections.append(f"- [ ] {d}")
        sections.append("")

    # 代码标记
    cl = data.get("codeLandmarks")
    if cl and isinstance(cl, list) and len(cl) > 0:
        sections.append("## 代码标记")
        for l in cl:
            if isinstance(l, dict):
                path = l.get("path") or l.get("file") or l.get("location") or ""
                symbol = l.get("symbol") or l.get("func") or l.get("name") or ""
                note = l.get("note") or l.get("description") or l.get("comment") or ""
                loc = f"`{path}"
                if symbol:
                    loc += f":{symbol}"
                loc += "`"
                if note:
                    sections.append(f"- {loc} — {note}")
                else:
                    sections.append(f"- {loc}")
            else:
                sections.append(f"- {l}")
        sections.append("")

    # 环境变量
    er = data.get("envRefs")
    if er and isinstance(er, list) and len(er) > 0:
        sections.append("## 环境变量")
        for e in er:
            if isinstance(e, dict):
                key = (
                    e.get("key")
                    or e.get("name")
                    or e.get("envKey")
                    or e.get("variable")
                    or str(e)
                )
                sections.append(f"- `{key}`")
            else:
                sections.append(f"- `{e}`")
        sections.append("")

    # 阻塞原因
    ds = data.get("dependencySnapshot")
    if ds:
        sections.append("## 阻塞原因")
        if isinstance(ds, dict):
            for k, v in ds.items():
                if isinstance(v, (dict, list)):
                    sections.append(f"- **{k}**: {json.dumps(v, ensure_ascii=False)}")
                else:
                    sections.append(f"- **{k}**: {v}")
        elif isinstance(ds, list):
            for item in ds:
                sections.append(f"- {item}")
        else:
            sections.append(str(ds))
        sections.append("")

    # 本轮总结
    ls = data.get("lastRunSummary")
    if ls:
        sections.append("## 本轮总结")
        if isinstance(ls, dict):
            what = ls.get("what") or ls.get("summary") or ls.get("done")
            decisions = ls.get("decisions")
            result = ls.get("result")
            nxt = ls.get("next") or ls.get("nextStep") or ls.get("todo")
            if what:
                sections.append("### 做了什么")
                sections.append(str(what))
                sections.append("")
            if decisions:
                sections.append("### 关键决策与理由")
                if isinstance(decisions, list):
                    for d in decisions:
                        sections.append(f"- {d}")
                else:
                    sections.append(str(decisions))
                sections.append("")
            if result:
                sections.append("### 结果")
                sections.append(str(result))
                sections.append("")
            if nxt:
                sections.append("### 下一步建议")
                if isinstance(nxt, list):
                    for n in nxt:
                        sections.append(f"- {n}")
                else:
                    sections.append(str(nxt))
                sections.append("")
            # 其他未识别的 key
            known = {"what", "summary", "done", "decisions", "result", "next", "nextStep", "todo"}
            for k, v in ls.items():
                if k not in known and v:
                    if isinstance(v, (dict, list)):
                        sections.append(f"### {k}")
                        sections.append(f"```json\n{json.dumps(v, ensure_ascii=False, indent=2)}\n```")
                    else:
                        sections.append(f"### {k}")
                        sections.append(str(v))
                    sections.append("")
        elif isinstance(ls, list):
            for item in ls:
                sections.append(f"- {item}")
            sections.append("")
        else:
            sections.append(str(ls))
            sections.append("")

    # 其他未知字段
    known_keys = {
        "requirementSnapshot", "openDecisions", "codeLandmarks",
        "envRefs", "dependencySnapshot", "lastRunSummary",
    }
    extra = {k: v for k, v in data.items() if k not in known_keys and v}
    if extra:
        sections.append("## 其他")
        for k, v in extra.items():
            if isinstance(v, (dict, list)):
                sections.append(f"### {k}")
                sections.append(f"```json\n{json.dumps(v, ensure_ascii=False, indent=2)}\n```")
            else:
                sections.append(f"- **{k}**: {v}")
        sections.append("")

    return "\n".join(sections).strip() + "\n"


def run_psql(db_url: str, sql: str) -> str:
    """执行 psql 命令并返回输出。"""
    result = subprocess.run(
        ["psql", db_url, "-t", "-A", "-c", sql],
        capture_output=True, text=True, check=True,
    )
    return result.stdout.strip()


def main():
    if len(sys.argv) < 2:
        print("用法: python3 migrate_memory_to_markdown.py <database_url>", file=sys.stderr)
        sys.exit(1)

    db_url = sys.argv[1]

    # 导出所有记录
    print("[migrate] 导出 requirement_memory 记录...")
    rows = run_psql(db_url, "SELECT id, snapshot FROM requirement_memory WHERE snapshot IS NOT NULL AND snapshot != 'null' AND snapshot != '{}'")

    if not rows:
        print("[migrate] 无需要迁移的记录。")
        return

    records = []
    for line in rows.split("\n"):
        if not line.strip():
            continue
        # psql -t -A 用 | 分隔列，但 snapshot 可能包含 |，所以只分割第一个 |
        parts = line.split("|", 1)
        if len(parts) != 2:
            print(f"[migrate] 跳过无法解析的行: {line[:80]}", file=sys.stderr)
            continue
        mem_id, snapshot = parts
        records.append((mem_id, snapshot))

    print(f"[migrate] 共 {len(records)} 条记录需要迁移。")

    migrated = 0
    skipped = 0
    for mem_id, snapshot in records:
        # 检查是否已经是 Markdown（不是 JSON 对象）
        stripped = snapshot.strip()
        if not stripped.startswith("{"):
            print(f"[migrate] 跳过 {mem_id}: 非 JSON（可能已是 Markdown）")
            skipped += 1
            continue

        markdown = json_to_markdown(stripped)
        if not markdown.strip():
            print(f"[migrate] 跳过 {mem_id}: 转换后为空")
            skipped += 1
            continue

        # 用 psql 更新，通过 stdin 传递参数避免转义问题
        update_sql = f"UPDATE requirement_memory SET snapshot = $snapshot${markdown}$snapshot$ WHERE id = '{mem_id}'"
        try:
            run_psql(db_url, update_sql)
            migrated += 1
            print(f"[migrate] 已迁移 {mem_id} ({len(markdown)} 字符)")
        except subprocess.CalledProcessError as e:
            print(f"[migrate] 迁移失败 {mem_id}: {e.stderr}", file=sys.stderr)
            skipped += 1

    print(f"\n[migrate] 完成: 迁移 {migrated} 条, 跳过 {skipped} 条")


if __name__ == "__main__":
    main()
