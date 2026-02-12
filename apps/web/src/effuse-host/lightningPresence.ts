export const DEFAULT_EXECUTOR_PRESENCE_MAX_AGE_MS = 10_000;

export const isExecutorPresenceFresh = (input: {
  readonly lastSeenAtMs: number | null;
  readonly nowMs: number;
  readonly maxAgeMs?: number | undefined;
}): boolean => {
  if (typeof input.lastSeenAtMs !== "number" || !Number.isFinite(input.lastSeenAtMs)) return false;
  const maxAgeMs =
    typeof input.maxAgeMs === "number" && Number.isFinite(input.maxAgeMs)
      ? Math.max(0, Math.floor(input.maxAgeMs))
      : DEFAULT_EXECUTOR_PRESENCE_MAX_AGE_MS;

  const nowMs = Number.isFinite(input.nowMs) ? Math.max(0, Math.floor(input.nowMs)) : Date.now();
  const lastSeenAtMs = Math.max(0, Math.floor(input.lastSeenAtMs));
  return nowMs - lastSeenAtMs <= maxAgeMs;
};

