// Computer-use action timeline — named beats that read as user actions.
//
// Modeled on executor's `e2e/src/timeline.ts`: an append-only ledger of named
// step beats. This is the basis for later distillation (a session timeline can
// be lowered into a committed black-box scenario), so it stays deliberately
// minimal and serializable: every beat is a plain, JSON-safe record.
//
// Public-safety note: a beat carries an intent `label` and a small `detail`
// map. NEVER put secrets, tokens, prompts, cookie values, or raw credentials in
// a beat — these are part of the public-safe artifact set.

export type TimelineSurface = "browser" | "terminal" | "filesystem" | "mcp";

export type TimelineBeatStatus = "ok" | "error";

export interface TimelineBeat {
  /** Wall-clock ms since the timeline started (monotonic-ish, for ordering). */
  readonly at: number;
  /** Which tool surface produced the beat. */
  readonly surface: TimelineSurface;
  /** A human-readable intent label, e.g. "navigate to /login". */
  readonly label: string;
  /** Outcome of the beat. */
  readonly status: TimelineBeatStatus;
  /**
   * Small, public-safe structured detail (selectors-as-intent, urls, exit
   * codes). MUST NOT contain secrets/tokens/prompts/cookie values.
   */
  readonly detail?: Readonly<Record<string, string | number | boolean>>;
}

export interface TimelineSnapshot {
  /** Wall-clock ms when the timeline clock started. */
  readonly startedAt: number;
  readonly beats: ReadonlyArray<TimelineBeat>;
}

export interface Timeline {
  /** Append a named beat. Returns the appended beat. */
  readonly beat: (input: {
    readonly surface: TimelineSurface;
    readonly label: string;
    readonly status?: TimelineBeatStatus;
    readonly detail?: Readonly<Record<string, string | number | boolean>>;
  }) => TimelineBeat;
  /** Immutable snapshot of the timeline so far. */
  readonly snapshot: () => TimelineSnapshot;
}

export interface MakeTimelineOptions {
  /** Injectable clock for deterministic tests. Defaults to `Date.now`. */
  readonly now?: () => number;
}

export function makeTimeline(options: MakeTimelineOptions = {}): Timeline {
  const now = options.now ?? Date.now;
  const startedAt = now();
  const beats: TimelineBeat[] = [];
  return {
    beat: (input) => {
      const beat: TimelineBeat = {
        at: now() - startedAt,
        surface: input.surface,
        label: input.label,
        status: input.status ?? "ok",
        ...(input.detail ? { detail: input.detail } : {}),
      };
      beats.push(beat);
      return beat;
    },
    snapshot: () => ({ startedAt, beats: [...beats] }),
  };
}
