const REF = /^[a-z][a-z0-9._/-]{2,511}$/u;
const DIGEST = /^sha256:[0-9a-f]{64}$/u;

export const CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA =
  "openagents.cloud_computer_command_stream_cursor.v1" as const;
export const CLOUD_COMPUTER_COMMAND_STREAM_PAGE_SCHEMA =
  "openagents.cloud_computer_command_stream_page.v1" as const;
export const CLOUD_COMPUTER_COMMAND_STREAM_RESET_SCHEMA =
  "openagents.cloud_computer_command_stream_reset.v1" as const;

export type CloudComputerCommandStreamScope = Readonly<{
  ownerRef: string;
  tenantRef: string;
  computerRef: string;
  workspaceRef: string;
  sessionRef: string;
  commandRef: string;
  runtimeGeneration: number;
}>;

export type CloudComputerCommandStreamCursor = Readonly<{
  schema: typeof CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA;
  sessionRef: string;
  commandRef: string;
  runtimeGeneration: number;
  sequence: number;
  retentionEpoch: number;
}>;

export type CloudComputerCommandStreamEvent = Readonly<{
  eventRef: string;
  sequence: number;
  eventDigest: string;
  kind: string;
  payload: Readonly<Record<string, unknown>>;
  artifactRefs: ReadonlyArray<string>;
  observedAt: string;
}>;

export type CloudComputerCommandStreamArtifact = Readonly<{
  artifactRef: string;
  kind: "stdout" | "stderr" | "result" | "diagnostic";
  contentDigest: string;
  byteCount: number;
  retainUntil: string;
}>;

export type CloudComputerCommandPublicStatus =
  | "admitted"
  | "not_dispatched"
  | "dispatched"
  | "may_have_started"
  | "running"
  | "completed"
  | "failed"
  | "cancelled"
  | "timed_out"
  | "lost";

export type CloudComputerCommandTerminalSummary = Readonly<{
  terminalRef: string;
  terminalDigest: string;
  outcome: "completed" | "failed" | "cancelled" | "timed_out" | "lost";
  reason: string;
  exitCode: number | null;
  outputDigest: string | null;
  completedAt: string;
}>;

type DurableStreamState = Readonly<{
  scope: CloudComputerCommandStreamScope;
  commandStatus: CloudComputerCommandPublicStatus;
  retentionWatermark: number;
  retentionEpoch: number;
  terminal: CloudComputerCommandTerminalSummary | null;
  artifacts: ReadonlyArray<CloudComputerCommandStreamArtifact>;
}>;

export type CloudComputerCommandStreamPage = DurableStreamState &
  Readonly<{
    schema: typeof CLOUD_COMPUTER_COMMAND_STREAM_PAGE_SCHEMA;
    kind: "page";
    cursor: CloudComputerCommandStreamCursor;
    events: ReadonlyArray<CloudComputerCommandStreamEvent>;
    hasMore: boolean;
  }>;

export type CloudComputerCommandStreamReset = DurableStreamState &
  Readonly<{
    schema: typeof CLOUD_COMPUTER_COMMAND_STREAM_RESET_SCHEMA;
    kind: "stale_cursor";
    reason: "cursor_behind_retention" | "retention_epoch_changed";
    resetCursor: CloudComputerCommandStreamCursor;
  }>;

export type CloudComputerCommandStreamRead =
  | CloudComputerCommandStreamPage
  | CloudComputerCommandStreamReset;

/** The journal is the sole durability authority. */
export type CloudComputerCommandJournalPort = Readonly<{
  read: (input: {
    scope: CloudComputerCommandStreamScope;
    cursor: CloudComputerCommandStreamCursor | null;
    limit: number;
  }) => Promise<CloudComputerCommandStreamRead>;
}>;

/** A Phoenix PubSub bridge implements this port with a topic subscription. */
export type CloudComputerCommandWakeupPort = Readonly<{
  subscribe: (topic: string, wake: () => void) => Promise<() => void | Promise<void>>;
}>;

export type CloudComputerCommandStreamPollPort = Readonly<{
  wait: (signal: AbortSignal) => Promise<void>;
}>;

export class CloudComputerCommandStreamError extends Error {
  constructor(
    readonly code: "invalid_input" | "invalid_journal",
    message: string,
  ) {
    super(message);
    this.name = "CloudComputerCommandStreamError";
  }
}

const assertRef = (value: string, field: string): void => {
  if (!REF.test(value))
    throw new CloudComputerCommandStreamError("invalid_input", `${field} is invalid`);
};

const assertInteger = (value: number, field: string, minimum = 0): void => {
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw new CloudComputerCommandStreamError("invalid_input", `${field} is invalid`);
  }
};

const assertScope = (scope: CloudComputerCommandStreamScope): void => {
  for (const [field, value] of Object.entries({
    ownerRef: scope.ownerRef,
    tenantRef: scope.tenantRef,
    computerRef: scope.computerRef,
    workspaceRef: scope.workspaceRef,
    sessionRef: scope.sessionRef,
    commandRef: scope.commandRef,
  }))
    assertRef(value, field);
  assertInteger(scope.runtimeGeneration, "runtime generation", 1);
};

const sameScope = (
  left: CloudComputerCommandStreamScope,
  right: CloudComputerCommandStreamScope,
): boolean =>
  left.ownerRef === right.ownerRef &&
  left.tenantRef === right.tenantRef &&
  left.computerRef === right.computerRef &&
  left.workspaceRef === right.workspaceRef &&
  left.sessionRef === right.sessionRef &&
  left.commandRef === right.commandRef &&
  left.runtimeGeneration === right.runtimeGeneration;

const assertCursor = (
  cursor: CloudComputerCommandStreamCursor,
  scope: CloudComputerCommandStreamScope,
): void => {
  if (
    cursor.schema !== CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA ||
    cursor.sessionRef !== scope.sessionRef ||
    cursor.commandRef !== scope.commandRef ||
    cursor.runtimeGeneration !== scope.runtimeGeneration
  ) {
    throw new CloudComputerCommandStreamError("invalid_input", "cursor scope differs");
  }
  assertInteger(cursor.sequence, "cursor sequence");
  assertInteger(cursor.retentionEpoch, "cursor retention epoch");
};

const assertTimestamp = (value: string, field: string): void => {
  if (!Number.isFinite(Date.parse(value))) {
    throw new CloudComputerCommandStreamError("invalid_journal", `${field} is invalid`);
  }
};

const assertDurableRead = (
  value: CloudComputerCommandStreamRead,
  scope: CloudComputerCommandStreamScope,
  requested: CloudComputerCommandStreamCursor | null,
): void => {
  if (!sameScope(value.scope, scope)) {
    throw new CloudComputerCommandStreamError("invalid_journal", "journal scope differs");
  }
  if (
    ![
      "admitted",
      "not_dispatched",
      "dispatched",
      "may_have_started",
      "running",
      "completed",
      "failed",
      "cancelled",
      "timed_out",
      "lost",
    ].includes(value.commandStatus)
  ) {
    throw new CloudComputerCommandStreamError("invalid_journal", "command status is invalid");
  }
  assertInteger(value.retentionWatermark, "retention watermark");
  assertInteger(value.retentionEpoch, "retention epoch");
  for (const artifact of value.artifacts) {
    assertRef(artifact.artifactRef, "artifact ref");
    if (!DIGEST.test(artifact.contentDigest)) {
      throw new CloudComputerCommandStreamError("invalid_journal", "artifact digest is invalid");
    }
    assertInteger(artifact.byteCount, "artifact byte count");
    assertTimestamp(artifact.retainUntil, "artifact retention");
  }
  if (value.terminal !== null) {
    assertRef(value.terminal.terminalRef, "terminal ref");
    if (!DIGEST.test(value.terminal.terminalDigest)) {
      throw new CloudComputerCommandStreamError("invalid_journal", "terminal digest is invalid");
    }
    if (value.terminal.outputDigest !== null && !DIGEST.test(value.terminal.outputDigest)) {
      throw new CloudComputerCommandStreamError("invalid_journal", "output digest is invalid");
    }
    assertTimestamp(value.terminal.completedAt, "terminal completion");
  }
  if (value.kind === "stale_cursor") {
    if (value.schema !== CLOUD_COMPUTER_COMMAND_STREAM_RESET_SCHEMA) {
      throw new CloudComputerCommandStreamError("invalid_journal", "reset schema differs");
    }
    assertCursor(value.resetCursor, scope);
    if (
      value.resetCursor.sequence < value.retentionWatermark ||
      value.resetCursor.retentionEpoch !== value.retentionEpoch
    ) {
      throw new CloudComputerCommandStreamError("invalid_journal", "reset cursor differs");
    }
    return;
  }
  if (value.schema !== CLOUD_COMPUTER_COMMAND_STREAM_PAGE_SCHEMA) {
    throw new CloudComputerCommandStreamError("invalid_journal", "page schema differs");
  }
  assertCursor(value.cursor, scope);
  if (
    value.cursor.retentionEpoch !== value.retentionEpoch ||
    value.cursor.sequence < value.retentionWatermark ||
    (requested !== null && value.cursor.sequence < requested.sequence)
  ) {
    throw new CloudComputerCommandStreamError("invalid_journal", "page cursor regressed");
  }
  let previous = requested?.sequence ?? value.retentionWatermark;
  for (const event of value.events) {
    assertRef(event.eventRef, "event ref");
    if (
      !DIGEST.test(event.eventDigest) ||
      event.sequence <= previous ||
      event.sequence > value.cursor.sequence
    ) {
      throw new CloudComputerCommandStreamError(
        "invalid_journal",
        "event sequence or digest differs",
      );
    }
    event.artifactRefs.forEach((ref) => assertRef(ref, "event artifact ref"));
    assertTimestamp(event.observedAt, "event timestamp");
    previous = event.sequence;
  }
};

export const cloudComputerCommandWakeupTopic = (scope: CloudComputerCommandStreamScope): string => {
  assertScope(scope);
  return `cloud-computer:command:${scope.sessionRef}:${scope.commandRef}`;
};

const defaultPoll = (intervalMs: number): CloudComputerCommandStreamPollPort => ({
  wait: (signal) =>
    new Promise((resolve) => {
      if (signal.aborted) {
        resolve();
        return;
      }
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        signal.removeEventListener("abort", finish);
        resolve();
      };
      const timer = setTimeout(finish, intervalMs);
      signal.addEventListener("abort", finish, { once: true });
    }),
});

const fingerprint = (read: CloudComputerCommandStreamPage): string =>
  JSON.stringify({
    commandStatus: read.commandStatus,
    cursor: read.cursor,
    events: read.events.map((event) => [event.eventRef, event.eventDigest]),
    artifacts: read.artifacts.map((artifact) => [artifact.artifactRef, artifact.contentDigest]),
    terminal: read.terminal,
    retentionWatermark: read.retentionWatermark,
    retentionEpoch: read.retentionEpoch,
  });

export const createCloudComputerCommandStream = (input: {
  journal: CloudComputerCommandJournalPort;
  wakeups: CloudComputerCommandWakeupPort;
  poll?: CloudComputerCommandStreamPollPort;
  pollIntervalMs?: number;
}) => {
  const interval = input.pollIntervalMs ?? 15_000;
  assertInteger(interval, "poll interval", 1);
  const poll = input.poll ?? defaultPoll(interval);

  return {
    open: async function* (request: {
      scope: CloudComputerCommandStreamScope;
      cursor: CloudComputerCommandStreamCursor | null;
      pageSize?: number;
      signal?: AbortSignal;
    }): AsyncGenerator<CloudComputerCommandStreamRead, void, void> {
      assertScope(request.scope);
      if (request.cursor !== null) assertCursor(request.cursor, request.scope);
      const pageSize = request.pageSize ?? 100;
      assertInteger(pageSize, "page size", 1);
      if (pageSize > 1_000) {
        throw new CloudComputerCommandStreamError("invalid_input", "page size is invalid");
      }

      const topic = cloudComputerCommandWakeupTopic(request.scope);
      let cursor = request.cursor;
      let pending: CloudComputerCommandStreamRead | undefined;
      let priorFingerprint: string | undefined;

      while (!request.signal?.aborted) {
        let first = pending;
        if (first === undefined) {
          // eslint-disable-next-line no-await-in-loop -- journal reads must remain cursor-ordered.
          first = await input.journal.read({ scope: request.scope, cursor, limit: pageSize });
        }
        pending = undefined;
        assertDurableRead(first, request.scope, cursor);
        if (first.kind === "stale_cursor") {
          yield first;
          return;
        }
        cursor = first.cursor;
        const currentFingerprint = fingerprint(first);
        if (currentFingerprint !== priorFingerprint) {
          priorFingerprint = currentFingerprint;
          yield first;
        }
        if (first.terminal !== null) return;
        if (first.hasMore) continue;

        let wake: (() => void) | undefined;
        const wakePromise = new Promise<void>((resolve) => {
          wake = resolve;
        });
        // eslint-disable-next-line no-await-in-loop -- install the wakeup hint before the race-closing read.
        const unsubscribe = await input.wakeups.subscribe(topic, () => wake?.());
        try {
          // eslint-disable-next-line no-await-in-loop -- this second read closes the subscription race.
          const second = await input.journal.read({
            scope: request.scope,
            cursor,
            limit: pageSize,
          });
          assertDurableRead(second, request.scope, cursor);
          if (second.kind === "stale_cursor" || fingerprint(second) !== priorFingerprint) {
            pending = second;
            continue;
          }
          const waitController = new AbortController();
          const abort = () => waitController.abort();
          request.signal?.addEventListener("abort", abort, { once: true });
          try {
            // eslint-disable-next-line no-await-in-loop -- each wake or poll begins a new durable read cycle.
            await Promise.race([wakePromise, poll.wait(waitController.signal)]);
          } finally {
            request.signal?.removeEventListener("abort", abort);
            waitController.abort();
          }
        } finally {
          // eslint-disable-next-line no-await-in-loop -- every cycle must release its exact subscription.
          await unsubscribe();
        }
      }
    },
  };
};
