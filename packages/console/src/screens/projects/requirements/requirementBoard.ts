import { REQUIREMENT_STATUS_LABEL } from "@/utils/workflow";

export const BOARD_STATUSES = ["pending", "running", "completed", "abandoned"] as const;
export type BoardStatus = (typeof BOARD_STATUSES)[number];
export type RequirementViewMode = "list" | "board";

export const BOARD_PAGE_SIZE = 20;
export const VIEW_STORAGE_PREFIX = "chunsun.requirements.view.";

export type RequirementCard = {
  id: string;
  status: string;
};

export type BoardColumnDef = {
  status: BoardStatus;
  label: string;
};

export const BOARD_COLUMNS: BoardColumnDef[] = BOARD_STATUSES.map(status => ({
  status,
  label: REQUIREMENT_STATUS_LABEL[status] ?? status,
}));

export function isBoardStatus(value: string): value is BoardStatus {
  return (BOARD_STATUSES as readonly string[]).includes(value);
}

export function readViewMode(projectId: string): RequirementViewMode {
  if (!projectId) return "list";
  try {
    const raw = localStorage.getItem(`${VIEW_STORAGE_PREFIX}${projectId}`);
    return raw === "board" ? "board" : "list";
  } catch {
    return "list";
  }
}

export function writeViewMode(projectId: string, mode: RequirementViewMode): void {
  if (!projectId) return;
  try {
    localStorage.setItem(`${VIEW_STORAGE_PREFIX}${projectId}`, mode);
  } catch {
    // 隐私模式等写失败时忽略，本会话内仍可切换
  }
}

export function buildColumnQuery(input: {
  status: BoardStatus;
  id?: string;
  ownerId?: string;
  page: number;
  pageSize?: number;
}): string {
  const params = new URLSearchParams();
  params.set("status", input.status);
  params.set("page", String(Math.max(1, input.page)));
  params.set("pageSize", String(input.pageSize ?? BOARD_PAGE_SIZE));
  const id = input.id?.trim();
  if (id) params.set("id", id);
  if (input.ownerId) params.set("ownerId", input.ownerId);
  return `?${params.toString()}`;
}

export function groupCardsByStatus<T extends RequirementCard>(
  cards: T[],
): Record<BoardStatus, T[]> {
  const grouped: Record<BoardStatus, T[]> = {
    pending: [],
    running: [],
    completed: [],
    abandoned: [],
  };
  for (const card of cards) {
    if (isBoardStatus(card.status)) grouped[card.status].push(card);
  }
  return grouped;
}

/** 工作流把状态改掉之后，卡片只出现在新列；没有「拖到某列」的入口。 */
export function relocateCardByStatus<T extends RequirementCard>(
  columns: Record<BoardStatus, T[]>,
  card: T,
): Record<BoardStatus, T[]> {
  const next: Record<BoardStatus, T[]> = {
    pending: columns.pending.filter(item => item.id !== card.id),
    running: columns.running.filter(item => item.id !== card.id),
    completed: columns.completed.filter(item => item.id !== card.id),
    abandoned: columns.abandoned.filter(item => item.id !== card.id),
  };
  if (isBoardStatus(card.status)) {
    next[card.status] = [card, ...next[card.status]];
  }
  return next;
}

export function appendUniqueById<T extends RequirementCard>(existing: T[], incoming: T[]): T[] {
  if (incoming.length === 0) return existing;
  const seen = new Set(existing.map(item => item.id));
  const extra = incoming.filter(item => !seen.has(item.id));
  return extra.length === 0 ? existing : [...existing, ...extra];
}

export function shouldLoadNextPage(input: {
  loaded: number;
  total: number;
  page: number;
  pageSize?: number;
  loading?: boolean;
}): boolean {
  if (input.loading) return false;
  if (input.total <= 0) return false;
  if (input.loaded >= input.total) return false;
  const size = input.pageSize ?? BOARD_PAGE_SIZE;
  return input.page * size < input.total || input.loaded < input.total;
}

export function parseListPageMeta(payload: {
  meta?: { page?: number; pageSize?: number; total?: number };
  data?: unknown;
}): { page: number; pageSize: number; total: number } | null {
  const meta = payload.meta;
  if (
    !meta ||
    typeof meta.page !== "number" ||
    typeof meta.pageSize !== "number" ||
    typeof meta.total !== "number"
  ) {
    return null;
  }
  return { page: meta.page, pageSize: meta.pageSize, total: meta.total };
}
