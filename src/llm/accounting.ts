import type { Usage } from "./model-types.js";

export const normalizeUsage = (usage: Partial<Usage> | undefined): Usage => {
  const safe = usage ?? {};
  return {
    input: safe.input ?? 0,
    output: safe.output ?? 0,
    cacheRead: safe.cacheRead ?? 0,
    cacheWrite: safe.cacheWrite ?? 0,
    cost: {
      input: safe.cost?.input ?? 0,
      output: safe.cost?.output ?? 0,
      cacheRead: safe.cost?.cacheRead ?? 0,
      cacheWrite: safe.cost?.cacheWrite ?? 0,
      total: safe.cost?.total ?? 0,
    },
  };
};
