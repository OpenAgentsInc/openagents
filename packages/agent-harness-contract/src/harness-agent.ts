import { Cause, Deferred, Effect, Queue, Stream } from "effect";
import type {
  KhalaRuntimeFinishReason,
  KhalaRuntimeSource,
  KhalaRuntimeUsage,
} from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness, HarnessStartError, HarnessStartOptions } from "./adapter.ts";
import type { HarnessCapabilityUnsupported } from "./capability.ts";
import type { HarnessHostToolSpec } from "./host-tool.ts";
import type { HarnessContinuationState, HarnessResumeState } from "./lifecycle-state.ts";
import type { HarnessBuiltinToolFiltering, HarnessPermissionMode } from "./permission.ts";
import type {
  HarnessContinueTurnOptions,
  HarnessPromptControl,
  HarnessPromptTurnOptions,
  HarnessSession,
  HarnessTurnResult,
} from "./session.ts";
import { HarnessTurnError } from "./session.ts";
import type { HarnessSkill } from "./skill.ts";
import {
  khalaEventToUiChunks,
  type KhalaEventToUiChunksOptions,
  type UiMessageChunk,
} from "./ui-message-chunk.ts";
import {
  applyUiChunk,
  initialUiMessage,
  UiMessageReducerError,
  type UiMessage,
} from "./ui-message-reducer.ts";

/**
 * HARN-07 — a thin `HarnessAgent`-style facade over any {@link AgentHarness}.
 *
 * The facade re-derives the Vercel `HarnessAgent` ergonomics (generate/stream
 * returning a coalesced result plus a live chunk stream, and session
 * create/resume helpers) WITHOUT adding a second runtime. The session verbs of
 * the contract (`promptTurn`, `continueTurn`, `suspendTurn`, `compact`,
 * `detach`, `stop`, `destroy`) stay underneath unchanged: this layer only
 * assembles the neutral `KhalaRuntimeEvent` stream into a coalesced value
 * through the EXISTING UI projection (`khalaEventToUiChunks` + `applyUiChunk`,
 * STREAM-02) and passes skills/host-tools/permission settings straight through
 * to the adapter.
 *
 * It adds NO authority and NO new semantics. In particular it never papers
 * over a capability gap: a verb the adapter cannot satisfy still surfaces
 * {@link HarnessCapabilityUnsupported} in the caller's error channel.
 */

/** Error channel of a driven turn: a turn failure or a malformed chunk sequence. */
export type HarnessAgentStreamError = HarnessTurnError | UiMessageReducerError;

/**
 * The coalesced final value of one turn, assembled from the neutral
 * `KhalaRuntimeEvent` stream through the existing projection. `message` is the
 * fully-folded {@link UiMessage} snapshot; `text` is its concatenated text
 * parts; `finishReason`/`usage`/`lastCursor` come from the turn's own
 * {@link HarnessTurnResult} summary.
 */
export interface HarnessAgentResult {
  readonly turnId: string;
  readonly message: UiMessage;
  readonly text: string;
  readonly finishReason: KhalaRuntimeFinishReason;
  readonly usage?: KhalaRuntimeUsage;
  readonly lastCursor: number;
}

/**
 * The streaming return: the live `UiMessageChunk` stream a renderer consumes,
 * plus a `result` effect that resolves to the coalesced value once the turn
 * ends. Both are fed by ONE consumption of the underlying turn stream (a
 * Queue/Deferred tee), so a live once-only adapter stream stays correct.
 */
export interface HarnessAgentStreamResult {
  readonly stream: Stream.Stream<UiMessageChunk, HarnessAgentStreamError>;
  readonly result: Effect.Effect<HarnessAgentResult, HarnessAgentStreamError>;
}

/** Agent-level defaults, applied to every session/turn unless overridden. */
export interface HarnessAgentOptions {
  /** Event source labelling for started sessions (required by `start`). */
  readonly source: KhalaRuntimeSource;
  /** Skills passed through to `start` for every session. */
  readonly skills?: ReadonlyArray<HarnessSkill>;
  /** Built-in tool approval policy passed through to `start`. */
  readonly permissionMode?: HarnessPermissionMode;
  /** Built-in tool filtering passed through to `start`. */
  readonly builtinToolFiltering?: HarnessBuiltinToolFiltering;
  /** Default session instructions passed through to `promptTurn`. */
  readonly instructions?: string;
  /** Default host tools passed through to `promptTurn`. */
  readonly tools?: ReadonlyArray<HarnessHostToolSpec>;
  /** Projection options for {@link khalaEventToUiChunks}. */
  readonly projection?: KhalaEventToUiChunksOptions;
  /** Id factory for auto-generated session/turn ids. */
  readonly generateId?: () => string;
}

/**
 * A turn input. A bare string is the prompt; the object form overrides the
 * per-turn id, instructions, host tools, and (for one-shot `generate`/`stream`)
 * the session id.
 */
export type HarnessAgentPromptInput =
  | string
  | {
      readonly prompt: string;
      readonly turnId?: string;
      readonly instructions?: string;
      readonly tools?: ReadonlyArray<HarnessHostToolSpec>;
      readonly sessionId?: string;
    };

/** Per-session overrides for {@link HarnessAgent.createSession}. */
export interface HarnessAgentSessionOptions {
  readonly sessionId?: string;
  readonly source?: KhalaRuntimeSource;
  readonly skills?: ReadonlyArray<HarnessSkill>;
  readonly permissionMode?: HarnessPermissionMode;
  readonly builtinToolFiltering?: HarnessBuiltinToolFiltering;
  /** Resume a parked session (passed to `start({ resumeFrom })`). */
  readonly resumeFrom?: HarnessResumeState;
}

/**
 * A live session wrapper. `generate`/`stream` drive a fresh prompt turn;
 * `resumeGenerate`/`resumeStream` drive `continueTurn` (slice continuation).
 * `session` is the raw {@link HarnessSession} for the lifecycle verbs
 * (`suspendTurn`, `compact`, `detach`, `stop`, `destroy`) — exposed as a
 * passthrough, never wrapped, so no authority or semantics are added.
 */
export interface HarnessAgentSession {
  readonly session: HarnessSession;
  readonly sessionId: string;
  readonly generate: (
    input: HarnessAgentPromptInput,
  ) => Effect.Effect<HarnessAgentResult, HarnessTurnError | HarnessAgentStreamError>;
  readonly stream: (
    input: HarnessAgentPromptInput,
  ) => Effect.Effect<HarnessAgentStreamResult, HarnessTurnError>;
  readonly resumeGenerate: (
    options?: HarnessContinueTurnOptions,
  ) => Effect.Effect<
    HarnessAgentResult,
    HarnessTurnError | HarnessCapabilityUnsupported | HarnessAgentStreamError
  >;
  readonly resumeStream: (
    options?: HarnessContinueTurnOptions,
  ) => Effect.Effect<HarnessAgentStreamResult, HarnessTurnError | HarnessCapabilityUnsupported>;
}

/** The facade over one {@link AgentHarness}. */
export interface HarnessAgent {
  readonly adapter: AgentHarness;
  /** One-shot: create a fresh session, run one prompt turn, coalesce. */
  readonly generate: (
    input: HarnessAgentPromptInput,
  ) => Effect.Effect<
    HarnessAgentResult,
    HarnessStartError | HarnessTurnError | HarnessAgentStreamError
  >;
  /** One-shot: create a fresh session, run one prompt turn, tee the stream. */
  readonly stream: (
    input: HarnessAgentPromptInput,
  ) => Effect.Effect<HarnessAgentStreamResult, HarnessStartError | HarnessTurnError>;
  /** Create an explicit multi-turn session over `start`. */
  readonly createSession: (
    options?: HarnessAgentSessionOptions,
  ) => Effect.Effect<HarnessAgentSession, HarnessStartError>;
  /** Resume a suspended turn: `start({ continueFrom })`, then `continueTurn`. */
  readonly resumeSession: (
    continuation: HarnessContinuationState,
    options?: HarnessAgentSessionOptions,
  ) => Effect.Effect<HarnessAgentSession, HarnessStartError>;
}

const normalizeInput = (
  input: HarnessAgentPromptInput,
): {
  readonly prompt: string;
  readonly turnId?: string;
  readonly instructions?: string;
  readonly tools?: ReadonlyArray<HarnessHostToolSpec>;
  readonly sessionId?: string;
} => (typeof input === "string" ? { prompt: input } : input);

/** Concatenate the message's text parts in order — the coalesced text. */
const textOf = (message: UiMessage): string =>
  message.parts.reduce((acc, part) => (part.type === "text" ? acc + part.text : acc), "");

const coalesce = (message: UiMessage, turn: HarnessTurnResult): HarnessAgentResult => ({
  turnId: turn.turnId,
  message,
  text: textOf(message),
  finishReason: turn.finishReason,
  ...(turn.usage === undefined ? {} : { usage: turn.usage }),
  lastCursor: turn.lastCursor,
});

/** Project the neutral turn event stream onto the existing UI chunk vocabulary. */
const projectControl = (
  control: HarnessPromptControl,
  projection: KhalaEventToUiChunksOptions | undefined,
): Stream.Stream<UiMessageChunk, HarnessTurnError> =>
  control.events.pipe(
    Stream.flatMap((event) => Stream.fromIterable(khalaEventToUiChunks(event, projection))),
  );

/** Apply one chunk through the existing reducer, mapping throws to the tagged error. */
const applyChunkEffect = (
  message: UiMessage,
  chunk: UiMessageChunk,
): Effect.Effect<UiMessage, UiMessageReducerError> =>
  Effect.try({
    try: () => applyUiChunk(message, chunk),
    catch: (cause) =>
      cause instanceof UiMessageReducerError
        ? cause
        : new UiMessageReducerError({
            chunkType: chunk.type,
            detail: `unexpected reducer failure: ${String(cause)}`,
          }),
  });

/** Drive a turn to its coalesced result by folding the whole projected stream. */
const driveGenerate = (
  control: HarnessPromptControl,
  projection: KhalaEventToUiChunksOptions | undefined,
): Effect.Effect<HarnessAgentResult, HarnessAgentStreamError> =>
  Effect.gen(function* () {
    const message = yield* projectControl(control, projection).pipe(
      Stream.runFoldEffect(
        () => initialUiMessage(),
        (msg, chunk) => applyChunkEffect(msg, chunk),
      ),
    );
    const turn = yield* control.done;
    return coalesce(message, turn);
  });

/**
 * Tee the projected chunk stream into a live queue (for the renderer) and a
 * folded coalesced result (for `result`), consuming the underlying turn stream
 * exactly once. The producer is forked as a child of the caller so it lives
 * with the caller's fiber; interruption cancels the turn.
 */
const driveStream = (
  control: HarnessPromptControl,
  projection: KhalaEventToUiChunksOptions | undefined,
): Effect.Effect<HarnessAgentStreamResult> =>
  Effect.gen(function* () {
    const queue = yield* Queue.unbounded<UiMessageChunk, HarnessAgentStreamError | Cause.Done>();
    const deferred = yield* Deferred.make<HarnessAgentResult, HarnessAgentStreamError>();

    const producer = projectControl(control, projection).pipe(
      Stream.runFoldEffect(
        () => initialUiMessage(),
        (msg, chunk) =>
          Queue.offer(queue, chunk).pipe(Effect.flatMap(() => applyChunkEffect(msg, chunk))),
      ),
      Effect.flatMap((message) => control.done.pipe(Effect.map((turn) => coalesce(message, turn)))),
      Effect.flatMap((result) =>
        Deferred.succeed(deferred, result).pipe(Effect.flatMap(() => Queue.end(queue))),
      ),
      Effect.catch((error) =>
        Deferred.fail(deferred, error).pipe(Effect.flatMap(() => Queue.fail(queue, error))),
      ),
    );

    yield* Effect.forkChild(producer);

    return {
      stream: Stream.fromQueue(queue),
      result: Deferred.await(deferred),
    };
  });

/**
 * Build a {@link HarnessAgent} over any {@link AgentHarness}. The adapter keeps
 * its full contract; the facade only assembles results and threads settings
 * through.
 */
export const makeHarnessAgent = (
  adapter: AgentHarness,
  options: HarnessAgentOptions,
): HarnessAgent => {
  const projection = options.projection;
  let counter = 0;
  const generateId = options.generateId ?? (() => `${adapter.harnessId}-${++counter}`);

  const buildStartOptions = (
    sessionId: string,
    so: HarnessAgentSessionOptions | undefined,
    lifecycle: {
      readonly resumeFrom?: HarnessResumeState;
      readonly continueFrom?: HarnessContinuationState;
    },
  ): HarnessStartOptions => {
    const skills = so?.skills ?? options.skills;
    const permissionMode = so?.permissionMode ?? options.permissionMode;
    const builtinToolFiltering = so?.builtinToolFiltering ?? options.builtinToolFiltering;
    return {
      sessionId,
      source: so?.source ?? options.source,
      ...(skills === undefined ? {} : { skills }),
      ...(permissionMode === undefined ? {} : { permissionMode }),
      ...(builtinToolFiltering === undefined ? {} : { builtinToolFiltering }),
      ...(lifecycle.resumeFrom === undefined ? {} : { resumeFrom: lifecycle.resumeFrom }),
      ...(lifecycle.continueFrom === undefined ? {} : { continueFrom: lifecycle.continueFrom }),
    };
  };

  const buildPromptTurnOptions = (
    input: HarnessAgentPromptInput,
    turnId: string,
  ): HarnessPromptTurnOptions => {
    const norm = normalizeInput(input);
    const instructions = norm.instructions ?? options.instructions;
    const tools = norm.tools ?? options.tools;
    return {
      turnId,
      prompt: norm.prompt,
      ...(instructions === undefined ? {} : { instructions }),
      ...(tools === undefined ? {} : { tools }),
    };
  };

  const makeAgentSession = (session: HarnessSession): HarnessAgentSession => ({
    session,
    sessionId: session.sessionId,
    generate: (input) =>
      Effect.gen(function* () {
        const turnId = normalizeInput(input).turnId ?? generateId();
        const control = yield* session.promptTurn(buildPromptTurnOptions(input, turnId));
        return yield* driveGenerate(control, projection);
      }),
    stream: (input) =>
      Effect.gen(function* () {
        const turnId = normalizeInput(input).turnId ?? generateId();
        const control = yield* session.promptTurn(buildPromptTurnOptions(input, turnId));
        return yield* driveStream(control, projection);
      }),
    resumeGenerate: (continueOptions) =>
      Effect.gen(function* () {
        const control = yield* session.continueTurn(continueOptions ?? {});
        return yield* driveGenerate(control, projection);
      }),
    resumeStream: (continueOptions) =>
      Effect.gen(function* () {
        const control = yield* session.continueTurn(continueOptions ?? {});
        return yield* driveStream(control, projection);
      }),
  });

  return {
    adapter,
    generate: (input) =>
      Effect.gen(function* () {
        const norm = normalizeInput(input);
        const sessionId = norm.sessionId ?? generateId();
        const session = yield* adapter.start(buildStartOptions(sessionId, undefined, {}));
        const turnId = norm.turnId ?? generateId();
        const control = yield* session.promptTurn(buildPromptTurnOptions(input, turnId));
        return yield* driveGenerate(control, projection);
      }),
    stream: (input) =>
      Effect.gen(function* () {
        const norm = normalizeInput(input);
        const sessionId = norm.sessionId ?? generateId();
        const session = yield* adapter.start(buildStartOptions(sessionId, undefined, {}));
        const turnId = norm.turnId ?? generateId();
        const control = yield* session.promptTurn(buildPromptTurnOptions(input, turnId));
        return yield* driveStream(control, projection);
      }),
    createSession: (so) =>
      Effect.gen(function* () {
        const sessionId = so?.sessionId ?? generateId();
        const session = yield* adapter.start(
          buildStartOptions(
            sessionId,
            so,
            so?.resumeFrom === undefined ? {} : { resumeFrom: so.resumeFrom },
          ),
        );
        return makeAgentSession(session);
      }),
    resumeSession: (continuation, so) =>
      Effect.gen(function* () {
        const sessionId = so?.sessionId ?? continuation.sessionId;
        const session = yield* adapter.start(
          buildStartOptions(sessionId, so, { continueFrom: continuation }),
        );
        return makeAgentSession(session);
      }),
  };
};
