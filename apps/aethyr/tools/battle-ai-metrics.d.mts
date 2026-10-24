export type AiActor = {
  id: number;
  position: number[];
  velocity: number[];
  health: number;
  team: number;
  state: number;
};
export function aiHash(value: number): number;
export function rangedBand(
  id: number,
  seed: number,
  radius: number,
): { preferred: number; inner: number; outer: number; pressure: number };
export function ledPoint(
  actor: AiActor,
  origin: number[],
  speed: number,
  maximumFlight: number,
): { flight: number; displacement: number[]; point: number[] };
export function siegeChoice(
  actors: AiActor[],
  siege: { position: number[]; team: number },
  id: number,
  tick: number,
  radius: number,
): {
  id: number | null;
  score?: number;
  point?: number[];
  candidates: { id: number; score: number; distance: number }[];
};
