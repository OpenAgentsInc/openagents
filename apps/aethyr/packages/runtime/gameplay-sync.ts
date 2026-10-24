import type { GameplayVisualState } from "../renderer/gameplay-effects.ts";

type VisualSnapshot = GameplayVisualState & {
  version: number;
  objectCount: number;
};
type VisualRenderer = {
  setGameplayState(mode: "day" | "night", state: GameplayVisualState): void;
};

/** Reuse only idle visuals from the trusted kernel; temporal state always advances. */
export class GameplayVisualSync {
  private idle?: {
    renderer: VisualRenderer;
    mode: "day" | "night";
    version: number;
    objectCount: number;
    destroyed: readonly number[];
  };

  reset(): void {
    this.idle = undefined;
  }

  apply(
    renderer: VisualRenderer,
    mode: "day" | "night",
    state: VisualSnapshot,
  ): boolean {
    if (!state.objectCount) {
      this.reset();
      return false;
    }
    const rubble = state.rubble ?? state.castle?.rubble ?? [];
    const quiescent =
      Number.isSafeInteger(state.version) &&
      state.version >= 0 &&
      Array.isArray(state.destroyed) &&
      Array.isArray(state.debris) &&
      !state.debris.length &&
      Array.isArray(state.effects) &&
      !state.effects.length &&
      Array.isArray(rubble) &&
      !rubble.length;
    const previous = this.idle;
    if (
      quiescent &&
      previous?.renderer === renderer &&
      previous.mode === mode &&
      previous.version === state.version &&
      previous.objectCount === state.objectCount &&
      previous.destroyed.length === state.destroyed.length &&
      previous.destroyed.every((id, index) => id === state.destroyed[index])
    )
      return false;

    // Cache only after validation and application succeed. A rejected visual
    // candidate must not suppress a later retry of the working state.
    renderer.setGameplayState(mode, state);
    this.idle = quiescent
      ? {
          renderer,
          mode,
          version: state.version,
          objectCount: state.objectCount,
          destroyed: state.destroyed.slice(),
        }
      : undefined;
    return true;
  }
}
