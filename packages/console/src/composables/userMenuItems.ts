import type { DropdownItem } from "@/ui";

/**
 * 左下角账户菜单的纯逻辑构建层（与 Vue / 路由解耦，便于 node 环境单测）。
 * 菜单项顺序：账户设置 →（管理员可见的）系统管理 → 使用文档 → 退出登录。
 */

/** 文档中心地址：同源营销侧文档站，由后端在 /docs 挂载 */
export const DOCS_PATH = "/docs";

export const USER_MENU_LABELS = {
  profile: "账户设置",
  admin: "系统管理",
  docs: "使用文档",
  logout: "退出登录",
} as const;

export type UserMenuActions = {
  /** 进入账户设置 */
  goProfile: () => void;
  /** 进入系统管理（仅管理员） */
  goAdmin: () => void;
  /** 跳转使用文档中心 */
  goDocs: () => void;
  /** 退出登录 */
  logout: () => void;
};

export type UserMenuOptions = {
  /** 是否允许出现「系统管理」入口（控制台主框架 true，系统管理工作区 false） */
  includeSystemAdmin: boolean;
  /** 当前用户是否为系统管理员 */
  isAdmin: boolean;
};

export function buildUserMenuItems(
  actions: UserMenuActions,
  options: UserMenuOptions,
): DropdownItem[] {
  const items: DropdownItem[] = [
    { label: USER_MENU_LABELS.profile, command: actions.goProfile },
  ];

  if (options.includeSystemAdmin && options.isAdmin) {
    items.push({ label: USER_MENU_LABELS.admin, command: actions.goAdmin });
  }

  items.push({ label: USER_MENU_LABELS.docs, command: actions.goDocs });
  items.push({ label: USER_MENU_LABELS.logout, command: actions.logout });

  return items;
}
