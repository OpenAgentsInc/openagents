export const BATTLE_TICK_MAX: number;
export const BATTLE_TICK_VIEWS: readonly string[];
export const BATTLE_VIEW_COUNTERS: readonly string[];
export interface BattleTickPlan {
  count: number;
  seed: number;
  tick: number;
  views: string[];
  cameraSeconds: number;
  time: number;
  viewport: { width: number; height: number };
  label: string;
}
export function battleTickPlan(
  env?: Record<string, string | undefined>,
): BattleTickPlan;
export function battleStateDigest(probe: any): { tick: number; sha256: string };
export function pngSize(png: ArrayBuffer | Uint8Array): {
  width: number;
  height: number;
};
export function compareRgba(
  a: ArrayBuffer | Uint8Array,
  b: ArrayBuffer | Uint8Array,
  width: number,
  height: number,
): { pixels: number; differing: number; maxDelta: number; identical: boolean };
export function compareBattleTickReports(
  a: any,
  b: any,
  loadRgba: (report: any, view: any) => Promise<ArrayBuffer | Uint8Array>,
): Promise<{
  ok: boolean;
  plan: Record<string, unknown>;
  mismatchedPlan: string[];
  revisions: [string | null, string | null];
  views: any[];
}>;
