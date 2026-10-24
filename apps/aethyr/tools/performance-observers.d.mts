export function performanceObserverPlan(
  mode: "diagnostic" | "qualification" | null,
  selection?: string,
  repeats?: number,
): { observers: ("off" | "on")[]; schedule: ("off" | "on")[][] };
