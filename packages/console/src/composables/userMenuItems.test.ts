import { describe, expect, test, vi } from "vitest";
import {
  DOCS_PATH,
  USER_MENU_LABELS,
  buildUserMenuItems,
  type UserMenuActions,
} from "./userMenuItems";

function makeActions() {
  return {
    goProfile: vi.fn(),
    goAdmin: vi.fn(),
    goDocs: vi.fn(),
    logout: vi.fn(),
  } satisfies UserMenuActions;
}

function labelsOf(items: ReturnType<typeof buildUserMenuItems>) {
  return items.map(item => item.label);
}

describe("buildUserMenuItems", () => {
  test("普通用户菜单：账户设置 → 使用文档 → 退出登录，不含系统管理", () => {
    const items = buildUserMenuItems(makeActions(), {
      includeSystemAdmin: true,
      isAdmin: false,
    });

    expect(labelsOf(items)).toEqual([
      USER_MENU_LABELS.profile,
      USER_MENU_LABELS.docs,
      USER_MENU_LABELS.logout,
    ]);
  });

  test("管理员在控制台主框架可见系统管理，使用文档位于其之后、退出登录之前", () => {
    const items = buildUserMenuItems(makeActions(), {
      includeSystemAdmin: true,
      isAdmin: true,
    });

    expect(labelsOf(items)).toEqual([
      USER_MENU_LABELS.profile,
      USER_MENU_LABELS.admin,
      USER_MENU_LABELS.docs,
      USER_MENU_LABELS.logout,
    ]);
  });

  test("系统管理工作区（includeSystemAdmin=false）即使是管理员也不显示系统管理，但仍有使用文档", () => {
    const items = buildUserMenuItems(makeActions(), {
      includeSystemAdmin: false,
      isAdmin: true,
    });

    expect(labelsOf(items)).toEqual([
      USER_MENU_LABELS.profile,
      USER_MENU_LABELS.docs,
      USER_MENU_LABELS.logout,
    ]);
  });

  test("每个菜单项的 command 正确绑定到对应动作", () => {
    const actions = makeActions();
    const items = buildUserMenuItems(actions, {
      includeSystemAdmin: true,
      isAdmin: true,
    });
    const byLabel = new Map(items.map(item => [item.label, item]));

    byLabel.get(USER_MENU_LABELS.profile)!.command!();
    byLabel.get(USER_MENU_LABELS.admin)!.command!();
    byLabel.get(USER_MENU_LABELS.docs)!.command!();
    byLabel.get(USER_MENU_LABELS.logout)!.command!();

    expect(actions.goProfile).toHaveBeenCalledTimes(1);
    expect(actions.goAdmin).toHaveBeenCalledTimes(1);
    expect(actions.goDocs).toHaveBeenCalledTimes(1);
    expect(actions.logout).toHaveBeenCalledTimes(1);
  });

  test("文档入口指向同源文档中心 /docs", () => {
    expect(DOCS_PATH).toBe("/docs");
  });
});
