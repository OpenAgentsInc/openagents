export type FrameTargetHz = 60 | "display";

/** Limit render attempts without replacing elapsed time or catching up missed frames. */
export class FramePacer {
  private deadline: number | undefined;
  private previous: number | undefined;
  private target: FrameTargetHz = "display";
  private readonly periodMs = 1000 / 60;
  // rAF timestamps near a 60 Hz deadline can arrive fractionally early.
  // A narrow tolerance prevents an accidental 30 Hz cadence without admitting
  // the intervening 120/144 Hz callback.
  private readonly earlyMs = 1;

  configure(target: FrameTargetHz): void {
    if (target !== 60 && target !== "display")
      throw new Error(
        "Choose display timing or the 60 Hz performance profile.",
      );
    this.target = target;
    this.reset();
  }
  reset(): void {
    this.deadline = undefined;
    this.previous = undefined;
  }
  due(now: number): boolean {
    if (
      !Number.isFinite(now) ||
      now < 0 ||
      now > Number.MAX_SAFE_INTEGER ||
      (this.previous !== undefined && now < this.previous)
    )
      throw new Error("Use a monotonic finite frame timestamp.");
    this.previous = now;
    if (this.target === "display") return true;
    if (this.deadline === undefined) {
      this.deadline = now + this.periodMs;
      return true;
    }
    if (now + this.earlyMs < this.deadline) return false;
    const missed = Math.max(
      1,
      Math.floor((now - this.deadline) / this.periodMs) + 1,
    );
    this.deadline += missed * this.periodMs;
    // Leave one future slot after a long gap; never issue a catch-up burst.
    if (this.deadline <= now + this.earlyMs) this.deadline += this.periodMs;
    return true;
  }
}
