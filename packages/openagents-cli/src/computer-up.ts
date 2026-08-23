import { Effect, Layer, Option } from "effect";
import * as Context from "effect/Context";

import { ComputerChannel, type ComputerChannelHandlers } from "./computer-channel.js";
import { ComputerClient } from "./computer-client.js";
import { ComputerConfiguration } from "./computer-config.js";
import { ComputerJournal, type JournalInterface } from "./computer-journal.js";
import {
  computerExecutionDefaults,
  executeComputerCommand,
  type ComputerExecutionLimits,
  type RunningComputerExecution,
} from "./computer-executor.js";
import { ComputerProbe } from "./computer-probe.js";
import { decide, tierAllows, type CommandRequest, type Tier } from "./computer-policy.js";
import { CredentialStore } from "./credential-store.js";
import { InputError, type CliError } from "./errors.js";

export interface ComputerUpInterface {
  readonly serve: (origin: string, agentVersion: string) => Effect.Effect<string, CliError>;
}

export class ComputerUp extends Context.Service<ComputerUp, ComputerUpInterface>()(
  "@openagentsinc/cli/ComputerUp",
) {}

const maximumConcurrency = 2;
const maximumArgvLength = 64;
const maximumArgumentLength = 1_024;
const maximumTimeoutMillis = computerExecutionDefaults.timeoutMillis;
const maximumOutputBytes = computerExecutionDefaults.maximumOutputBytes;

const requestFields = (payload: Record<string, unknown>): CommandRequest | undefined => {
  const argv = payload.argv;
  const cwd = payload.cwd;
  if (
    !Array.isArray(argv) ||
    argv.length === 0 ||
    argv.length > maximumArgvLength ||
    !argv.every((value) => typeof value === "string" && value.length <= maximumArgumentLength) ||
    typeof cwd !== "string" ||
    cwd.length > 4_096
  ) {
    return undefined;
  }
  return { argv, cwd };
};

const numberField = (
  payload: Record<string, unknown>,
  names: ReadonlyArray<string>,
  fallback: number,
  ceiling: number,
): number => {
  const requested = names.map((name) => payload[name]).find((value) => typeof value === "number");
  if (typeof requested !== "number" || !Number.isFinite(requested) || requested <= 0)
    return fallback;
  return Math.min(Math.floor(requested), ceiling);
};

const journal = (
  journalService: JournalInterface,
  requestId: string,
  request: CommandRequest,
  decision: string,
  outcome: string,
  detail: string,
): void => {
  Effect.runFork(
    journalService.append({
      requestId,
      argv: request.argv,
      cwd: request.cwd,
      decision,
      outcome,
      detail,
    }),
  );
};

export const computerUpLayer = Layer.effect(
  ComputerUp,
  Effect.gen(function* () {
    const channel = yield* ComputerChannel;
    const client = yield* ComputerClient;
    const config = yield* ComputerConfiguration;
    const credentials = yield* CredentialStore;
    const journalService = yield* ComputerJournal;
    const probe = yield* ComputerProbe;
    const probeContext = yield* Effect.context<ComputerProbe>();

    const serve = Effect.fn("ComputerUp.serve")(function* (origin: string, agentVersion: string) {
      const stored = yield* credentials.get(origin, "computer");
      if (Option.isNone(stored)) {
        return yield* new InputError({
          message: `This Computer is not paired with ${origin}; run computer pair first.`,
        });
      }
      const status = yield* client.status(origin, stored.value);
      if (Option.isNone(status)) {
        return yield* new InputError({
          message: `This Computer is no longer active on ${origin}; run computer logout.`,
        });
      }
      const initialProbe = yield* probe.probe(config.roots);
      const executions = new Map<string, RunningComputerExecution>();
      let active = 0;
      const append = (
        requestId: string,
        request: CommandRequest,
        decision: string,
        outcome: string,
        detail: string,
      ) => journal(journalService, requestId, request, decision, outcome, detail);
      const handlers: ComputerChannelHandlers = {
        onProbe: async (requestId) => {
          const request = { argv: ["<probe>"], cwd: config.roots[0] ?? "" };
          append(requestId, request, "received", "pending", "read-only probe requested");
          try {
            const report = await Effect.runPromiseWith(probeContext)(probe.probe(config.roots));
            append(requestId, request, "allowed", "completed", "probe completed");
            return report;
          } catch (cause) {
            append(requestId, request, "allowed", "refused", String(cause));
            throw cause;
          }
        },
        onRun: (requestId, payload, responder) => {
          const request = requestFields(payload);
          if (request === undefined) {
            const malformed = { argv: ["<invalid>"], cwd: "" };
            append(requestId, malformed, "refused", "refused", "invalid command request");
            responder.refused("invalid_request", "argv and cwd are required and must be bounded");
            return;
          }
          append(requestId, request, "received", "pending", "command request received");
          const requestedTier = payload.tier;
          if (
            (requestedTier === "probe" ||
              requestedTier === "curated" ||
              requestedTier === "shell") &&
            !tierAllows(config.tier, requestedTier as Tier)
          ) {
            append(
              requestId,
              request,
              "tier_insufficient",
              "refused",
              "the requested tier exceeds the local ceiling",
            );
            responder.refused("tier_insufficient", "the requested tier exceeds the local ceiling");
            return;
          }
          const decision = decide(request, config);
          if ("reason" in decision) {
            append(requestId, request, decision.reason, "refused", decision.detail);
            responder.refused(decision.reason, decision.detail);
            return;
          }
          if (decision.needsConfirmation) {
            append(
              requestId,
              request,
              "confirmation_required",
              "refused",
              "local confirmation is required",
            );
            responder.refused(
              "confirmation_required",
              "local confirmation is required for this command",
            );
            return;
          }
          if (active >= maximumConcurrency) {
            append(
              requestId,
              request,
              "allowed",
              "refused",
              "local execution concurrency limit reached",
            );
            responder.refused("busy", "the local execution limit is reached");
            return;
          }
          active += 1;
          const limits: ComputerExecutionLimits = {
            timeoutMillis: numberField(
              payload,
              ["timeout_ms", "timeout"],
              maximumTimeoutMillis,
              maximumTimeoutMillis,
            ),
            maximumOutputBytes: numberField(
              payload,
              ["maximum_output_bytes", "output_max_bytes", "max_output_bytes"],
              maximumOutputBytes,
              maximumOutputBytes,
            ),
          };
          append(requestId, request, "allowed", "running", `timeout=${limits.timeoutMillis}`);
          const execution = executeComputerCommand(
            request.argv,
            request.cwd,
            limits,
            responder.chunk,
          );
          executions.set(requestId, execution);
          void execution.done
            .then((outcome) => {
              active -= 1;
              executions.delete(requestId);
              const terminalStatus = outcome.cancelled
                ? "cancelled"
                : outcome.timedOut
                  ? "timeout"
                  : outcome.exitCode === 0
                    ? "completed"
                    : "failed";
              const detail = outcome.truncated ? "output truncated" : "";
              append(requestId, request, "allowed", terminalStatus, detail);
              responder.exit({
                status: terminalStatus,
                exit_code: outcome.exitCode,
                timed_out: outcome.timedOut,
                cancelled: outcome.cancelled,
                truncated: outcome.truncated,
                duration_ms: outcome.durationMillis,
              });
            })
            .catch(() => {
              active -= 1;
              executions.delete(requestId);
              append(requestId, request, "allowed", "failed", "execution failed");
              responder.exit({ status: "failed", exit_code: null, truncated: false });
            });
        },
        onCancel: (requestId) => {
          const execution = executions.get(requestId);
          if (execution === undefined) return;
          execution.cancel();
          const request = { argv: ["<cancel>"], cwd: "" };
          append(
            requestId,
            request,
            "allowed",
            "cancelling",
            "process group termination requested",
          );
        },
        onJoined: () => undefined,
        onEvent: (event) => {
          const request = { argv: ["<connection>"], cwd: "" };
          append("connection", request, "transport", "event", event);
        },
        onClosed: (reason) => {
          const request = { argv: ["<connection>"], cwd: "" };
          append("connection", request, "transport", "closed", reason);
          for (const execution of executions.values()) execution.cancel();
        },
      };
      return yield* channel.serve(
        {
          origin,
          token: stored.value,
          machineId: status.value.machine_id,
          hello: {
            agent_version: agentVersion,
            tier: config.tier,
            roots: config.roots,
            platform: `${process.platform}-${process.arch}`,
            probe: initialProbe,
          },
        },
        handlers,
      );
    });
    return ComputerUp.of({ serve });
  }),
);
