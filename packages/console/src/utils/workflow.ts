export const REQUIREMENT_STATUS_LABEL: Record<string, string> = {
  pending: "待处理",
  running: "运行中",
  completed: "已完成",
  abandoned: "已放弃",
};

export const DEFECT_STATUS_LABEL: Record<string, string> = {
  open: "待处理",
  processing: "处理中",
  resolved: "已解决",
  closed: "已关闭",
};

export const DEFECT_SEVERITY_LABEL: Record<string, string> = {
  critical: "致命",
  major: "严重",
  minor: "一般",
  trivial: "轻微",
};

export const SCENARIO_STATUS_LABEL: Record<string, string> = {
  pending: "待验收",
  passing: "通过",
  failing: "失败",
  blocked: "受阻",
  waived: "已豁免",
};

export const RUN_STATUS_LABEL: Record<string, string> = {
  running: "运行中",
  finished: "已结束",
  completed: "已完成",
  abandoned: "已放弃",
};
