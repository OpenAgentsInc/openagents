/**
 * The operator fleet promotion client.
 *
 * It speaks only the operator API from OpenAgentsInc/openagents.com#57 —
 * `POST/GET /api/v1/admin/forge/targets` — behind the same `/api/v1` error
 * envelope every other command family reads. It never touches `/admin/forge`,
 * SSH, or any internal RPC, and it adds only what a terminal caller cannot do
 * for itself: an idempotent re-send after a failed transport, and bounded
 * polling of the status resource to a terminal state.
 */

import { Clock, Duration, Effect, Layer } from "effect";
import * as Context from "effect/Context";

import { ApiTransport } from "./api-transport.js";
import { FLEET_TARGETS_PATH } from "./constants.js";
import { ApiError, DeploymentWaitTimeout, type CliError } from "./errors.js";
import type { AuthenticatedApi } from "./repository-client.js";
import { asRecord, asText, makeTrackerRequest, trackerErrorDetails } from "./tracker-request.js";

/** The one route family from OpenAgentsInc/openagents.com#57. */
export { FLEET_TARGETS_PATH };

/** The privileged scope the server requires; `forge:write` cannot promote. */
export const OPERATOR_SCOPE = "deployments:promote";

/**
 * The states polling stops on. The server marks `live`, `failed`, and
 * `reverted` terminal; `needs_rolling_replace` additionally ends automatic
 * execution and waits on an operator, so a poll that reached it would
 * otherwise never return.
 */
export const TERMINAL_STATES: ReadonlyArray<string> = [
  "live",
  "failed",
  "reverted",
  "needs_rolling_replace",
];

/** How many times a promotion is re-sent after a failed transport. */
export const PROMOTE_TRANSPORT_RETRIES = 2;

const retryDelayMs = 500;

/** Bounded backoff: 2s, 4s, 8s, then every 10s until the deadline. */
export const POLL_BASE_DELAY_MS = 2_000;
export const POLL_MAXIMUM_DELAY_MS = 10_000;

export interface FleetPromoteInput extends AuthenticatedApi {
  readonly repo: string;
  readonly sha: string;
  readonly environment: string;
  /** Generated once by the caller and reused across automatic retries. */
  readonly idempotencyKey: string;
  readonly expectedCurrentTargetId?: string;
}

export interface FleetPromoteResult {
  /** True when the server answered `202 Accepted` with a new target. */
  readonly accepted: boolean;
  /** True when the idempotency key replayed an existing identical promotion. */
  readonly replayed: boolean;
  readonly target: Record<string, unknown>;
}

export interface FleetTargetInput extends AuthenticatedApi {
  readonly id: string;
}

export interface FleetWaitInput extends FleetTargetInput {
  readonly timeoutMs: number;
  readonly baseDelayMs?: number;
  readonly maximumDelayMs?: number;
}

export interface FleetListInput extends AuthenticatedApi {
  readonly repo?: string;
  readonly limit?: number;
}

interface FleetClientInterface {
  readonly promote: (input: FleetPromoteInput) => Effect.Effect<FleetPromoteResult, CliError>;
  readonly view: (input: FleetTargetInput) => Effect.Effect<Record<string, unknown>, CliError>;
  readonly list: (input: FleetListInput) => Effect.Effect<unknown, CliError>;
  readonly wait: (input: FleetWaitInput) => Effect.Effect<Record<string, unknown>, CliError>;
}

export class FleetClient extends Context.Service<FleetClient, FleetClientInterface>()(
  "@openagentsinc/cli/FleetClient",
) {}

/** Reads the lifecycle state off a target body. */
export const targetStatus = (target: Record<string, unknown>): string =>
  asText(target["status"]) ?? "unknown";

/** Whether polling has nothing further to learn about this target. */
export const terminalStatus = (status: string): boolean => TERMINAL_STATES.includes(status);

export const fleetClientLayer = Layer.effect(
  FleetClient,
  Effect.gen(function* () {
    const transport = yield* ApiTransport;
    const request = makeTrackerRequest(transport);

    const view = Effect.fn("FleetClient.view")(function* (input: FleetTargetInput) {
      const body = yield* request("read a fleet target", {
        origin: input.origin,
        token: input.token,
        method: "GET",
        path: `${FLEET_TARGETS_PATH}/${encodeURIComponent(input.id)}`,
        acceptedStatuses: [200],
      });
      return asRecord(body);
    });

    // `202` admits a new target; `200` replays the identical promotion the
    // same key already named. The distinction is the answer to "did I just
    // deploy, or had I already?", so the status must survive translation and
    // the shared accepted-status helper cannot carry it.
    const promoteAttempt = Effect.fn("FleetClient.promoteAttempt")(function* (
      input: FleetPromoteInput,
    ) {
      const response = yield* transport.request({
        origin: input.origin,
        token: input.token,
        method: "POST",
        path: FLEET_TARGETS_PATH,
        body: {
          repo: input.repo,
          sha: input.sha,
          environment: input.environment,
          idempotency_key: input.idempotencyKey,
          ...(input.expectedCurrentTargetId === undefined
            ? {}
            : { expected_current_target_id: input.expectedCurrentTargetId }),
        },
      });
      if (response.status !== 202 && response.status !== 200) {
        const details = trackerErrorDetails(response.body, response.status);
        return yield* new ApiError({
          operation: "promote a fleet target",
          status: response.status,
          ...(details.code === undefined ? {} : { code: details.code }),
          message: details.message,
          ...(response.requestId === undefined && details.requestId === undefined
            ? {}
            : { requestId: response.requestId ?? details.requestId }),
        });
      }
      return {
        accepted: response.status === 202,
        replayed: response.status === 200,
        target: asRecord(response.body),
      } satisfies FleetPromoteResult;
    });

    // The idempotency key travels in the body, so every attempt names the
    // same promotion and a re-send can never deploy twice. Only a failed
    // transport is retried — the request may never have reached the server;
    // a refusal the server actually made is final.
    const promote = Effect.fn("FleetClient.promote")(function* (input: FleetPromoteInput) {
      let attempt = 0;
      while (true) {
        const outcome = yield* promoteAttempt(input).pipe(
          Effect.map((value) => ({ ok: true, value }) as const),
          Effect.catchTag("OpenAgentsCli.TransportError", (failure) =>
            Effect.succeed({ ok: false, failure } as const),
          ),
        );
        if (outcome.ok) return outcome.value;
        if (attempt >= PROMOTE_TRANSPORT_RETRIES) return yield* outcome.failure;
        attempt += 1;
        yield* Effect.sleep(Duration.millis(retryDelayMs * attempt));
      }
    });

    const list = (input: FleetListInput) => {
      const parameters = new URLSearchParams();
      if (input.repo !== undefined) parameters.set("repo", input.repo);
      if (input.limit !== undefined) parameters.set("limit", String(input.limit));
      const query = parameters.toString();
      return request("list fleet targets", {
        origin: input.origin,
        token: input.token,
        method: "GET",
        path: query === "" ? FLEET_TARGETS_PATH : `${FLEET_TARGETS_PATH}?${query}`,
        acceptedStatuses: [200],
      });
    };

    const wait = Effect.fn("FleetClient.wait")(function* (input: FleetWaitInput) {
      const baseDelayMs = input.baseDelayMs ?? POLL_BASE_DELAY_MS;
      const maximumDelayMs = input.maximumDelayMs ?? POLL_MAXIMUM_DELAY_MS;
      const startedAt = yield* Clock.currentTimeMillis;
      let attempt = 0;
      while (true) {
        const target = yield* view(input);
        const status = targetStatus(target);
        if (terminalStatus(status)) return target;
        const now = yield* Clock.currentTimeMillis;
        if (now - startedAt >= input.timeoutMs) {
          // The target is still running; only the CLI's watching ended.
          return yield* new DeploymentWaitTimeout({
            targetId: input.id,
            timeoutMs: input.timeoutMs,
            lastStatus: status,
            message:
              `The fleet target ${input.id} was still ${status} after ` +
              `${Math.round(input.timeoutMs / 1_000)}s. The deployment has not failed; ` +
              `resume with: openagents deploy view ${input.id} --wait`,
          });
        }
        const delay = Math.min(baseDelayMs * 2 ** attempt, maximumDelayMs);
        attempt += 1;
        yield* Effect.sleep(Duration.millis(delay));
      }
    });

    return FleetClient.of({ promote, view, list, wait });
  }),
);
