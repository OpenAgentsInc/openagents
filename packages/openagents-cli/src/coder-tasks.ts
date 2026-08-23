/**
 * The task registry behind delegation in `openagents coder`.
 *
 * A fleet of child agents is not expressible as a transcript. The transcript in
 * `coder-session.ts` is an ordered list of settled and unsettled entries, which
 * is the right shape for one conversation and the wrong shape for fifteen
 * children running at once: each child has its own status, its own counters,
 * its own last activity, and its own transcript, and all of that keeps changing
 * after the entry that launched it has settled.
 *
 * So delegation state lives here instead, in one registry the renderers read
 * the same way they read a session snapshot: through immutable copies, with a
 * change callback. Nothing in this module knows how a task is drawn and nothing
 * in it knows how a child is executed. `coder-delegate.ts` runs children and
 * writes here; `coder-fleet.ts` reads here and returns rows.
 *
 * Two rules in here exist because of what they prevent:
 *
 * - An update that changes nothing returns the same object, so a child
 *   reporting identical progress does not wake every renderer.
 * - Counters are aggregated on write. Recomputing them from a stored event log
 *   at paint time is what turns a 15-way fan-out into a redraw cost that grows
 *   with the length of the run.
 */

/** Stable identity, minted before a child starts and never reused. */
export type CoderTaskId = string;

/**
 * Where a task is.
 *
 * `stopped` is a deliberate cancellation and is not `failed`: a child the
 * operator stopped did not go wrong, and reporting it as a failure teaches the
 * reader to ignore failures.
 */
export type CoderTaskStatus = "pending" | "running" | "completed" | "failed" | "stopped";

/** One thing a child did, in the shape a one-line status needs. */
export interface CoderToolActivity {
  readonly toolName: string;
  /**
   * What the child was working on, as the child's own harness described it —
   * a path, a command, a query. Undefined when the harness said nothing, and
   * then only the tool name is shown.
   */
  readonly target: string | undefined;
}

/** What a fleet row and a detail view read. Aggregated, never recomputed. */
export interface CoderTaskProgress {
  readonly toolUseCount: number;
  /**
   * Tokens attributable to this child so far.
   *
   * Providers report input usage cumulatively for the whole context, so the
   * latest input count replaces the previous one while output counts add up.
   * Summing both would multiply-count the prompt on every step, and a child
   * that read three files would appear to have spent five times what it did.
   */
  readonly tokenCount: number;
  readonly lastActivity: CoderToolActivity | undefined;
  /** Newest last, bounded by `MAX_RECENT_ACTIVITIES`. */
  readonly recentActivities: ReadonlyArray<CoderToolActivity>;
}

/** One delegated child agent. */
export interface CoderTask {
  readonly id: CoderTaskId;
  /** Three to five words. Display only, and the only text a fleet row shows. */
  readonly description: string;
  readonly prompt: string;
  /** The harness that runs the child, for example `opencode`. */
  readonly agent: string;
  readonly model: string;
  /** Where the child works. A worktree path when the child is isolated. */
  readonly cwd: string;
  readonly status: CoderTaskStatus;
  /** False while the caller is waiting on this child synchronously. */
  readonly background: boolean;
  /** True from completion until the result is read, so nothing lands silently. */
  readonly unread: boolean;
  readonly startedAt: number;
  readonly endedAt: number | undefined;
  readonly progress: CoderTaskProgress;
  /** The child's own transcript, once the harness has named it. */
  readonly transcriptPath: string | undefined;
  /** The child's final text, on completion. */
  readonly result: string | undefined;
  readonly error: string | undefined;
}

/** How many activities a task keeps. A fleet of 15 cannot keep every step. */
export const MAX_RECENT_ACTIVITIES = 5;

/** How long a stopped task stays listed, so the reader sees the transition. */
export const STOPPED_DISPLAY_MS = 3_000;

/** What `register` needs. Everything else is derived or arrives later. */
export interface CoderTaskInput {
  readonly id: CoderTaskId;
  readonly description: string;
  readonly prompt: string;
  readonly agent: string;
  readonly model: string;
  readonly cwd: string;
  readonly background: boolean;
}

interface TaskRecord {
  task: CoderTask;
  /** Kept out of the task so a snapshot cannot cancel a child. */
  controller: AbortController | undefined;
  /** Provider input usage is cumulative, so the latest reading replaces. */
  latestInputTokens: number;
  cumulativeOutputTokens: number;
}

/**
 * The registry. One per session.
 *
 * Every mutator is a no-op on an unknown id rather than a throw: a child that
 * outlives a cleared registry should not be able to crash the interface it can
 * no longer draw into.
 */
export class CoderTaskRegistry {
  private readonly records = new Map<CoderTaskId, TaskRecord>();
  /** Insertion order, so the fleet does not reshuffle as children finish. */
  private readonly order: CoderTaskId[] = [];
  private readonly listeners = new Set<() => void>();

  /**
   * Register a child before it starts.
   *
   * Registering first is what makes a child that fails to launch visible. If
   * the launcher registered on success, a harness that is not installed would
   * produce nothing at all on screen.
   */
  register(input: CoderTaskInput, nowMs = Date.now()): CoderTask {
    const task: CoderTask = {
      id: input.id,
      description: input.description,
      prompt: input.prompt,
      agent: input.agent,
      model: input.model,
      cwd: input.cwd,
      status: "pending",
      background: input.background,
      unread: false,
      startedAt: nowMs,
      endedAt: undefined,
      progress: {
        toolUseCount: 0,
        tokenCount: 0,
        lastActivity: undefined,
        recentActivities: [],
      },
      transcriptPath: undefined,
      result: undefined,
      error: undefined,
    };

    this.records.set(task.id, {
      task,
      controller: undefined,
      latestInputTokens: 0,
      cumulativeOutputTokens: 0,
    });
    this.order.push(task.id);
    this.emit();
    return task;
  }

  /** Mark a child running and store the handle that can cancel it. */
  start(id: CoderTaskId, controller: AbortController): void {
    const record = this.records.get(id);
    if (record === undefined) return;
    record.controller = controller;
    this.write(record, { status: "running" });
  }

  /** Note the child's own transcript, once its harness has named one. */
  attachTranscript(id: CoderTaskId, transcriptPath: string): void {
    const record = this.records.get(id);
    if (record === undefined) return;
    this.write(record, { transcriptPath });
  }

  /**
   * Count one tool use and remember what it was.
   *
   * Counted on the call rather than the result, because a fleet row has to say
   * what a child is doing now and a long-running command would otherwise leave
   * the row reading `Initializing…` for its whole duration.
   */
  recordToolUse(id: CoderTaskId, activity: CoderToolActivity): void {
    const record = this.records.get(id);
    if (record === undefined) return;
    const recent = [...record.task.progress.recentActivities, activity];
    while (recent.length > MAX_RECENT_ACTIVITIES) recent.shift();
    this.write(record, {
      progress: {
        ...record.task.progress,
        toolUseCount: record.task.progress.toolUseCount + 1,
        lastActivity: activity,
        recentActivities: recent,
      },
    });
  }

  /** Fold one usage report into the token count. See `tokenCount`. */
  recordTokens(id: CoderTaskId, usage: { readonly input: number; readonly output: number }): void {
    const record = this.records.get(id);
    if (record === undefined) return;
    record.latestInputTokens = usage.input;
    record.cumulativeOutputTokens += usage.output;
    this.write(record, {
      progress: {
        ...record.task.progress,
        tokenCount: record.latestInputTokens + record.cumulativeOutputTokens,
      },
    });
  }

  complete(id: CoderTaskId, result: string, nowMs = Date.now()): void {
    const record = this.records.get(id);
    if (record === undefined) return;
    record.controller = undefined;
    this.write(record, {
      status: "completed",
      endedAt: nowMs,
      result,
      // Unread only where someone has to come back for it. A synchronous child
      // was already awaited by its caller, so marking it unread would leave a
      // permanent badge for a result that has been read.
      unread: record.task.background,
    });
  }

  fail(id: CoderTaskId, error: string, nowMs = Date.now()): void {
    const record = this.records.get(id);
    if (record === undefined) return;
    record.controller = undefined;
    this.write(record, { status: "failed", endedAt: nowMs, error, unread: record.task.background });
  }

  /** Cancel a running child. Terminal tasks are left alone. */
  stop(id: CoderTaskId, nowMs = Date.now()): boolean {
    const record = this.records.get(id);
    if (record === undefined) return false;
    if (isTerminal(record.task.status)) return false;
    record.controller?.abort();
    record.controller = undefined;
    this.write(record, { status: "stopped", endedAt: nowMs, unread: false });
    return true;
  }

  /** Clear the unread badge once the result has been shown. */
  markRead(id: CoderTaskId): void {
    const record = this.records.get(id);
    if (record === undefined) return;
    this.write(record, { unread: false });
  }

  /**
   * Drop tasks nothing will look at again.
   *
   * Stopped tasks linger briefly so the reader sees them stop, and completed
   * tasks stay until read. Everything else terminal is forgotten, because a
   * long session that keeps every child forever grows a fleet block nobody
   * asked for.
   */
  prune(nowMs = Date.now(), graceMs = STOPPED_DISPLAY_MS): void {
    let changed = false;
    for (const id of [...this.order]) {
      const record = this.records.get(id);
      if (record === undefined) continue;
      const task = record.task;
      if (!isTerminal(task.status)) continue;
      if (task.unread) continue;
      const endedAt = task.endedAt ?? task.startedAt;
      if (nowMs - endedAt < graceMs) continue;
      this.records.delete(id);
      this.order.splice(this.order.indexOf(id), 1);
      changed = true;
    }
    if (changed) this.emit();
  }

  get(id: CoderTaskId): CoderTask | undefined {
    return this.records.get(id)?.task;
  }

  /** Every task, oldest first. The array is a copy; the tasks are frozen. */
  list(): ReadonlyArray<CoderTask> {
    const out: CoderTask[] = [];
    for (const id of this.order) {
      const record = this.records.get(id);
      if (record !== undefined) out.push(record.task);
    }
    return out;
  }

  /** How many children are in flight, which is what a concurrency cap reads. */
  get activeCount(): number {
    let count = 0;
    for (const record of this.records.values()) {
      if (!isTerminal(record.task.status)) count += 1;
    }
    return count;
  }

  onChange(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Cancel everything still running, for shutdown. */
  stopAll(nowMs = Date.now()): void {
    for (const id of [...this.order]) this.stop(id, nowMs);
  }

  private write(record: TaskRecord, patch: Partial<CoderTask>): void {
    const next = { ...record.task, ...patch };
    if (isSameTask(record.task, next)) return;
    record.task = Object.freeze(next);
    this.emit();
  }

  private emit(): void {
    for (const listener of this.listeners) listener();
  }
}

export function isTerminal(status: CoderTaskStatus): boolean {
  return status === "completed" || status === "failed" || status === "stopped";
}

/**
 * Whether an update is worth telling anyone about.
 *
 * Compared field by field rather than by reference because every writer builds
 * a fresh object. Progress is compared on what a row shows, so a child
 * reporting the same counters and the same activity twice does not repaint the
 * fleet.
 */
function isSameTask(previous: CoderTask, next: CoderTask): boolean {
  return (
    previous.status === next.status &&
    previous.unread === next.unread &&
    previous.background === next.background &&
    previous.endedAt === next.endedAt &&
    previous.result === next.result &&
    previous.error === next.error &&
    previous.transcriptPath === next.transcriptPath &&
    previous.progress.toolUseCount === next.progress.toolUseCount &&
    previous.progress.tokenCount === next.progress.tokenCount &&
    previous.progress.lastActivity?.toolName === next.progress.lastActivity?.toolName &&
    previous.progress.lastActivity?.target === next.progress.lastActivity?.target
  );
}
