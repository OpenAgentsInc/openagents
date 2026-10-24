export interface FrameCompletion {
  frame: number;
  submittedAt: number;
  /** Synchronous submit return; submittedAt precedes encoder.finish and queue.submit. */
  submitReturnedAt?: number;
  completedAt: number;
  latencyMs: number;
  ok: boolean;
}

/** Bounded optional queue-fence observations, independent of the engine clock. */
export class FrameCompletions {
  private current?: {
    active: boolean;
    capacity: number;
    first: number | null;
    last: number | null;
    submitted: number;
    truncated: boolean;
    events: FrameCompletion[];
  };
  start(capacity = 20_000) {
    if (!Number.isSafeInteger(capacity) || capacity < 1 || capacity > 20_000)
      throw new Error("Choose a frame observation limit from 1 to 20,000.");
    this.current = {
      active: true,
      capacity,
      first: null,
      last: null,
      submitted: 0,
      truncated: false,
      events: [],
    };
  }
  stop() {
    if (this.current) this.current.active = false;
  }
  get recording() {
    return this.current?.active ?? false;
  }
  submitted(frame: number, at: number, submitReturnedAt?: number) {
    const session = this.current;
    if (!session?.active) return undefined;
    if (session.submitted >= session.capacity) {
      session.truncated = true;
      return undefined;
    }
    session.submitted++;
    session.first ??= frame;
    session.last = frame;
    let completed = false;
    return (completedAt: number, ok = true) => {
      if (completed) return;
      completed = true;
      // Capture the recording session so a late fence cannot contaminate a new run.
      session.events.push({
        frame,
        submittedAt: at,
        ...(submitReturnedAt === undefined ? {} : { submitReturnedAt }),
        completedAt,
        latencyMs: Math.max(0, completedAt - at),
        ok,
      });
    };
  }
  snapshot() {
    return {
      completionRange: {
        firstFrame: this.current?.first ?? null,
        lastFrame: this.current?.last ?? null,
      },
      completedFrames:
        this.current?.events.map((event) => ({ ...event })) ?? [],
      completionTruncated: this.current?.truncated ?? false,
    };
  }
}
