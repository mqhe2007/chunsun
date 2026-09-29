/**
 * 「这一行确实还不存在」与「这次请求失败了」的分辨。
 *
 * 两者都会让调用方拿到一个 rejected promise，但善后动作完全相反：
 *
 * - **行不存在**是良性空状态，可以拿空正文开始编辑，用 `revision = 0` 表达
 *   「我认为这行还不存在」——后端对此的定义正是 upsert 的建行意图。
 * - **请求失败**（网络抖动、5xx、鉴权过期）意味着**我们不知道**服务端有什么。
 *   按空文档放行，用户会在一个空白编辑器里写下内容，然后带着 `revision = 0`
 *   提交，把一份原本存在的记忆或文档**整篇覆盖**掉。
 *
 * 早先知识库工作台记忆分支的 `try/catch` 没有做这个区分：任何异常都退化成
 * 「空记忆 + revision 0」。乐观锁版本号丢失的 bug 掩盖了它（反正版本号恒为 0，
 * 表现不出差别），但版本号修好之后，这个 `catch` 会立刻变成真实的数据覆盖风险。
 *
 * 抽成纯函数与 `versionConflict.ts` 同一个理由：错误分支写错了没人会立刻发现，
 * 要等下次真的网络故障或真的空行。渲染留在组件里，这里只做判定。
 */

import { errorCodeOf } from "./versionConflict";

/**
 * 「后端明确告诉我这行不存在」的错误码——404 且带下列码之一。
 *
 * 与 `api.ts` 里被放行的两个码同源（那里用它们跳过通用错误 toast）：
 * 空状态不该弹错误提示，该由页面渲染成「尚无内容，可直接编辑」。
 */
export const ABSENT_ROW_ERROR_CODES = ["MEMORY_NOT_FOUND", "CONTEXT_NOT_FOUND"] as const;

export type AbsentRowErrorCode = (typeof ABSENT_ROW_ERROR_CODES)[number];

export function isAbsentRowErrorCode(code: string | null): code is AbsentRowErrorCode {
  return code != null && (ABSENT_ROW_ERROR_CODES as readonly string[]).includes(code);
}

/**
 * 这次失败是否等价于「这行不存在」。
 *
 * **必须是 404**：光看错误码不够 —— 同一套错误码若被别的路径以别的状态码抛出，
 * 只看码会把一个需要用户处理的问题静默降级成空状态。状态码缺省（非 axios 错误、
 * 网络层异常）一律返回 false，让调用方走失败分支。
 */
export function isAbsentRowError(error: unknown): boolean {
  const status = (error as { response?: { status?: unknown } } | null | undefined)?.response
    ?.status;
  if (status !== 404) return false;
  return isAbsentRowErrorCode(errorCodeOf(error));
}
