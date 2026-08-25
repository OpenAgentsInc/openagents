/**
 * The Box client for conversation-owned Cloud Computer sandbox VMs.
 *
 * Interacts with OpenAgents backend at `/api/v1/conversations/:id/boxes`
 * with support for listing, creating, inspecting, commanding, stopping,
 * running durable jobs, and fanout planning.
 */

import { Effect, Layer } from "effect";
import * as Context from "effect/Context";

import { ApiTransport } from "./api-transport.js";
import { API_VERSION_PATH } from "./constants.js";
import { ApiError, type CliError } from "./errors.js";
import type { AuthenticatedApi } from "./repository-client.js";
import { asNumber, asRecord, asRows, asText, makeTrackerRequest } from "./tracker-request.js";

export interface BoxRecord {
  readonly box_id: string;
  readonly label?: string | null;
  readonly state: string;
  readonly setup_status: string;
  readonly created_at: string;
  readonly updated_at?: string | null;
  readonly stopped_at?: string | null;
}

export interface BoxCommandResult {
  readonly box_id: string;
  readonly exit_code: number;
  readonly stdout: string;
  readonly stderr: string;
  readonly timed_out: boolean;
  readonly stdout_truncated: boolean;
  readonly stderr_truncated: boolean;
}

export interface BoxRunRecord {
  readonly id: string;
  readonly box_id: string;
  readonly command: string;
  readonly state: string;
  readonly exit_status?: number | null;
  readonly timed_out?: boolean | null;
  readonly output_offset?: number | null;
  readonly output_base_offset?: number | null;
  readonly failure_reason?: string | null;
  readonly admitted_at?: string | null;
  readonly dispatched_at?: string | null;
  readonly started_at?: string | null;
  readonly finished_at?: string | null;
  readonly deadline_at?: string | null;
  readonly cancellation_requested_at?: string | null;
  readonly cancellation_effective_at?: string | null;
}

export interface BoxFanoutItem {
  readonly position: number;
  readonly label: string;
  readonly state: string;
  readonly box_id?: string | null;
  readonly queue_reason?: string | null;
  readonly estimated_burn_rate_microusd?: number | null;
  readonly admitted_at?: string | null;
}

export interface BoxFanoutPlan {
  readonly id: string;
  readonly requested_count: number;
  readonly admitted: ReadonlyArray<BoxFanoutItem>;
  readonly queued: ReadonlyArray<BoxFanoutItem>;
  readonly effective_limits?: Record<string, unknown>;
  readonly budgeted: boolean;
  readonly created_at?: string | null;
  readonly updated_at?: string | null;
}

export interface BoxListInput extends AuthenticatedApi {
  readonly conversationId?: string;
}

export interface BoxCreateInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly label?: string;
}

export interface BoxViewInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly boxId: string;
}

export interface BoxCommandInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly boxId: string;
  readonly command: string;
  readonly timeoutSeconds?: number;
}

export interface BoxStopInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly boxId: string;
}

export interface BoxRunCreateInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly boxId: string;
  readonly command: string;
  readonly idempotencyKey?: string;
}

export interface BoxRunListInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly boxId: string;
}

export interface BoxRunViewInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly boxId: string;
  readonly runId: string;
}

export interface BoxRunOutput {
  readonly run_id: string;
  readonly output: string;
  /** The offset to pass to the next read to resume where this one stopped. */
  readonly next_offset: number;
  /** True when the box dropped bytes before the requested offset. */
  readonly truncated: boolean;
}

export interface BoxRunOutputInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly boxId: string;
  readonly runId: string;
  readonly offset?: number;
}

export interface BoxRunCancelInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly boxId: string;
  readonly runId: string;
}

export interface BoxFanoutInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly count: number;
  readonly labels?: ReadonlyArray<string>;
  readonly budgeted?: boolean;
}

export interface BoxFanoutViewInput extends AuthenticatedApi {
  readonly conversationId?: string;
  readonly requestId: string;
}

export interface BoxClientInterface {
  readonly resolveConversationId: (input: AuthenticatedApi) => Effect.Effect<string, CliError>;
  readonly list: (input: BoxListInput) => Effect.Effect<ReadonlyArray<BoxRecord>, CliError>;
  readonly create: (input: BoxCreateInput) => Effect.Effect<BoxRecord, CliError>;
  readonly view: (input: BoxViewInput) => Effect.Effect<BoxRecord, CliError>;
  readonly exec: (input: BoxCommandInput) => Effect.Effect<BoxCommandResult, CliError>;
  readonly stop: (input: BoxStopInput) => Effect.Effect<BoxRecord, CliError>;
  readonly startRun: (input: BoxRunCreateInput) => Effect.Effect<BoxRunRecord, CliError>;
  readonly listRuns: (input: BoxRunListInput) => Effect.Effect<ReadonlyArray<BoxRunRecord>, CliError>;
  readonly viewRun: (input: BoxRunViewInput) => Effect.Effect<BoxRunRecord, CliError>;
  readonly runOutput: (input: BoxRunOutputInput) => Effect.Effect<BoxRunOutput, CliError>;
  readonly cancelRun: (input: BoxRunCancelInput) => Effect.Effect<BoxRunRecord, CliError>;
  readonly fanout: (input: BoxFanoutInput) => Effect.Effect<BoxFanoutPlan, CliError>;
  readonly viewFanout: (input: BoxFanoutViewInput) => Effect.Effect<BoxFanoutPlan, CliError>;
}

export class BoxClient extends Context.Service<BoxClient, BoxClientInterface>()(
  "@openagentsinc/cli/BoxClient",
) {}

export const boxClientLayer = Layer.effect(
  BoxClient,
  Effect.gen(function* () {
    const transport = yield* ApiTransport;
    const request = makeTrackerRequest(transport);

    // No deployed endpoint resolves the account's conversation yet. `GET
    // /api/v1/user` answers the forge identity and never carries a
    // `conversation_id`, and it sits behind `forge:write` rather than the
    // `box:control` scope a box token carries, so this probe fails twice over
    // against production. The probe stays because a deployment that grows the
    // field should start working without a client release, but the refusal
    // now names the flag that actually gets the caller unblocked instead of
    // claiming the account has no conversation.
    const resolveConversationId = Effect.fn("BoxClient.resolveConversationId")(function* (
      input: AuthenticatedApi,
    ) {
      // `/conversation` is the route a `box:control` token can actually
      // reach, and it creates the account's conversation when there is none.
      // `/user` sits behind `forge:write`, so resolving through it refused
      // every box command a box-scoped credential tried to run.
      const named = yield* transport.request({
        origin: input.origin,
        method: "GET",
        path: `${API_VERSION_PATH}/conversation`,
        token: input.token,
      });
      if (named.status === 200) {
        const convId = asText(asRecord(named.body)["conversation_id"]);
        if (convId !== undefined) return convId;
      }

      // A deployment that predates that route still answers on `/user`, so a
      // published CLI keeps working against one. Kept deliberately: the CLI
      // ships on its own schedule and cannot assume the server is ahead of it.
      const user = yield* transport.request({
        origin: input.origin,
        method: "GET",
        path: `${API_VERSION_PATH}/user`,
        token: input.token,
      });
      if (user.status === 200) {
        const body = asRecord(user.body);
        const convId =
          asText(body["conversation_id"]) ??
          asText(asRecord(body["openagents"])["conversation_id"]) ??
          asText(asRecord(body["user"])["conversation_id"]);
        if (convId !== undefined) return convId;
      }

      return yield* new ApiError({
        operation: "resolve user conversation",
        status: named.status === 200 ? user.status : named.status,
        message:
          "This deployment does not report a conversation for the account. " +
          "Pass --conversation <conversation_id> to name the conversation to use.",
      });
    });

    const getConvId = (input: AuthenticatedApi & { readonly conversationId?: string }) =>
      input.conversationId !== undefined
        ? Effect.succeed(input.conversationId)
        : resolveConversationId(input);

    const list = Effect.fn("BoxClient.list")(function* (input: BoxListInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("list conversation boxes", {
        origin: input.origin,
        token: input.token,
        method: "GET",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes`,
        acceptedStatuses: [200],
      });
      const rows = asRows(body, "boxes");
      return rows.map((row) => ({
        box_id: asText(row["box_id"]) ?? "",
        label: asText(row["label"]) ?? null,
        state: asText(row["state"]) ?? "unknown",
        setup_status: asText(row["setup_status"]) ?? "unknown",
        created_at: asText(row["created_at"]) ?? "",
        updated_at: asText(row["updated_at"]) ?? null,
        stopped_at: asText(row["stopped_at"]) ?? null,
      }));
    });

    const create = Effect.fn("BoxClient.create")(function* (input: BoxCreateInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("create box", {
        origin: input.origin,
        token: input.token,
        method: "POST",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes`,
        body: input.label !== undefined ? { label: input.label } : {},
        acceptedStatuses: [201],
      });
      const box = asRecord(asRecord(body)["box"]);
      return {
        box_id: asText(box["box_id"]) ?? "",
        label: asText(box["label"]) ?? null,
        state: asText(box["state"]) ?? "unknown",
        setup_status: asText(box["setup_status"]) ?? "unknown",
        created_at: asText(box["created_at"]) ?? "",
        updated_at: asText(box["updated_at"]) ?? null,
        stopped_at: asText(box["stopped_at"]) ?? null,
      };
    });

    const view = Effect.fn("BoxClient.view")(function* (input: BoxViewInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("view box", {
        origin: input.origin,
        token: input.token,
        method: "GET",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes/${encodeURIComponent(input.boxId)}`,
        acceptedStatuses: [200],
      });
      const box = asRecord(asRecord(body)["box"]);
      return {
        box_id: asText(box["box_id"]) ?? "",
        label: asText(box["label"]) ?? null,
        state: asText(box["state"]) ?? "unknown",
        setup_status: asText(box["setup_status"]) ?? "unknown",
        created_at: asText(box["created_at"]) ?? "",
        updated_at: asText(box["updated_at"]) ?? null,
        stopped_at: asText(box["stopped_at"]) ?? null,
      };
    });

    const exec = Effect.fn("BoxClient.exec")(function* (input: BoxCommandInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("run box command", {
        origin: input.origin,
        token: input.token,
        method: "POST",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes/${encodeURIComponent(input.boxId)}/commands`,
        body: {
          command: input.command,
          ...(input.timeoutSeconds !== undefined ? { timeout_seconds: input.timeoutSeconds } : {}),
        },
        acceptedStatuses: [200],
      });
      const res = asRecord(asRecord(body)["result"]);
      return {
        box_id: asText(res["box_id"]) ?? input.boxId,
        exit_code: typeof res["exit_code"] === "number" ? res["exit_code"] : -1,
        stdout: asText(res["stdout"]) ?? "",
        stderr: asText(res["stderr"]) ?? "",
        timed_out: res["timed_out"] === true,
        stdout_truncated: res["stdout_truncated"] === true,
        stderr_truncated: res["stderr_truncated"] === true,
      };
    });

    const stop = Effect.fn("BoxClient.stop")(function* (input: BoxStopInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("stop box", {
        origin: input.origin,
        token: input.token,
        method: "POST",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes/${encodeURIComponent(input.boxId)}/stop`,
        acceptedStatuses: [200],
      });
      const box = asRecord(asRecord(body)["box"]);
      return {
        box_id: asText(box["box_id"]) ?? "",
        label: asText(box["label"]) ?? null,
        state: asText(box["state"]) ?? "unknown",
        setup_status: asText(box["setup_status"]) ?? "unknown",
        created_at: asText(box["created_at"]) ?? "",
        updated_at: asText(box["updated_at"]) ?? null,
        stopped_at: asText(box["stopped_at"]) ?? null,
      };
    });

    const startRun = Effect.fn("BoxClient.startRun")(function* (input: BoxRunCreateInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("start box run", {
        origin: input.origin,
        token: input.token,
        method: "POST",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes/${encodeURIComponent(input.boxId)}/runs`,
        body: {
          command: input.command,
          idempotency_key: input.idempotencyKey ?? crypto.randomUUID(),
        },
        acceptedStatuses: [200, 202],
      });
      const run = asRecord(asRecord(body)["run"]);
      return {
        id: asText(run["id"]) ?? "",
        box_id: asText(run["box_id"]) ?? input.boxId,
        command: asText(run["command"]) ?? input.command,
        state: asText(run["state"]) ?? "unknown",
        exit_status: typeof run["exit_status"] === "number" ? run["exit_status"] : null,
        timed_out: typeof run["timed_out"] === "boolean" ? run["timed_out"] : null,
        output_offset: typeof run["output_offset"] === "number" ? run["output_offset"] : null,
        output_base_offset: typeof run["output_base_offset"] === "number" ? run["output_base_offset"] : null,
        failure_reason: asText(run["failure_reason"]) ?? null,
        admitted_at: asText(run["admitted_at"]) ?? null,
        dispatched_at: asText(run["dispatched_at"]) ?? null,
        started_at: asText(run["started_at"]) ?? null,
        finished_at: asText(run["finished_at"]) ?? null,
        deadline_at: asText(run["deadline_at"]) ?? null,
        cancellation_requested_at: asText(run["cancellation_requested_at"]) ?? null,
        cancellation_effective_at: asText(run["cancellation_effective_at"]) ?? null,
      };
    });

    const listRuns = Effect.fn("BoxClient.listRuns")(function* (input: BoxRunListInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("list box runs", {
        origin: input.origin,
        token: input.token,
        method: "GET",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes/${encodeURIComponent(input.boxId)}/runs`,
        acceptedStatuses: [200],
      });
      const rows = asRows(body, "runs");
      return rows.map((run) => ({
        id: asText(run["id"]) ?? "",
        box_id: asText(run["box_id"]) ?? input.boxId,
        command: asText(run["command"]) ?? "",
        state: asText(run["state"]) ?? "unknown",
        exit_status: typeof run["exit_status"] === "number" ? run["exit_status"] : null,
        timed_out: typeof run["timed_out"] === "boolean" ? run["timed_out"] : null,
        output_offset: typeof run["output_offset"] === "number" ? run["output_offset"] : null,
        output_base_offset: typeof run["output_base_offset"] === "number" ? run["output_base_offset"] : null,
        failure_reason: asText(run["failure_reason"]) ?? null,
        admitted_at: asText(run["admitted_at"]) ?? null,
        dispatched_at: asText(run["dispatched_at"]) ?? null,
        started_at: asText(run["started_at"]) ?? null,
        finished_at: asText(run["finished_at"]) ?? null,
        deadline_at: asText(run["deadline_at"]) ?? null,
        cancellation_requested_at: asText(run["cancellation_requested_at"]) ?? null,
        cancellation_effective_at: asText(run["cancellation_effective_at"]) ?? null,
      }));
    });

    const viewRun = Effect.fn("BoxClient.viewRun")(function* (input: BoxRunViewInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("view box run", {
        origin: input.origin,
        token: input.token,
        method: "GET",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes/${encodeURIComponent(input.boxId)}/runs/${encodeURIComponent(input.runId)}`,
        acceptedStatuses: [200],
      });
      const run = asRecord(asRecord(body)["run"]);
      return {
        id: asText(run["id"]) ?? input.runId,
        box_id: asText(run["box_id"]) ?? input.boxId,
        command: asText(run["command"]) ?? "",
        state: asText(run["state"]) ?? "unknown",
        exit_status: typeof run["exit_status"] === "number" ? run["exit_status"] : null,
        timed_out: typeof run["timed_out"] === "boolean" ? run["timed_out"] : null,
        output_offset: typeof run["output_offset"] === "number" ? run["output_offset"] : null,
        output_base_offset: typeof run["output_base_offset"] === "number" ? run["output_base_offset"] : null,
        failure_reason: asText(run["failure_reason"]) ?? null,
        admitted_at: asText(run["admitted_at"]) ?? null,
        dispatched_at: asText(run["dispatched_at"]) ?? null,
        started_at: asText(run["started_at"]) ?? null,
        finished_at: asText(run["finished_at"]) ?? null,
        deadline_at: asText(run["deadline_at"]) ?? null,
        cancellation_requested_at: asText(run["cancellation_requested_at"]) ?? null,
        cancellation_effective_at: asText(run["cancellation_effective_at"]) ?? null,
      };
    });

    const runOutput = Effect.fn("BoxClient.runOutput")(function* (input: BoxRunOutputInput) {
      const convId = yield* getConvId(input);
      const query = input.offset !== undefined ? `?offset=${String(input.offset)}` : "";
      const body = yield* request("get box run output", {
        origin: input.origin,
        token: input.token,
        method: "GET",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes/${encodeURIComponent(input.boxId)}/runs/${encodeURIComponent(input.runId)}/output${query}`,
        acceptedStatuses: [200],
      });
      // The server nests the read under `output`: the envelope is
      // `{"run_id": …, "output": {"output": …, "next_offset": …, "truncated": …}}`,
      // so reading the envelope's `output` as text yielded an empty string and
      // `box runs output` printed nothing. Read the nested record, and fall
      // back to a bare string for any older deployment still answering flat.
      const res = asRecord(body);
      const nested = asRecord(res["output"]);
      const flat = asText(res["output"]);
      return {
        run_id: asText(res["run_id"]) ?? input.runId,
        output: flat ?? asText(nested["output"]) ?? "",
        next_offset: asNumber(nested["next_offset"]) ?? input.offset ?? 0,
        truncated: nested["truncated"] === true,
      };
    });

    const cancelRun = Effect.fn("BoxClient.cancelRun")(function* (input: BoxRunCancelInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("cancel box run", {
        origin: input.origin,
        token: input.token,
        method: "POST",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes/${encodeURIComponent(input.boxId)}/runs/${encodeURIComponent(input.runId)}/cancel`,
        acceptedStatuses: [200, 202],
      });
      const run = asRecord(asRecord(body)["run"]);
      return {
        id: asText(run["id"]) ?? input.runId,
        box_id: asText(run["box_id"]) ?? input.boxId,
        command: asText(run["command"]) ?? "",
        state: asText(run["state"]) ?? "unknown",
        exit_status: typeof run["exit_status"] === "number" ? run["exit_status"] : null,
        timed_out: typeof run["timed_out"] === "boolean" ? run["timed_out"] : null,
        output_offset: typeof run["output_offset"] === "number" ? run["output_offset"] : null,
        output_base_offset: typeof run["output_base_offset"] === "number" ? run["output_base_offset"] : null,
        failure_reason: asText(run["failure_reason"]) ?? null,
        admitted_at: asText(run["admitted_at"]) ?? null,
        dispatched_at: asText(run["dispatched_at"]) ?? null,
        started_at: asText(run["started_at"]) ?? null,
        finished_at: asText(run["finished_at"]) ?? null,
        deadline_at: asText(run["deadline_at"]) ?? null,
        cancellation_requested_at: asText(run["cancellation_requested_at"]) ?? null,
        cancellation_effective_at: asText(run["cancellation_effective_at"]) ?? null,
      };
    });

    const fanout = Effect.fn("BoxClient.fanout")(function* (input: BoxFanoutInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("request box fanout", {
        origin: input.origin,
        token: input.token,
        method: "POST",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes/fanout`,
        body: {
          count: input.count,
          ...(input.labels !== undefined ? { labels: input.labels } : {}),
          ...(input.budgeted !== undefined ? { budgeted: input.budgeted } : {}),
        },
        acceptedStatuses: [200, 202],
      });
      const plan = asRecord(asRecord(body)["plan"]);
      const admittedRows = asRows(plan, "admitted");
      const queuedRows = asRows(plan, "queued");
      const parseItem = (item: Record<string, unknown>): BoxFanoutItem => ({
        position: typeof item["position"] === "number" ? item["position"] : 0,
        label: asText(item["label"]) ?? "",
        state: asText(item["state"]) ?? "unknown",
        box_id: asText(item["box_id"]) ?? null,
        queue_reason: asText(item["queue_reason"]) ?? null,
        estimated_burn_rate_microusd: typeof item["estimated_burn_rate_microusd"] === "number" ? item["estimated_burn_rate_microusd"] : null,
        admitted_at: asText(item["admitted_at"]) ?? null,
      });

      return {
        id: asText(plan["id"]) ?? "",
        requested_count: typeof plan["requested_count"] === "number" ? plan["requested_count"] : input.count,
        admitted: admittedRows.map(parseItem),
        queued: queuedRows.map(parseItem),
        effective_limits: asRecord(plan["effective_limits"]),
        budgeted: plan["budgeted"] === true,
        created_at: asText(plan["created_at"]) ?? null,
        updated_at: asText(plan["updated_at"]) ?? null,
      };
    });

    const viewFanout = Effect.fn("BoxClient.viewFanout")(function* (input: BoxFanoutViewInput) {
      const convId = yield* getConvId(input);
      const body = yield* request("view box fanout", {
        origin: input.origin,
        token: input.token,
        method: "GET",
        path: `${API_VERSION_PATH}/conversations/${encodeURIComponent(convId)}/boxes/fanout/${encodeURIComponent(input.requestId)}`,
        acceptedStatuses: [200],
      });
      const plan = asRecord(asRecord(body)["plan"]);
      const admittedRows = asRows(plan, "admitted");
      const queuedRows = asRows(plan, "queued");
      const parseItem = (item: Record<string, unknown>): BoxFanoutItem => ({
        position: typeof item["position"] === "number" ? item["position"] : 0,
        label: asText(item["label"]) ?? "",
        state: asText(item["state"]) ?? "unknown",
        box_id: asText(item["box_id"]) ?? null,
        queue_reason: asText(item["queue_reason"]) ?? null,
        estimated_burn_rate_microusd: typeof item["estimated_burn_rate_microusd"] === "number" ? item["estimated_burn_rate_microusd"] : null,
        admitted_at: asText(item["admitted_at"]) ?? null,
      });

      return {
        id: asText(plan["id"]) ?? input.requestId,
        requested_count: typeof plan["requested_count"] === "number" ? plan["requested_count"] : 0,
        admitted: admittedRows.map(parseItem),
        queued: queuedRows.map(parseItem),
        effective_limits: asRecord(plan["effective_limits"]),
        budgeted: plan["budgeted"] === true,
        created_at: asText(plan["created_at"]) ?? null,
        updated_at: asText(plan["updated_at"]) ?? null,
      };
    });

    return BoxClient.of({
      resolveConversationId,
      list,
      create,
      view,
      exec,
      stop,
      startRun,
      listRuns,
      viewRun,
      runOutput,
      cancelRun,
      fanout,
      viewFanout,
    });
  }),
);
