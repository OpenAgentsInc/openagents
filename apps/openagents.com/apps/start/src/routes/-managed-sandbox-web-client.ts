import {
  decodeManagedSandboxSupervisionCommand,
  decodeManagedSandboxSupervisionEnvelope,
  decodeManagedSandboxSupervisionOutcome,
  type ManagedSandboxSupervisionCommand,
  type ManagedSandboxSupervisionEnvelope,
  type ManagedSandboxSupervisionOutcome,
  type ManagedSandboxSupervisionProjection,
} from "@openagentsinc/managed-sandbox-contract";

import { MANAGED_SANDBOX_WEB_PROXY_PATH } from "../managed-sandbox-proxy";

export type WebManagedSandboxAction = "interrupt" | "stop" | "resume" | "delete";
export type WebManagedSandboxFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

type StorageLike = Pick<Storage, "getItem" | "setItem">;

type StoredCommand = Readonly<{
  command: ManagedSandboxSupervisionCommand;
  bodyJson: string;
}>;

export const WEB_MANAGED_SANDBOX_OUTBOX_KEY = "openagents.managed-sandbox.web-outbox.v1" as const;

const readRows = (storage: StorageLike): ReadonlyArray<StoredCommand> => {
  try {
    const parsed: unknown = JSON.parse(storage.getItem(WEB_MANAGED_SANDBOX_OUTBOX_KEY) ?? "[]");
    if (!Array.isArray(parsed)) return [];
    return parsed.flatMap((row) => {
      if (typeof row !== "object" || row === null) return [];
      const bodyJson = (row as { bodyJson?: unknown }).bodyJson;
      if (typeof bodyJson !== "string") return [];
      try {
        const body = JSON.parse(bodyJson) as { command?: unknown };
        const command = decodeManagedSandboxSupervisionCommand(body.command);
        return command.surface === "web" ? [{ command, bodyJson }] : [];
      } catch {
        return [];
      }
    });
  } catch {
    return [];
  }
};

const writeRows = (storage: StorageLike, rows: ReadonlyArray<StoredCommand>): void => {
  storage.setItem(WEB_MANAGED_SANDBOX_OUTBOX_KEY, JSON.stringify(rows));
};

export const makeWebManagedSandboxOutbox = (storage: StorageLike) => ({
  pending: (): ReadonlyArray<StoredCommand> => readRows(storage),
  put: (row: StoredCommand): void => {
    const rows = readRows(storage);
    const existing = rows.find(
      (candidate) => candidate.command.commandRef === row.command.commandRef,
    );
    if (existing !== undefined && existing.bodyJson !== row.bodyJson) {
      throw new Error("managed-sandbox web command ref is bound to different bytes");
    }
    if (existing === undefined) writeRows(storage, [...rows, row]);
  },
  settle: (commandRef: string): void => {
    writeRows(
      storage,
      readRows(storage).filter((row) => row.command.commandRef !== commandRef),
    );
  },
});

const legal = (
  projection: ManagedSandboxSupervisionProjection,
  action: WebManagedSandboxAction,
): boolean => {
  if (action === "interrupt") {
    return (
      projection.runtime !== null &&
      (projection.runtime.status === "running" || projection.runtime.status === "interrupting")
    );
  }
  if (action === "stop") return ["ready", "idle", "running"].includes(projection.state.lifecycle);
  if (action === "resume") return projection.state.lifecycle === "stopped";
  return projection.state.lifecycle !== "deleted" && projection.state.lifecycle !== "deleting";
};

export const buildWebManagedSandboxCommand = (
  input: Readonly<{
    projection: ManagedSandboxSupervisionProjection;
    action: WebManagedSandboxAction;
    invocationRef: string;
    issuedAt: string;
  }>,
): ManagedSandboxSupervisionCommand | null => {
  if (!legal(input.projection, input.action)) return null;
  const issuedAt = Date.parse(input.issuedAt);
  const suffix = input.invocationRef.replace(/[^A-Za-z0-9._:-]/gu, "").slice(0, 120);
  if (!Number.isFinite(issuedAt) || suffix.length < 3) return null;
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
    commandRef: `command.web.sandbox.${input.action}.${suffix}`,
    idempotencyRef: `idempotency.web.sandbox.${input.action}.${suffix}`,
    surface: "web" as const,
    sandboxRef: input.projection.sandboxRef,
    expectedVersion: input.projection.version,
    expectedResourceGeneration: input.projection.resourceGeneration,
    issuedAt: new Date(issuedAt).toISOString(),
    expiresAt: new Date(issuedAt + 60_000).toISOString(),
  };
  try {
    return decodeManagedSandboxSupervisionCommand(
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
    );
  } catch {
    return null;
  }
};

export const fetchWebManagedSandboxEnvelope = async (
  fetchImpl: WebManagedSandboxFetch = fetch,
): Promise<ManagedSandboxSupervisionEnvelope> => {
  const response = await fetchImpl(MANAGED_SANDBOX_WEB_PROXY_PATH, { method: "GET" });
  if (!response.ok) throw new Error(`managed sandbox supervision unavailable (${response.status})`);
  return decodeManagedSandboxSupervisionEnvelope(await response.json());
};

export const makeWebManagedSandboxController = (
  input: Readonly<{
    storage: StorageLike;
    fetchImpl?: WebManagedSandboxFetch;
    randomId?: () => string;
    now?: () => Date;
  }>,
) => {
  const outbox = makeWebManagedSandboxOutbox(input.storage);
  const fetchImpl = input.fetchImpl ?? fetch;
  const randomId = input.randomId ?? (() => Math.random().toString(36).slice(2, 12));
  const now = input.now ?? (() => new Date());

  const flush = async (): Promise<ReadonlyArray<ManagedSandboxSupervisionOutcome>> => {
    const outcomes: ManagedSandboxSupervisionOutcome[] = [];
    for (const row of outbox.pending()) {
      try {
        const response = await fetchImpl(MANAGED_SANDBOX_WEB_PROXY_PATH, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: row.bodyJson,
        });
        if (response.status === 401 || response.status === 403 || response.status >= 500) continue;
        const outcome = decodeManagedSandboxSupervisionOutcome(await response.json());
        outbox.settle(row.command.commandRef);
        outcomes.push(outcome);
      } catch {
        // Offline and decode failures retain the exact command bytes. The
        // next flush replays them under the same native idempotency key.
      }
    }
    return outcomes;
  };

  const request = async (
    projection: ManagedSandboxSupervisionProjection,
    action: WebManagedSandboxAction,
  ): Promise<ManagedSandboxSupervisionOutcome | null> => {
    const command = buildWebManagedSandboxCommand({
      projection,
      action,
      invocationRef: `click.${randomId()}`,
      issuedAt: now().toISOString(),
    });
    if (command === null) return null;
    const row = { command, bodyJson: JSON.stringify({ command }) };
    outbox.put(row);
    const outcomes = await flush();
    return outcomes.find((outcome) => outcome.commandRef === command.commandRef) ?? null;
  };

  return { flush, request, pending: outbox.pending };
};
