/**
 * 乐观锁冲突（409 `*_CONFLICT`）的识别与拆解。
 *
 * 后端四个「全量覆盖」写入点（知识文档 / 宪法 / 项目记忆 / 需求记忆）在
 * `revision` 不匹配时统一返回 409，`data` 里带 `yourRevision`、`currentRevision`
 * 与**当前正文全文**，目的是让调用方就地合并重试——服务端不做自动合并、不做 diff。
 *
 * 抽成纯函数的理由与 CLI 侧 `ConflictInfo::parse` 相同：字段抽取（四种 data 形状、
 * `currentContent` 与 `currentSnapshot` 的择一）是最容易写错的部分，而它又只出现在
 * 错误分支里——写错了没人会立刻发现，得等下一次真冲突。渲染留在组件里，这里只出数据。
 */

/** 后端会以 409 报出的冲突错误码。 */
export const CONFLICT_ERROR_CODES = [
  "KNOWLEDGE_DOC_CONFLICT",
  "CONSTITUTION_CONFLICT",
  "MEMORY_CONFLICT",
] as const;

export type ConflictErrorCode = (typeof CONFLICT_ERROR_CODES)[number];

export type VersionConflict = {
  code: ConflictErrorCode;
  /** 我方提交时携带的版本号；报文没带则为 null。 */
  yourRevision: number | null;
  /** 服务端当前版本号；**合并重试时要用的是它**。 */
  currentRevision: number | null;
  /** 当前正文 / 记忆快照全文；报文没带则为 null。 */
  currentContent: string | null;
  /** 正文是记忆快照（`currentSnapshot`）还是知识文档正文（`currentContent`）。 */
  isSnapshot: boolean;
};

/** 从 axios 错误里取 `error` 码；非 axios 错误、无 `data` 都返回 null。 */
export function errorCodeOf(error: unknown): string | null {
  const code = (
    error as { response?: { data?: { error?: unknown } } } | null | undefined
  )?.response?.data?.error;
  return typeof code === "string" ? code : null;
}

export function isConflictErrorCode(code: string | null): code is ConflictErrorCode {
  return code != null && (CONFLICT_ERROR_CODES as readonly string[]).includes(code);
}

function asNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function asString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

/**
 * 把 409 冲突报文拆成可用的字段。**不是**冲突（状态码不是 409、或错误码不在表里、
 * 或报文体残缺）时返回 null，调用方据此走原有的通用错误分支。
 *
 * 不因缺字段而判定为「不是冲突」：状态码与错误码已经足够确认是冲突，此时
 * 正文缺失只意味着提示要退化成一句话，而不是把冲突当成普通错误弹一个裸错误码。
 */
export function parseVersionConflict(error: unknown): VersionConflict | null {
  const response = (
    error as { response?: { status?: unknown; data?: Record<string, unknown> } } | null | undefined
  )?.response;
  if (!response || response.status !== 409) return null;
  const code = asString(response.data?.error);
  if (!isConflictErrorCode(code)) return null;

  const data = response.data?.data as Record<string, unknown> | null | undefined;
  const snapshot = asString(data?.currentSnapshot);

  return {
    code,
    yourRevision: asNumber(data?.yourRevision),
    currentRevision: asNumber(data?.currentRevision),
    currentContent: asString(data?.currentContent) ?? snapshot,
    isSnapshot: snapshot !== null,
  };
}

/** 冲突的一行人读摘要，供 toast / 提示条复用（正文不进摘要，可能几万字）。 */
export function describeVersionConflict(conflict: VersionConflict): string {
  const what = conflict.isSnapshot ? "记忆" : "文档";
  if (conflict.yourRevision != null && conflict.currentRevision != null) {
    return `这篇${what}已被他人修改（你的版本 ${conflict.yourRevision}，当前版本 ${conflict.currentRevision}）`;
  }
  return `这篇${what}已被他人修改`;
}
