/** Internal session/checkpoint bound; screenshot overrides have a separate limit. */
export const MAX_WORLD_ANIMATION_TIME = 365 * 24 * 60 * 60;

export function validWorldAnimationTime(value: unknown): value is number {
  return (
    typeof value === "number" &&
    Number.isFinite(value) &&
    value >= 0 &&
    value <= MAX_WORLD_ANIMATION_TIME
  );
}

/** A monotonic world age, anchored only after the first world frame is ready. */
export class WorldClock {
  private anchor: number | undefined;
  private latestTimestamp = 0;
  private age: number;

  constructor(private readonly restoredAge = 0) {
    if (!validWorldAnimationTime(restoredAge))
      throw new RangeError(
        "World animation time is outside its session limit.",
      );
    this.age = restoredAge;
  }
  get time(): number {
    return this.age;
  }

  start(timestampMilliseconds: number): number {
    this.checkTimestamp(timestampMilliseconds);
    if (this.anchor === undefined) {
      this.anchor = timestampMilliseconds;
      this.latestTimestamp = timestampMilliseconds;
    }
    return this.sample(timestampMilliseconds);
  }

  sample(timestampMilliseconds: number): number {
    this.checkTimestamp(timestampMilliseconds);
    if (
      this.anchor === undefined ||
      timestampMilliseconds <= this.latestTimestamp
    )
      return this.age;
    const next =
      this.restoredAge + (timestampMilliseconds - this.anchor) / 1000;
    // Fail explicitly beyond the one-year session bound instead of freezing effects.
    if (!validWorldAnimationTime(next))
      throw new RangeError("World animation time exceeded its session limit.");
    this.latestTimestamp = timestampMilliseconds;
    this.age = Math.max(this.age, next);
    return this.age;
  }
  private checkTimestamp(value: number) {
    if (!Number.isFinite(value) || value < 0 || value > Number.MAX_SAFE_INTEGER)
      throw new RangeError(
        "World clock timestamps must be finite milliseconds.",
      );
  }
}
