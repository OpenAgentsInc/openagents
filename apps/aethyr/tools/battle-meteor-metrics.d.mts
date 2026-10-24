export interface MeteorProjectileObservation {
  id: number;
  kind: number;
  ownerEpoch: number;
  age: number;
  flight: number;
  radius: number;
  damage: number;
  position: readonly number[];
  target: readonly number[];
}
export interface MeteorFrameObservation {
  tick: number;
  ordnance: { samples: readonly MeteorProjectileObservation[] };
  probe: {
    gpu: {
      meteorCasts: number;
      meteorImpacts: number;
      participantDamage: number;
      participantKills: number;
      meteorHits: number;
    };
  };
}
export interface MeteorTimelineOptions {
  epoch: number;
  castTick: number;
  aim: readonly number[];
  castsBefore: number;
  impactsBefore: number;
  damageBefore: number;
  hitsBefore: number;
  killsBefore: number;
  requireKill: boolean;
}
export function evaluateBattleMeteors(
  frames: readonly MeteorFrameObservation[],
  options: MeteorTimelineOptions,
): {
  ok: boolean;
  failures: string[];
  summary: {
    damageRoll?: number;
    impacts?: number;
    damage?: number;
    kills?: number;
    hits?: number;
    projectiles?: unknown[];
  };
};
