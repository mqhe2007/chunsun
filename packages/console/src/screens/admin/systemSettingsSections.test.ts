import { describe, expect, test } from "vitest";
import {
  SETTINGS_SECTION_KEYS,
  pickSectionSettings,
  sectionKeySetsAreDisjoint,
} from "./systemSettingsSections";

describe("systemSettingsSections", () => {
  test("pickSectionSettings 只返回该 section 的键", () => {
    const settings = {
      "registration.inviteOnly": "true",
      "security.passwordMinLength": "10",
      "email.fromAddress": "a@b.c",
      "rateLimit.generalMax": "100",
      "rateLimit.generalWindowMs": "60000",
      "rateLimit.authMax": "20",
      "rateLimit.authWindowMs": "60000",
    };

    expect(pickSectionSettings("registration", settings)).toEqual({
      "registration.inviteOnly": "true",
    });

    expect(pickSectionSettings("rateLimit", settings)).toEqual({
      "rateLimit.generalMax": "100",
      "rateLimit.generalWindowMs": "60000",
      "rateLimit.authMax": "20",
      "rateLimit.authWindowMs": "60000",
    });

    expect(Object.keys(pickSectionSettings("email", settings))).toEqual([
      ...SETTINGS_SECTION_KEYS.email,
    ]);
  });

  test("各 section 键集合互不重叠且覆盖 UI 字段", () => {
    expect(sectionKeySetsAreDisjoint()).toBe(true);
    expect(SETTINGS_SECTION_KEYS.registration).toContain("registration.inviteOnly");
    expect(SETTINGS_SECTION_KEYS.security).toEqual(
      expect.arrayContaining([
        "security.passwordMinLength",
        "security.loginMaxAttempts",
      ]),
    );
    expect(SETTINGS_SECTION_KEYS.email).toContain("email.smtpPassword");
  });
});
