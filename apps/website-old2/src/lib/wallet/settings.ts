export interface UserSettings {
  depositMaxFee: { type: "rate"; satPerVbyte: number };
  syncIntervalSecs?: number;
  lnurlDomain?: string;
  preferSparkOverLightning?: boolean;
}

const KEY = "wallet_user_settings_v1";
const defaults: UserSettings = { depositMaxFee: { type: "rate", satPerVbyte: 1 } };

export function getSettings(): UserSettings {
  try {
    const raw = typeof localStorage !== "undefined" ? localStorage.getItem(KEY) : null;
    if (!raw) return defaults;
    const p = JSON.parse(raw) as Partial<UserSettings>;
    return {
      depositMaxFee: p?.depositMaxFee && typeof (p.depositMaxFee as { satPerVbyte?: number }).satPerVbyte === "number"
        ? (p.depositMaxFee as UserSettings["depositMaxFee"])
        : defaults.depositMaxFee,
      syncIntervalSecs: typeof p?.syncIntervalSecs === "number" ? p.syncIntervalSecs : undefined,
      lnurlDomain: typeof p?.lnurlDomain === "string" ? p.lnurlDomain : undefined,
      preferSparkOverLightning: typeof p?.preferSparkOverLightning === "boolean" ? p.preferSparkOverLightning : undefined,
    };
  } catch {
    return defaults;
  }
}

export function saveSettings(settings: UserSettings): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(settings));
  } catch {
    // ignore
  }
}
