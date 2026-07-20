import { mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";

import { Context, Effect, Exit, Layer, Schema, Scope } from "effect";

import type { DesktopSessionCredential } from "../desktop-session-vault.ts";
import type { IdeAgentCodeHost } from "./agent-code-host.ts";
import {
  IdeManagedSandboxAdmissionSchema,
  IdeManagedSandboxCommandResultSchema,
  IdeManagedSandboxGatewayResultSchema,
  IdeManagedSandboxSnapshotSchema,
  decodeIdeManagedSandboxCommand,
  decodeIdeManagedSandboxSnapshot,
  emptyIdeManagedSandboxSnapshot,
  type IdeManagedSandboxCommandResult,
  type IdeManagedSandboxSnapshot,
} from "./managed-sandbox-contract.ts";
import {
  IdeManagedSandboxService,
  makeIdeManagedSandboxLayer,
  type IdeManagedSandboxGateway,
} from "./managed-sandbox-service.ts";
import type { IdePortableMutationAuthority } from "./portable-mutation-authority.ts";
import { IdeTimestampSchema } from "./project-contract.ts";

export type IdeManagedSandboxHost = Readonly<{
  ownerRef: string | null;
  snapshot: () => Promise<IdeManagedSandboxSnapshot>;
  command: (value: unknown) => Promise<IdeManagedSandboxCommandResult>;
  quiesce: () => Promise<void>;
  dispose: () => Promise<void>;
}>;

const safeMessage = (message: string): string =>
  message.trim().slice(0, 800) || "The managed-sandbox request was refused.";

const unavailableSnapshot = (reason: string, now: () => Date): IdeManagedSandboxSnapshot =>
  IdeManagedSandboxSnapshotSchema.make({
    ...emptyIdeManagedSandboxSnapshot(now().toISOString()),
    admission: {
      _tag: "Unavailable",
      reason: safeMessage(reason),
      checkedAt: IdeTimestampSchema.make(now().toISOString()),
    },
    lastError: safeMessage(reason),
  });

const loadSnapshot = (
  persistencePath: string | null,
  now: () => Date,
): IdeManagedSandboxSnapshot => {
  if (persistencePath === null) return emptyIdeManagedSandboxSnapshot(now().toISOString());
  try {
    return (
      decodeIdeManagedSandboxSnapshot(JSON.parse(readFileSync(persistencePath, "utf8"))) ??
      unavailableSnapshot("Persisted managed-sandbox state is invalid.", now)
    );
  } catch (error) {
    return (error as NodeJS.ErrnoException | null)?.code === "ENOENT"
      ? emptyIdeManagedSandboxSnapshot(now().toISOString())
      : unavailableSnapshot("Persisted managed-sandbox state is unavailable.", now);
  }
};

const persistSnapshot = (
  persistencePath: string | null,
  snapshot: IdeManagedSandboxSnapshot,
): void => {
  if (persistencePath === null) return;
  mkdirSync(path.dirname(persistencePath), { recursive: true, mode: 0o700 });
  const temporary = `${persistencePath}.${process.pid}.${snapshot.revision}.tmp`;
  try {
    writeFileSync(temporary, JSON.stringify(snapshot), {
      encoding: "utf8",
      flag: "wx",
      mode: 0o600,
    });
    renameSync(temporary, persistencePath);
  } catch {
    try {
      unlinkSync(temporary);
    } catch {
      // Best-effort temporary cleanup.
    }
  }
};

const fetchJson = (
  fetchImpl: typeof fetch,
  url: string,
  accessToken: string,
  body: unknown,
  signal?: AbortSignal,
): Effect.Effect<unknown, Error> =>
  Effect.tryPromise({
    try: async (effectSignal) => {
      const controller = new AbortController();
      const abort = () => controller.abort();
      effectSignal.addEventListener("abort", abort, { once: true });
      signal?.addEventListener("abort", abort, { once: true });
      try {
        const response = await fetchImpl(url, {
          method: "POST",
          headers: {
            authorization: `Bearer ${accessToken}`,
            "content-type": "application/json",
            accept: "application/json",
          },
          body: JSON.stringify(body),
          signal: controller.signal,
        });
        const text = await response.text();
        if (text.length > 2 * 1024 * 1024)
          throw new Error("managed-sandbox response exceeded 2 MiB");
        if (!response.ok)
          throw new Error(`managed-sandbox service returned HTTP ${response.status}`);
        return JSON.parse(text);
      } finally {
        effectSignal.removeEventListener("abort", abort);
        signal?.removeEventListener("abort", abort);
      }
    },
    catch: () => new Error("managed-sandbox service is unavailable"),
  });

export const makeIdeManagedSandboxHttpGateway = (
  input: Readonly<{
    baseUrl: string;
    accessToken: string;
    fetchImpl?: typeof fetch;
  }>,
): IdeManagedSandboxGateway => {
  const fetchImpl = input.fetchImpl ?? fetch;
  const baseUrl = input.baseUrl.replace(/\/+$/u, "");
  return {
    admission: ({ attachment }, options) =>
      fetchJson(
        fetchImpl,
        `${baseUrl}/api/managed-sandboxes/desktop/admission`,
        input.accessToken,
        {
          schemaVersion: "openagents.desktop.ide-managed-sandbox.v1",
          attachment,
        },
        options?.signal,
      ).pipe(
        Effect.flatMap((value) =>
          Effect.try({
            try: () => {
              const result = value;
              if (typeof result !== "object" || result === null || !("admission" in result)) {
                throw new Error("missing admission");
              }
              return result.admission;
            },
            catch: () => new Error("invalid managed-sandbox admission response"),
          }),
        ),
        Effect.flatMap((value) =>
          Schema.decodeUnknownEffect(IdeManagedSandboxAdmissionSchema)(value).pipe(
            Effect.mapError(() => new Error("invalid managed-sandbox admission response")),
          ),
        ),
      ),
    execute: (command, options) =>
      fetchJson(
        fetchImpl,
        `${baseUrl}/api/managed-sandboxes/desktop/commands`,
        input.accessToken,
        {
          schemaVersion: "openagents.desktop.ide-managed-sandbox.v1",
          command,
          ...(options?.prompt === undefined ? {} : { prompt: options.prompt }),
          ...(options?.attachmentGeneration === undefined
            ? {}
            : { attachmentGeneration: options.attachmentGeneration }),
        },
        options?.signal,
      ).pipe(
        Effect.flatMap((value) =>
          Effect.try({
            try: () => {
              const result = value;
              if (typeof result !== "object" || result === null || !("result" in result)) {
                throw new Error("missing result");
              }
              return result.result;
            },
            catch: () => new Error("invalid managed-sandbox command response"),
          }),
        ),
        Effect.flatMap((value) =>
          Schema.decodeUnknownEffect(IdeManagedSandboxGatewayResultSchema)(value).pipe(
            Effect.mapError(() => new Error("invalid managed-sandbox command response")),
          ),
        ),
      ),
  };
};

const fallbackHost = (
  reason: Extract<IdeManagedSandboxCommandResult, { _tag: "Refused" }>["reason"],
  message: string,
  now: () => Date,
): IdeManagedSandboxHost => {
  const snapshot = unavailableSnapshot(message, now);
  return {
    ownerRef: null,
    snapshot: async () => snapshot,
    command: async () =>
      IdeManagedSandboxCommandResultSchema.cases.Refused.make({
        reason,
        message: safeMessage(message),
        snapshot,
      }),
    quiesce: async () => undefined,
    dispose: async () => undefined,
  };
};

export const openIdeManagedSandboxHost = async (
  input: Readonly<{
    enabled: boolean;
    credential: () => DesktopSessionCredential | null;
    baseUrl: string;
    agentCodeHost: IdeAgentCodeHost;
    mutationAuthority: IdePortableMutationAuthority;
    persistencePath?: string | null;
    gateway?: IdeManagedSandboxGateway;
    fetchImpl?: typeof fetch;
    now?: () => Date;
  }>,
): Promise<IdeManagedSandboxHost> => {
  const now = input.now ?? (() => new Date());
  if (!input.enabled)
    return fallbackHost(
      "not_configured",
      "OpenAgents-managed placement is default-off in this Desktop build.",
      now,
    );
  const credential = input.credential();
  if (credential === null)
    return fallbackHost("signed_out", "Sign in before using OpenAgents-managed placement.", now);
  const persistencePath = input.persistencePath ?? null;
  const initialSnapshot = loadSnapshot(persistencePath, now);
  const gateway =
    input.gateway ??
    makeIdeManagedSandboxHttpGateway({
      baseUrl: input.baseUrl,
      accessToken: credential.accessToken,
      fetchImpl: input.fetchImpl,
    });
  const scope = await Effect.runPromise(Scope.make());
  const layer = makeIdeManagedSandboxLayer({
    principal: {
      ownerRef: credential.ownerUserId,
      tenantRef: credential.ownerUserId,
      requestedByRef: "principal.desktop",
    },
    gateway,
    mutationAuthority: input.mutationAuthority,
    currentAgentSnapshot: () =>
      Effect.tryPromise({
        try: () => input.agentCodeHost.snapshot(),
        catch: () => new Error("agent graph unavailable"),
      }),
    initialSnapshot,
  });
  const context = await Effect.runPromise(Layer.buildWithScope(layer, scope));
  const service = Context.get(context, IdeManagedSandboxService);
  let disposed = false;
  let quiesced = false;
  let quiescePromise: Promise<void> | null = null;
  let disposePromise: Promise<void> | null = null;

  const snapshot = async (): Promise<IdeManagedSandboxSnapshot> => {
    if (disposed) return unavailableSnapshot("The managed-sandbox scope is closed.", now);
    return Effect.runPromise(service.snapshot()).catch(() =>
      unavailableSnapshot("The managed-sandbox snapshot is unavailable.", now),
    );
  };

  const refused = async (
    reason: Extract<IdeManagedSandboxCommandResult, { _tag: "Refused" }>["reason"],
    message: string,
  ): Promise<IdeManagedSandboxCommandResult> =>
    IdeManagedSandboxCommandResultSchema.cases.Refused.make({
      reason,
      message: safeMessage(message),
      snapshot: await snapshot(),
    });

  const command = async (value: unknown): Promise<IdeManagedSandboxCommandResult> => {
    if (disposed || quiesced)
      return refused("gateway_unavailable", "The managed-sandbox scope is closed.");
    const decoded = decodeIdeManagedSandboxCommand(value);
    if (decoded === null)
      return refused(
        "invalid_input",
        "The managed-sandbox command did not match the schema boundary.",
      );
    const settled = await Effect.runPromise(
      service.command(decoded).pipe(
        Effect.match({
          onFailure: (error) => ({ ok: false as const, error }),
          onSuccess: (next) => ({ ok: true as const, next }),
        }),
      ),
    );
    if (!settled.ok) return refused(settled.error.reason, settled.error.message);
    if (disposed || quiesced)
      return refused("gateway_unavailable", "The managed-sandbox scope is closed.");
    persistSnapshot(persistencePath, settled.next);
    return IdeManagedSandboxCommandResultSchema.cases.Succeeded.make({ snapshot: settled.next });
  };

  const quiesce = (): Promise<void> => {
    if (quiescePromise !== null) return quiescePromise;
    quiesced = true;
    quiescePromise = Effect.runPromise(service.quiesce());
    return quiescePromise;
  };

  const dispose = (): Promise<void> => {
    if (disposePromise !== null) return disposePromise;
    disposed = true;
    disposePromise = (async () => {
      await quiesce();
      const finalSnapshot = await Effect.runPromise(service.snapshot()).catch(() => null);
      if (finalSnapshot !== null) persistSnapshot(persistencePath, finalSnapshot);
      await Effect.runPromise(Scope.close(scope, Exit.void));
    })();
    return disposePromise;
  };

  return { ownerRef: credential.ownerUserId, snapshot, command, quiesce, dispose };
};
