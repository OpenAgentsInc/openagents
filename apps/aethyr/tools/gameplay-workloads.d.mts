export const ENEMY_WORKLOADS: readonly string[];
export const AIM_WORKLOADS: readonly string[];
export function distance(a: readonly number[], b: readonly number[]): number;
export interface PatrolPose {
  id: number;
  position: readonly number[];
  alive: boolean;
}
export function enemyMotion(
  samples: readonly {
    enemies: { total: number; actors: readonly PatrolPose[] };
  }[],
): { id: number; pathMeters: number }[];
export function assertHeldCorpses(
  before: { actors: readonly (PatrolPose & { deathSeconds: number })[] },
  after: { actors: readonly (PatrolPose & { deathSeconds: number })[] },
  elapsedSeconds?: number,
): number[];
export function projectWorldTarget(options: {
  eye: readonly number[];
  viewEye?: readonly number[];
  target: readonly number[];
  point: readonly number[];
  width: number;
  height: number;
  fov?: number;
}): [number, number];
export function gameplayActivityComplete(
  activity: unknown,
  feature: string,
  options?: { editor?: boolean },
): boolean;
export function runEnemyWorkload(
  adapter: {
    state(): Promise<{
      enemies: unknown;
      renderer: { modelLoaded: boolean; submittedActors: number };
      revision: number;
      player: readonly number[];
      shaderIdentity?: string;
      gameplay: unknown;
    }>;
    wait(ms: number): Promise<unknown>;
    restore(): Promise<unknown>;
    point(target: readonly number[], eye: readonly number[]): Promise<unknown>;
    cast(ability: string, expected: boolean): Promise<unknown>;
    rejectShader?(): Promise<unknown>;
  },
  feature: string,
  durationMs: number,
): Promise<Record<string, unknown>>;
