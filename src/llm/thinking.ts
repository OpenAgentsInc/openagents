import type { Model } from "./model-types.js";

export type ThinkingLevel = "off" | "minimal" | "low" | "medium" | "high";

export const supportsThinking = (model: Model<any>): boolean => Boolean(model.reasoning);

export const nextThinkingLevel = (current: ThinkingLevel, supported: boolean): ThinkingLevel => {
  if (!supported) return "off";
  const order: ThinkingLevel[] = ["off", "minimal", "low", "medium", "high"];
  const idx = order.indexOf(current);
  const next = order[(idx + 1) % order.length];
  return next;
};
