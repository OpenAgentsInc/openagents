import {
  buildTextDelta,
  buildTurnFinished,
  buildTurnStarted,
  HarnessCapabilityUnsupported,
  HarnessStartError,
  HarnessTurnError,
  type HarnessPromptControl,
  type HarnessPromptTurnOptions,
  type HarnessResumeState,
  type HarnessSession,
  type HarnessStartOptions,
  type HarnessStreamEvent,
  type HarnessTurnResult,
} from "@openagentsinc/agent-harness-contract";
import type { KhalaRuntimeFinishReason, KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import { Effect, Stream } from "effect";

import {
  HarnessEnvironmentError,
  openAgentsCloudCodingSessionLaunchUrl,
  type HarnessEnvironmentRunner,
  type HarnessEnvironmentStartInput,
  type OpenAgentsCloudHarnessEnvironment,
} from "./contract.js";
import {
  isTerminalCloudCodingSessionState,
  makeCloudCodingSessionClient,
  type CloudCodingAdapter,
  type CloudCodingLane,
  type CloudCodingSessionClient,
  type CloudCodingSessionHttpError,
  type CloudCodingSessionProjection,
  type CloudCodingSessionState,
  type FetchLike,
  type RepoTrustTier,
} from "./openagents-cloud-client.js";

const DEFAULT_HARNESS_ID = "openagents-cloud";
const DEFAULT_POLL_INTERVAL_MS = 1_000;
const DEFAULT_MAX_POLL_ATTEMPTS = 120;

export interface OpenAgentsCloudRunnerConfig {
  /**
   * Bearer token for the Worker cloud-coding-sessions surface.
   * Never stored on {@link OpenAgentsCloudHarnessEnvironment}.
   */
  readonly bearerToken: string;
  /** Public-safe repository ref forwarded as `repoRef` on launch. */
  readonly repoRef: string;
  readonly lane?: CloudCodingLane;
  readonly adapter?: CloudCodingAdapter;
  readonly repoTrustTier?: RepoTrustTier;
  readonly verify?: ReadonlyArray<string>;
  readonly timeoutSeconds?: number;
  readonly workContextRef?: string;
  readonly threadRef?: string;
  readonly repoBindingRef?: string;
  /** Injected fetch for tests. Defaults to `globalThis.fetch`. */
  readonly fetch?: FetchLike;
  /** Poll spacing while waiting for a terminal session state. */
  readonly pollIntervalMs?: number;
  /** Maximum lifecycle GETs after launch before `poll_timeout`. */
  readonly maxPollAttempts?: number;
  /** Optional harness id used in typed start/turn errors. */
  readonly harnessId?: string;
  /** Injected sleep for tests. Defaults to `Effect.sleep`. */
  readonly sleep?: (durationMs: number) => Effect.Effect<void>;
}

const finishReasonForState = (
  state: CloudCodingSessionState,
): KhalaRuntimeFinishReason => {
  if (state === "completed") return "stop";
  if (state === "cancelled") return "cancelled";
  if (state === "failed") return "error";
  return "unknown";
};

const publicStatusText = (session: CloudCodingSessionProjection): string => {
  const parts = [
    `cloud coding session ${session.id}`,
    `state=${session.state}`,
  ];
  if (session.agent_computer_state !== undefined) {
    parts.push(`agent_computer_state=${session.agent_computer_state}`);
  }
  if (session.placement_ref) {
    parts.push(`placement_ref=${session.placement_ref}`);
  }
  if (session.artifact_ref) {
    parts.push(`artifact_ref=${session.artifact_ref}`);
  }
  return parts.join("; ");
};

const toTurnError = (input: {
  readonly harnessId: string;
  readonly sessionId: string;
  readonly turnId: string;
  readonly error: CloudCodingSessionHttpError;
}): HarnessTurnError =>
  new HarnessTurnError({
    harnessId: input.harnessId,
    sessionId: input.sessionId,
    turnId: input.turnId,
    failureClass: input.error.failureClass,
    ...(input.error.detail === undefined ? {} : { detail: input.error.detail }),
    ...(input.error.cause === undefined ? {} : { cause: input.error.cause }),
  });

const makeUnsupported = (
  harnessId: string,
  capability: "continue_turn" | "suspend_turn" | "compact" | "detach",
) =>
  new HarnessCapabilityUnsupported({
    harnessId,
    capability,
    detail: `openagents_cloud runner does not support ${capability}.`,
  });

const makePromptControl = (input: {
  readonly harnessId: string;
  readonly sessionId: string;
  readonly turnId: string;
  readonly events: ReadonlyArray<HarnessStreamEvent>;
  readonly result: HarnessTurnResult;
}): HarnessPromptControl => ({
  turnId: input.turnId,
  events: Stream.fromIterable(input.events),
  done: Effect.succeed(input.result),
  submitToolResult: () =>
    Effect.fail(
      new HarnessTurnError({
        harnessId: input.harnessId,
        sessionId: input.sessionId,
        turnId: input.turnId,
        failureClass: "no_active_tool_call",
        detail: "openagents_cloud turns do not accept host tool results.",
      }),
    ),
  submitToolApproval: () =>
    Effect.fail(
      new HarnessTurnError({
        harnessId: input.harnessId,
        sessionId: input.sessionId,
        turnId: input.turnId,
        failureClass: "no_active_tool_call",
        detail: "openagents_cloud turns do not accept tool approvals.",
      }),
    ),
  submitUserMessage: () => Effect.void,
  interrupt: () => Effect.void,
});

const pollUntilTerminal = Effect.fn("OpenAgentsCloudRunner.pollUntilTerminal")(
  function* (input: {
    readonly client: CloudCodingSessionClient;
    readonly sessionId: string;
    readonly pollIntervalMs: number;
    readonly maxPollAttempts: number;
    readonly sleep: (durationMs: number) => Effect.Effect<void>;
  }) {
    let latest: CloudCodingSessionProjection | undefined;
    for (let attempt = 0; attempt < input.maxPollAttempts; attempt += 1) {
      latest = yield* input.client.get(input.sessionId);
      if (isTerminalCloudCodingSessionState(latest.state)) {
        return latest;
      }
      yield* input.sleep(input.pollIntervalMs);
    }
    return yield* new HarnessTurnError({
      harnessId: DEFAULT_HARNESS_ID,
      sessionId: input.sessionId,
      turnId: "unknown",
      failureClass: "poll_timeout",
      detail: `Cloud coding session ${input.sessionId} did not reach a terminal state after ${String(input.maxPollAttempts)} polls.`,
    });
  },
);

const buildTurnEvents = (input: {
  readonly turnId: string;
  readonly threadId: string;
  readonly source: KhalaRuntimeSource;
  readonly launched: CloudCodingSessionProjection;
  readonly finished: CloudCodingSessionProjection;
}): {
  readonly events: ReadonlyArray<HarnessStreamEvent>;
  readonly result: HarnessTurnResult;
} => {
  const events: Array<HarnessStreamEvent> = [];
  let sequence = 0;
  events.push(
    buildTurnStarted({
      turnId: input.turnId,
      threadId: input.threadId,
      sequence,
      source: input.source,
    }),
  );
  sequence += 1;
  const messageId = `msg.${input.turnId}`;
  events.push(
    buildTextDelta({
      turnId: input.turnId,
      threadId: input.threadId,
      sequence,
      source: input.source,
      messageId,
      text: `Launched ${publicStatusText(input.launched)}.`,
    }),
  );
  sequence += 1;
  if (input.finished.id !== input.launched.id || input.finished.state !== input.launched.state) {
    events.push(
      buildTextDelta({
        turnId: input.turnId,
        threadId: input.threadId,
        sequence,
        source: input.source,
        messageId,
        text: `Finished ${publicStatusText(input.finished)}.`,
      }),
    );
    sequence += 1;
  }
  const finishReason = finishReasonForState(input.finished.state);
  events.push(
    buildTurnFinished({
      turnId: input.turnId,
      threadId: input.threadId,
      sequence,
      source: input.source,
      finishReason,
    }),
  );
  return {
    events,
    result: {
      turnId: input.turnId,
      finishReason,
      lastCursor: sequence,
    },
  };
};

const makeSession = (input: {
  readonly harnessId: string;
  readonly options: HarnessStartOptions;
  readonly config: OpenAgentsCloudRunnerConfig;
  readonly client: CloudCodingSessionClient;
}): HarnessSession => {
  const { harnessId, options, config, client } = input;
  const sessionId = options.sessionId;
  const source = options.source;
  const pollIntervalMs = config.pollIntervalMs ?? DEFAULT_POLL_INTERVAL_MS;
  const maxPollAttempts = config.maxPollAttempts ?? DEFAULT_MAX_POLL_ATTEMPTS;
  const sleep =
    config.sleep ?? ((durationMs: number) => Effect.sleep(durationMs));

  const promptTurn = (
    turn: HarnessPromptTurnOptions,
  ): Effect.Effect<HarnessPromptControl, HarnessTurnError> =>
    Effect.gen(function* () {
      const objective = turn.prompt.trim();
      if (objective === "") {
        return yield* new HarnessTurnError({
          harnessId,
          sessionId,
          turnId: turn.turnId,
          failureClass: "invalid_prompt",
          detail: "openagents_cloud promptTurn requires a non-empty prompt as objective.",
        });
      }

      const launched = yield* client
        .launch({
          repoRef: config.repoRef,
          objective,
          ...(config.lane === undefined ? {} : { lane: config.lane }),
          ...(config.adapter === undefined ? {} : { adapter: config.adapter }),
          ...(config.repoTrustTier === undefined
            ? {}
            : { repoTrustTier: config.repoTrustTier }),
          ...(config.verify === undefined ? {} : { verify: [...config.verify] }),
          ...(config.timeoutSeconds === undefined
            ? {}
            : { timeoutSeconds: config.timeoutSeconds }),
          ...(config.workContextRef === undefined
            ? {}
            : { workContextRef: config.workContextRef }),
          ...(config.threadRef === undefined ? {} : { threadRef: config.threadRef }),
          ...(config.repoBindingRef === undefined
            ? {}
            : { repoBindingRef: config.repoBindingRef }),
        })
        .pipe(
          Effect.mapError((error) =>
            toTurnError({
              harnessId,
              sessionId,
              turnId: turn.turnId,
              error,
            }),
          ),
        );

      const finished = isTerminalCloudCodingSessionState(launched.state)
        ? launched
        : yield* pollUntilTerminal({
            client,
            sessionId: launched.id,
            pollIntervalMs,
            maxPollAttempts,
            sleep,
          }).pipe(
            Effect.mapError((error) => {
              if (error instanceof HarnessTurnError) {
                return new HarnessTurnError({
                  harnessId,
                  sessionId,
                  turnId: turn.turnId,
                  failureClass: error.failureClass,
                  ...(error.detail === undefined ? {} : { detail: error.detail }),
                  ...(error.cause === undefined ? {} : { cause: error.cause }),
                });
              }
              return toTurnError({
                harnessId,
                sessionId,
                turnId: turn.turnId,
                error,
              });
            }),
          );

      const { events, result } = buildTurnEvents({
        turnId: turn.turnId,
        threadId: sessionId,
        source,
        launched,
        finished,
      });
      return makePromptControl({
        harnessId,
        sessionId,
        turnId: turn.turnId,
        events,
        result,
      });
    });

  const stop = (): Effect.Effect<HarnessResumeState> =>
    Effect.succeed({
      harnessId,
      sessionId,
      data: { environment: "openagents_cloud" },
    });

  return {
    sessionId,
    isResume: options.resumeFrom !== undefined || options.continueFrom !== undefined,
    modelId: `openagents_cloud/${config.adapter ?? "codex"}`,
    promptTurn,
    continueTurn: () => Effect.fail(makeUnsupported(harnessId, "continue_turn")),
    suspendTurn: () => Effect.fail(makeUnsupported(harnessId, "suspend_turn")),
    compact: () => Effect.fail(makeUnsupported(harnessId, "compact")),
    detach: () => Effect.fail(makeUnsupported(harnessId, "detach")),
    stop,
    destroy: () => Effect.void,
  };
};

/**
 * Build the production `openagents_cloud` {@link HarnessEnvironmentRunner}.
 *
 * The runner launches managed Agent Computer sessions through the existing
 * Worker `POST /v1/cloud-coding-sessions` surface and maps lifecycle state onto
 * the neutral harness turn stream. Desktop `desktop_local` is unchanged.
 */
export const makeOpenAgentsCloudHarnessEnvironmentRunner = (
  config: OpenAgentsCloudRunnerConfig,
): HarnessEnvironmentRunner<OpenAgentsCloudHarnessEnvironment> => {
  const harnessId = config.harnessId ?? DEFAULT_HARNESS_ID;

  const start = (
    input: HarnessEnvironmentStartInput<OpenAgentsCloudHarnessEnvironment>,
  ): Effect.Effect<HarnessSession, HarnessStartError | HarnessEnvironmentError> =>
    Effect.gen(function* () {
      const bearerToken = config.bearerToken.trim();
      const repoRef = config.repoRef.trim();
      if (bearerToken === "") {
        return yield* new HarnessEnvironmentError({
          environment: "openagents_cloud",
          failureClass: "missing_bearer_token",
          detail: "openagents_cloud runner requires a non-empty bearerToken.",
        });
      }
      if (repoRef === "") {
        return yield* new HarnessEnvironmentError({
          environment: "openagents_cloud",
          failureClass: "missing_repo_ref",
          detail: "openagents_cloud runner requires a non-empty repoRef.",
        });
      }
      if (input.environment._tag !== "openagents_cloud") {
        return yield* new HarnessEnvironmentError({
          environment: "openagents_cloud",
          failureClass: "environment_mismatch",
          detail: `Expected openagents_cloud, received ${input.environment._tag}.`,
        });
      }

      const client = makeCloudCodingSessionClient({
        launchUrl: openAgentsCloudCodingSessionLaunchUrl(input.environment),
        bearerToken,
        ...(config.fetch === undefined ? {} : { fetch: config.fetch }),
      });

      return makeSession({
        harnessId,
        options: input.options,
        config: { ...config, bearerToken, repoRef },
        client,
      });
    });

  return {
    environment: "openagents_cloud",
    start,
  };
};
