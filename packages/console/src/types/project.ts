export type ProjectMemberRole = "OWNER" | "ADMIN" | "MEMBER";

export type ProjectMember = {
  id: string;
  userId: string;
  role: ProjectMemberRole;
  createdAt: string;
  user: {
    id: string;
    email: string;
    nickname: string | null;
    qq: string | null;
  };
};

export type RepositorySummary = {
  id: string;
  projectId: string;
  name: string;
  slug: string;
  rootHint: string | null;
  isDefault: boolean;
  createdAt: string;
  updatedAt: string;
};

export type ProjectStatistics = {
  requirements?: {
    total: number;
    pending: number;
    processing: number;
    completed: number;
    abandoned: number;
    running?: number;
    byStatus?: Record<string, number>;
  };
  board?: {
    total: number;
    active: number;
    done: number;
    scheduled?: number;
    unscheduledActive?: number;
    byStage?: Record<string, number>;
  };
  rates?: {
    requirementCompletionPct: number | null;
    boardDonePct: number | null;
    scheduleCoveragePct: number | null;
  };
  defects?: {
    total: number;
    open: number;
    processing: number;
    resolved: number;
    closed: number;
    byStatus?: Record<string, number>;
    critical: number;
  };
};

export type ProjectActivity = {
  id: string;
  action: string;
  entityType?: string | null;
  entityId?: string | null;
  description: string;
  metadata?: unknown;
  createdAt: string;
  user?: {
    id?: string;
    nickname: string | null;
    qq?: string | null;
  } | null;
};

export type Project = {
  id: string;
  userId: string;
  name: string;
  description?: string | null;
  projectPath?: string | null;
  statistics?: ProjectStatistics;
  enableProposeStage?: boolean;
  enablePlanStage?: boolean;
  enableTasksStage?: boolean;
  enableImplementStage?: boolean;
  enableTestStage?: boolean;
  enabledStages?: string[];
  requireCasesPassed?: boolean;
  enableSchedule?: boolean;
  /** 是否已配置项目密钥（不含密钥明文） */
  hasSecretKey?: boolean;
  repositories?: RepositorySummary[];
  createdAt: string;
  updatedAt: string;
};
