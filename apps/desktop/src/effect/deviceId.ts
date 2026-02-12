const STORAGE_KEY = "openagents.desktop.executor.deviceId.v1";

const randomId = (): string => {
  const cryptoAny = (globalThis as unknown as { crypto?: { randomUUID?: () => string } }).crypto;
  if (cryptoAny && typeof cryptoAny.randomUUID === "function") return cryptoAny.randomUUID();
  return `${Date.now().toString(16)}-${Math.random().toString(16).slice(2)}`;
};

export const getOrCreateDesktopDeviceId = (): string => {
  try {
    if (typeof window === "undefined") return `desktop-${randomId()}`;
    const storage = window.localStorage;
    const existing = storage.getItem(STORAGE_KEY);
    if (typeof existing === "string" && existing.trim().length > 0) {
      return existing.trim().slice(0, 128);
    }
    const created = `desktop-${randomId()}`;
    storage.setItem(STORAGE_KEY, created);
    return created;
  } catch {
    return `desktop-${randomId()}`;
  }
};
