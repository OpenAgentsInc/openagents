export type SquadActor = {
  id: number;
  position: number[];
  health: number;
  kind: number;
};
export type SquadFrame = { tick: number; actors: SquadActor[] };
export function squadMotion(
  initial: SquadActor[],
  frames: SquadFrame[],
  members: number[],
  direction: number[],
  minimumProjection?: number,
  minimumFraction?: number,
): {
  ok: boolean;
  passed: number;
  members: number;
  fraction: number;
  meanProjection: number;
  actors: {
    id: number;
    projection: number;
    distance: number;
    alive: boolean;
  }[];
};
export function squadClearance(
  frames: SquadFrame[],
  radii: readonly number[],
): {
  minimumCoreGap: number | null;
  pair: { tick: number; left: number; right: number } | null;
};
