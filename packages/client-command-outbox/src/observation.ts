import type { ObservationCacheEntry, ObservationPhase, ObservationProjection } from "./model.ts";

export const projectObservation = (input: {
  readonly entry: ObservationCacheEntry | null;
  readonly connected: boolean;
  readonly synchronizing: boolean;
  readonly nowMs: number;
}): ObservationProjection | null => {
  if (input.entry === null) return null;
  const phase: ObservationPhase = input.connected
    ? input.synchronizing
      ? "synchronizing"
      : "live"
    : "cached";
  return {
    phase,
    ageMs: Math.max(0, input.nowMs - input.entry.observedAtMs),
    value: JSON.parse(input.entry.valueJson) as unknown,
  };
};
