const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null;

export type StoredPaneRect = {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
};

const isFiniteNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);

export const clampPaneRectToScreen = (
  rect: StoredPaneRect,
  screen: { readonly width: number; readonly height: number },
): StoredPaneRect => {
  const width = Math.max(320, Math.min(rect.width, Math.max(320, screen.width)));
  const height = Math.max(220, Math.min(rect.height, Math.max(220, screen.height)));
  const maxX = Math.max(0, screen.width - width);
  const maxY = Math.max(0, screen.height - height);
  const x = Math.max(0, Math.min(rect.x, maxX));
  const y = Math.max(0, Math.min(rect.y, maxY));
  return { x, y, width, height };
};

export const parseStoredPaneRect = (value: unknown): StoredPaneRect | null => {
  const rec = asRecord(value);
  if (!rec) return null;
  const { x, y, width, height } = rec;
  if (!isFiniteNumber(x) || !isFiniteNumber(y) || !isFiniteNumber(width) || !isFiniteNumber(height)) return null;
  if (width <= 0 || height <= 0) return null;
  return { x, y, width, height };
};

export const readStoredPaneRect = (
  key: string,
  screen: { readonly width: number; readonly height: number },
): StoredPaneRect | null => {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(key);
    if (!raw) return null;
    const parsed = parseStoredPaneRect(JSON.parse(raw));
    if (!parsed) return null;
    return clampPaneRectToScreen(parsed, screen);
  } catch {
    return null;
  }
};

export const writeStoredPaneRect = (key: string, rect: StoredPaneRect): void => {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(key, JSON.stringify(rect));
  } catch {
    // ignore storage failures
  }
};
