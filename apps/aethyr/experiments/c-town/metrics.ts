import type { CTownBackend } from "./config.ts";
import {
  FrameMetrics,
  validateFrameRequest,
  type FrameBenchmarkRequest,
  type FrameBenchmarkResult,
} from "../../packages/runtime/frame-metrics.ts";
import type { SpellSwarmCount } from "../../packages/runtime/agent-contract.ts";
export { type FrameSample as CTownFrameSample } from "../../packages/runtime/frame-metrics.ts";
export type CTownBenchmarkRequest =
  FrameBenchmarkRequest | { op: "readSwarm" } | { op: "readBattle" };
export interface SpellSwarmProbe {
  count: SpellSwarmCount;
  finite: boolean;
  maxDistanceFromCenter: number;
  positions: number[][];
  simulationSteps: number;
  stateSha256?: string;
}
export type CTownBenchmarkResult = FrameBenchmarkResult<CTownBackend> & {
  swarmProbe?: SpellSwarmProbe | null;
  battleProbe?:
    import("../../packages/renderer/battle-crowd.ts").BattleCrowdProbe | null;
};
export function validateCTownRequest(value: CTownBenchmarkRequest): void {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("Choose a bounded frame benchmark request.");
  if (value?.op === "readSwarm" || value?.op === "readBattle") {
    if (Object.keys(value).length !== 1)
      throw new Error("Read creatures without changing benchmark settings.");
    return;
  }
  validateFrameRequest(value);
}
/** Compatibility name for the original C-town experiment protocol. */
export class CTownMetrics extends FrameMetrics<CTownBackend> {}
