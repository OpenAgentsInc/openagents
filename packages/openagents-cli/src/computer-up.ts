import { Effect, Layer, Option, Redacted } from "effect";
import * as Context from "effect/Context";

import type { ForgeCredentials } from "./delegation-push.js";

import {
  ComputerChannel,
  type ComputerAgentResponder,
  type ComputerChannelHandlers,
} from "./computer-channel.js";
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
import {
  buildAgentCatalog,
  ComputerAgentProcess,
  resolveAgent,
  startAgentDelegation,
  type AgentDelegationJob,
  type AgentDelegationOutcome,
} from "./computer-agents.js";
import { scrubbedEnvironment } from "./computer-executor.js";
import { withinRoot } from "./computer-policy.js";
import { resolve } from "node:path";

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
const maximumPromptLength = 32_768;

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
    const agentProcess = yield* Effect.serviceOption(ComputerAgentProcess);
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
      const agentJobs = new Map<
        string,
        {
          readonly job: AgentDelegationJob;
          readonly request: { readonly argv: ReadonlyArray<string>; readonly cwd: string };
          readonly setResponder: (responder: ComputerAgentResponder) => void;
          sessionId: string;
        }
      >();
      let active = 0;
      const append = (
        requestId: string,
        request: CommandRequest,
        decision: string,
        outcome: string,
        detail: string,
      ) => journal(journalService, requestId, request, decision, outcome, detail);
      const recordValue = (value: unknown): Record<string, unknown> =>
        typeof value === "object" && value !== null && !Array.isArray(value)
          ? { ...(value as Record<string, unknown>) }
          : {};
      const asString = (value: unknown): string | undefined =>
        typeof value === "string" && value !== "" ? value : undefined;
      const extractForgeCredentials = (
        payload: Record<string, unknown>,
      ): ForgeCredentials | undefined => {
        const raw = payload.assignment_credential ?? payload.forge_credentials;
        let token: string | undefined;
        let repository: string | undefined;
        let branch: string | undefined;
        if (typeof raw === "string") {
          token = raw;
          repository = asString(payload.assignment_repository);
          branch = asString(payload.assignment_branch);
        } else if (typeof raw === "object" && raw !== null) {
          const cred = recordValue(raw);
          const candidate =
            cred.token ?? cred.value ?? cred.password ?? cred.access_token;
          token = asString(candidate);
          repository = asString(cred.repository ?? payload.assignment_repository);
          branch = asString(cred.branch ?? payload.assignment_branch);
        }
        if (token === undefined || repository === undefined || branch === undefined)
          return undefined;
        return { token: Redacted.make(token), repository, branch };
      };
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
        onAgent: (requestId, payload, responder) => {
          const agentId = typeof payload.agent_id === "string" ? payload.agent_id : "";
          const prompt = typeof payload.prompt === "string" ? payload.prompt : "";
          const cwd = typeof payload.cwd === "string" ? resolve(payload.cwd) : "";
          const request = { argv: ["<agent>", agentId.slice(0, 64)], cwd };
          append(requestId, request, "received", "pending", "ACP delegation received");
          const forgeCredentials = extractForgeCredentials(payload);
          if (forgeCredentials !== undefined) {
            append(
              requestId,
              request,
              "credentials_delivered",
              "configured",
              "scoped forge credentials configured for delegated push",
            );
          } else if (
            Object.hasOwn(payload, "assignment_credential") ||
            Object.hasOwn(payload, "forge_credentials")
          ) {
            append(
              requestId,
              request,
              "credentials_delivered",
              "incomplete",
              "scoped forge credentials delivered but missing repository or branch",
            );
          }
          if (Option.isNone(agentProcess)) {
            append(requestId, request, "unsupported", "refused", "ACP delegation is unavailable");
            responder.refused("unsupported", "ACP delegation is unavailable");
            return;
          }
          if (
            agentId === "" ||
            prompt === "" ||
            prompt.length > maximumPromptLength ||
            cwd === ""
          ) {
            append(
              requestId,
              request,
              "refused",
              "refused",
              "agent, prompt, and cwd are required and bounded",
            );
            responder.refused("invalid_request", "agent, prompt, and cwd are required and bounded");
            return;
          }
          if (!config.roots.some((root) => withinRoot(cwd, root))) {
            append(
              requestId,
              request,
              "root_not_declared",
              "refused",
              "the working directory is outside every declared root",
            );
            responder.refused(
              "root_not_declared",
              "the working directory is outside every declared root",
            );
            return;
          }
          const catalog = buildAgentCatalog(
            { agents: config.agents ?? [], registryAgents: config.registryAgents ?? false },
            initialProbe.codingAgents,
          );
          const resolution = resolveAgent(catalog, agentId);
          if (resolution._tag === "unavailable") {
            const detail = `agent ${resolution.requestedId} is unavailable; available agents: ${resolution.availableIds.join(", ") || "(none)"}`;
            append(requestId, request, "agent_unavailable", "refused", detail);
            responder.refused("agent_unavailable", detail);
            return;
          }
          const resumeSessionId =
            typeof payload.resume_session_id === "string"
              ? payload.resume_session_id.slice(0, 128)
              : undefined;
          if (resumeSessionId !== undefined) {
            const existing = [...agentJobs.values()].find(
              (value) => value.sessionId === resumeSessionId,
            );
            if (existing !== undefined) {
              existing.setResponder(responder);
              agentJobs.set(requestId, existing);
              append(
                requestId,
                request,
                "reattached",
                "running",
                "reattached to the existing ACP session",
              );
              return;
            }
            append(
              requestId,
              request,
              "session_not_found",
              "refused",
              "the requested ACP session is no longer live",
            );
            responder.refused("session_not_found", "the requested ACP session is no longer live");
            return;
          }
          if (active >= maximumConcurrency) {
            append(
              requestId,
              request,
              "allowed",
              "refused",
              "local delegation concurrency limit reached",
            );
            responder.refused("busy", "the local delegation limit is reached");
            return;
          }
          active += 1;
          let currentResponder = responder;
          let sessionId = "";
          const entry = resolution.entry;
          const environment = scrubbedEnvironment(process.env);
          for (const name of entry.env) {
            const value = process.env[name];
            if (value !== undefined) environment[name] = value;
          }
          const job = startAgentDelegation(agentProcess.value, {
            entry,
            prompt,
            cwd,
            tier: config.tier,
            roots: config.roots,
            curatedExecute: config.curatedExecute ?? [],
            env: environment,
            ...(forgeCredentials !== undefined
              ? { forgeCredentials, forgeOrigin: origin }
              : {}),
            timeoutMs: numberField(
              payload,
              ["timeout_ms", "timeout"],
              maximumTimeoutMillis,
              maximumTimeoutMillis,
            ),
            maximumOutputBytes: numberField(
              payload,
              ["maximum_output_bytes", "max_output_bytes"],
              maximumOutputBytes,
              maximumOutputBytes,
            ),
            onChunk: (text) => currentResponder.chunk(text),
            onSession: (value) => {
              sessionId = value;
              currentResponder.session(value);
            },
            onPermission: (allowed, detail) => {
              append(
                requestId,
                request,
                allowed ? "permission_granted" : "permission_refused",
                "running",
                detail,
              );
            },
          });
          const state = {
            job,
            request,
            setResponder: (value: ComputerAgentResponder) => {
              currentResponder = value;
              if (sessionId !== "") value.session(sessionId);
            },
            get sessionId() {
              return sessionId;
            },
          };
          agentJobs.set(requestId, state);
          append(requestId, request, "allowed", "running", `agent=${entry.id}`);
          void job.done.then((outcome: AgentDelegationOutcome) => {
            active -= 1;
            for (const [key, value] of agentJobs) {
              if (value === state) agentJobs.delete(key);
            }
            const terminal = outcome.status;
            append(
              requestId,
              request,
              "allowed",
              terminal,
              outcome.detail || (outcome.truncated ? "output truncated" : ""),
            );
            currentResponder.exit({
              status: terminal,
              session_id: outcome.sessionId,
              truncated: outcome.truncated,
              duration_ms: outcome.durationMs,
            });
          });
        },
        onCancel: (requestId) => {
          const execution = executions.get(requestId);
          if (execution !== undefined) {
            execution.cancel();
          } else {
            agentJobs.get(requestId)?.job.cancel();
          }
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
