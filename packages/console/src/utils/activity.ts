import {
  ArrowRight,
  Bug,
  Calendar,
  Circle,
  CircleCheck,
  ClipboardList,
  FilePen,
  FileText,
  Folder,
  Package,
  Pencil,
  RefreshCw,
  SlidersHorizontal,
  Trash2,
  type LucideIcon,
} from "@lucide/vue";

export type ActivityIcon = {
  icon: LucideIcon;
  color: string;
};

const ACTION_ICONS: Record<string, ActivityIcon> = {
  PROJECT_CREATED: { icon: Folder, color: "var(--color-primary)" },
  PROJECT_UPDATED: { icon: Pencil, color: "color-mix(in oklab, var(--color-base-content) 65%, transparent)" },
  PROJECT_DELETED: { icon: Trash2, color: "var(--color-error)" },
  PROJECT_INITIALIZED: { icon: CircleCheck, color: "var(--color-success)" },
  REQUIREMENT_CREATED: { icon: FileText, color: "var(--color-info)" },
  REQUIREMENT_UPDATED: { icon: FilePen, color: "color-mix(in oklab, var(--color-base-content) 65%, transparent)" },
  FEATURE_CREATED: { icon: Package, color: "var(--color-primary)" },
  FEATURE_STAGE: { icon: ArrowRight, color: "var(--color-success)" },
  FEATURE_DOCUMENT: { icon: ClipboardList, color: "var(--color-accent)" },
  FEATURE_SCHEDULE: { icon: Calendar, color: "var(--color-warning)" },
  DEFECT_CREATED: { icon: Bug, color: "var(--color-warning)" },
  DEFECT_UPDATED: { icon: Bug, color: "color-mix(in oklab, var(--color-base-content) 65%, transparent)" },
  DEFECT_DELETED: { icon: Bug, color: "var(--color-error)" },
  DEFECT_CONVERTED: { icon: RefreshCw, color: "var(--color-info)" },
  ENV_VAR_CREATED: { icon: SlidersHorizontal, color: "var(--color-success)" },
  ENV_VAR_UPDATED: { icon: SlidersHorizontal, color: "color-mix(in oklab, var(--color-base-content) 65%, transparent)" },
  ENV_VAR_DELETED: { icon: SlidersHorizontal, color: "var(--color-error)" },
};

export function activityIcon(action: string): ActivityIcon {
  return (
    ACTION_ICONS[action] ?? {
      icon: Circle,
      color: "color-mix(in oklab, var(--color-base-content) 65%, transparent)",
    }
  );
}

function normalizeEntityType(entityType?: string | null): string | null {
  if (!entityType) return null;
  return entityType.toLowerCase();
}

/** Deep-link path inside project, or null if not navigable. */
export function activityLink(
  projectId: string,
  entityType?: string | null,
  entityId?: string | null,
): string | null {
  const type = normalizeEntityType(entityType);
  if (!type || !entityId) return null;

  if (type === "feature") {
    return `/projects/${projectId}/requirements`;
  }
  if (type === "requirement") {
    return `/projects/${projectId}/requirements/${encodeURIComponent(entityId)}`;
  }
  if (type === "defect") {
    return `/projects/${projectId}/defects`;
  }
  if (type === "env_var") {
    return `/projects/${projectId}/settings/env-vars`;
  }
  return null;
}
