import {
  decodeManagedSandboxSupervisionCommand,
  decodeManagedSandboxSupervisionEnvelope,
  decodeManagedSandboxSupervisionOutcome,
  type ManagedSandboxSupervisionCommand,
  type ManagedSandboxSupervisionEnvelope,
  type ManagedSandboxSupervisionOutcome,
  type ManagedSandboxSupervisionProjection,
} from "@openagentsinc/managed-sandbox-contract";

export const MOBILE_MANAGED_SANDBOX_SUPERVISION_PATH =
  "/api/managed-sandboxes/mobile/supervision" as const;

export type MobileManagedSandboxSnapshot =
  | Readonly<{ state: "available"; envelope: ManagedSandboxSupervisionEnvelope }>
  | Readonly<{ state: "unauthorized" }>
  | Readonly<{ state: "unavailable" }>;

export type MobileManagedSandboxControlAction = "interrupt" | "stop" | "resume" | "delete";

export type MobileManagedSandboxControlResult =
  | Readonly<{ state: "settled"; outcome: ManagedSandboxSupervisionOutcome }>
  | Readonly<{ state: "pending" }>
  | Readonly<{ state: "rejected"; reasonRef: string }>;

export type MobileManagedSandboxFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

export type MobileManagedSandboxOutboxRecord = Readonly<{
  command: ManagedSandboxSupervisionCommand;
  bodyJson: string;
  outcome: ManagedSandboxSupervisionOutcome | null;
}>;

export type MobileManagedSandboxOutbox = Readonly<{
  put: (record: MobileManagedSandboxOutboxRecord) => Promise<void>;
  pending: () => Promise<ReadonlyArray<MobileManagedSandboxOutboxRecord>>;
  settle: (commandRef: string, outcome: ManagedSandboxSupervisionOutcome) => Promise<void>;
  close?: () => void;
}>;

const endpoint = (baseUrl: string): string =>
  `${baseUrl.replace(/\/$/u, "")}${MOBILE_MANAGED_SANDBOX_SUPERVISION_PATH}`;

export const fetchMobileManagedSandboxes = async (
  input: Readonly<{
    baseUrl: string;
    accessToken: string;
    fetchImpl?: MobileManagedSandboxFetch;
  }>,
): Promise<MobileManagedSandboxSnapshot> => {
  const fetchImpl = input.fetchImpl ?? fetch;
  try {
    const response = await fetchImpl(endpoint(input.baseUrl), {
      method: "GET",
      headers: { authorization: `Bearer ${input.accessToken}` },
    });
    if (response.status === 401 || response.status === 403) return { state: "unauthorized" };
    if (!response.ok) return { state: "unavailable" };
    return {
      state: "available",
      envelope: decodeManagedSandboxSupervisionEnvelope(await response.json()),
    };
  } catch {
    return { state: "unavailable" };
  }
};

const actionAllowed = (
  projection: ManagedSandboxSupervisionProjection,
  action: MobileManagedSandboxControlAction,
): boolean => {
  switch (action) {
    case "interrupt":
      return (
        projection.runtime !== null &&
        (projection.runtime.status === "running" || projection.runtime.status === "interrupting")
      );
    case "stop":
      return ["ready", "idle", "running"].includes(projection.state.lifecycle);
    case "resume":
      return projection.state.lifecycle === "stopped";
    case "delete":
      return projection.state.lifecycle !== "deleted" && projection.state.lifecycle !== "deleting";
  }
};

const boundedSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9._:-]/gu, "").slice(0, 120);

export const buildMobileManagedSandboxCommand = (
  input: Readonly<{
    projection: ManagedSandboxSupervisionProjection;
    action: MobileManagedSandboxControlAction;
    invocationRef: string;
    issuedAt: string;
    ttlMillis?: number;
  }>,
): Readonly<
  | { state: "ready"; command: ManagedSandboxSupervisionCommand }
  | { state: "rejected"; reasonRef: string }
> => {
  if (!actionAllowed(input.projection, input.action)) {
    return { state: "rejected", reasonRef: "reason.action_unavailable" };
  }
  const issuedAtMs = Date.parse(input.issuedAt);
  const ttlMillis = input.ttlMillis ?? 60_000;
  const invocationRef = boundedSuffix(input.invocationRef);
  if (
    invocationRef.length < 3 ||
    !Number.isFinite(issuedAtMs) ||
    !Number.isSafeInteger(ttlMillis) ||
    ttlMillis < 1_000 ||
    ttlMillis > 300_000
  ) {
    return { state: "rejected", reasonRef: "reason.invalid_invocation" };
  }
  const tag =
    input.action === "interrupt"
      ? "Interrupt"
      : input.action === "stop"
        ? "Stop"
        : input.action === "resume"
          ? "Resume"
          : "Delete";
  const base = {
    schema: "openagents.managed_sandbox_supervision_command.v1" as const,
    _tag: tag,
    commandRef: `command.mobile.sandbox.${input.action}.${invocationRef}`,
    idempotencyRef: `idempotency.mobile.sandbox.${input.action}.${invocationRef}`,
    surface: "mobile" as const,
    sandboxRef: input.projection.sandboxRef,
    expectedVersion: input.projection.version,
    expectedResourceGeneration: input.projection.resourceGeneration,
    issuedAt: new Date(issuedAtMs).toISOString(),
    expiresAt: new Date(issuedAtMs + ttlMillis).toISOString(),
  };
  try {
    return {
      state: "ready",
      command: decodeManagedSandboxSupervisionCommand(
        tag === "Interrupt"
          ? {
              ...base,
              _tag: tag,
              turnRef: input.projection.runtime?.turnRef,
              reasonRef: "reason.owner_interrupt",
            }
          : tag === "Stop" || tag === "Delete"
            ? { ...base, _tag: tag, reasonRef: `reason.owner_${input.action}` }
            : { ...base, _tag: tag },
      ),
    };
  } catch {
    return { state: "rejected", reasonRef: "reason.invalid_invocation" };
  }
};

export const makeMobileManagedSandboxController = (
  input: Readonly<{
    baseUrl: string;
    accessToken: () => string | null;
    outbox: MobileManagedSandboxOutbox;
    fetchImpl?: MobileManagedSandboxFetch;
    randomId?: () => string;
    now?: () => Date;
  }>,
) => {
  const fetchImpl = input.fetchImpl ?? fetch;
  const randomId = input.randomId ?? (() => Math.random().toString(36).slice(2, 12));
  const now = input.now ?? (() => new Date());

  const flush = async (): Promise<ReadonlyArray<ManagedSandboxSupervisionOutcome>> => {
    const accessToken = input.accessToken();
    if (accessToken === null) return [];
    const settled: ManagedSandboxSupervisionOutcome[] = [];
    for (const record of await input.outbox.pending()) {
      try {
        const response = await fetchImpl(endpoint(input.baseUrl), {
          method: "POST",
          headers: {
            authorization: `Bearer ${accessToken}`,
            "content-type": "application/json",
          },
          body: record.bodyJson,
        });
        if (response.status === 401 || response.status === 403 || response.status >= 500) continue;
        const outcome = decodeManagedSandboxSupervisionOutcome(await response.json());
        await input.outbox.settle(record.command.commandRef, outcome);
        settled.push(outcome);
      } catch {
        // Network and decode faults remain pending. Reconciliation replays the
        // exact stored bytes and the server's native idempotency key prevents
        // a second effect.
      }
    }
    return settled;
  };

  const request = async (
    projection: ManagedSandboxSupervisionProjection,
    action: MobileManagedSandboxControlAction,
  ): Promise<MobileManagedSandboxControlResult> => {
    const built = buildMobileManagedSandboxCommand({
      projection,
      action,
      invocationRef: `tap.${randomId()}`,
      issuedAt: now().toISOString(),
    });
    if (built.state === "rejected") return built;
    const bodyJson = JSON.stringify({ command: built.command });
    await input.outbox.put({ command: built.command, bodyJson, outcome: null });
    const outcomes = await flush();
    const outcome = outcomes.find((candidate) => candidate.commandRef === built.command.commandRef);
    return outcome === undefined ? { state: "pending" } : { state: "settled", outcome };
  };

  return { flush, request };
};
