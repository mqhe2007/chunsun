/** 平台设置子页与对应 system settings 键（不含站点 publicOrigin，走 /admin/instance）。 */

export type SettingsSection =
  | "site"
  | "registration"
  | "security"
  | "rateLimit"
  | "email";

export type SettingsMap = Record<string, string>;

export const SETTINGS_SECTION_KEYS: Record<
  Exclude<SettingsSection, "site">,
  readonly string[]
> = {
  registration: ["registration.inviteOnly"],
  security: [
    "security.passwordMinLength",
    "security.passwordRequireNumber",
    "security.passwordRequireUppercase",
    "security.passwordRequireSpecialChar",
    "security.loginMaxAttempts",
    "security.loginLockoutMinutes",
  ],
  rateLimit: [
    "rateLimit.generalMax",
    "rateLimit.generalWindowMs",
    "rateLimit.authMax",
    "rateLimit.authWindowMs",
  ],
  email: [
    "email.fromAddress",
    "email.fromName",
    "email.smtpHost",
    "email.smtpPort",
    "email.smtpSecure",
    "email.smtpUser",
    "email.smtpPassword",
  ],
};

/** 从完整 settings 中挑出指定子页要 PATCH 的键值。 */
export function pickSectionSettings(
  section: Exclude<SettingsSection, "site">,
  settings: SettingsMap,
): SettingsMap {
  const patch: SettingsMap = {};
  for (const key of SETTINGS_SECTION_KEYS[section]) {
    if (Object.prototype.hasOwnProperty.call(settings, key)) {
      patch[key] = settings[key] ?? "";
    } else {
      patch[key] = "";
    }
  }
  return patch;
}

/** 各子页键集合互不重叠（用于验收）。 */
export function sectionKeySetsAreDisjoint(): boolean {
  const seen = new Set<string>();
  for (const keys of Object.values(SETTINGS_SECTION_KEYS)) {
    for (const key of keys) {
      if (seen.has(key)) return false;
      seen.add(key);
    }
  }
  return true;
}
