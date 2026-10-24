export const BATTLE_COVERAGE_VIEWS: readonly string[];
export function battleParticipantWindow(
  samples: readonly any[],
  events?: readonly any[],
): any;
export function battleWindowAround60(row: any): any;
export function battleAllViewsCoverage(
  windows: readonly any[],
  count: number,
): any;

export function battleParticipationAim(view: string): {
  lookX: number;
  lookY: number;
};
