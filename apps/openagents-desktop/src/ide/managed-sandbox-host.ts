import { mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs"
import path from "node:path"

import { Context, Effect, Exit, Layer, Schema, Scope } from "effect"

import type { DesktopSessionCredential } from "../desktop-session-vault.ts"
import type { IdeAgentCodeHost } from "./agent-code-host.ts"
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
} from "./managed-sandbox-contract.ts"
import {
  IdeManagedSandboxService,
  makeIdeManagedSandboxLayer,
  type IdeManagedSandboxGateway,
} from "./managed-sandbox-service.ts"
import { IdeTimestampSchema } from "./project-contract.ts"

export type IdeManagedSandboxHost = Readonly<{
  ownerRef: string | null
  snapshot: () => Promise<IdeManagedSandboxSnapshot>
  command: (value: unknown) => Promise<IdeManagedSandboxCommandResult>
  dispose: () => Promise<void>
}>

const safeMessage = (message: string): string =>
  message.trim().slice(0, 800) || "The managed-sandbox request was refused."

const unavailableSnapshot = (reason: string, now: () => Date): IdeManagedSandboxSnapshot =>
  IdeManagedSandboxSnapshotSchema.make({
    ...emptyIdeManagedSandboxSnapshot(now().toISOString()),
    admission: {
      _tag: "Unavailable",
      reason: safeMessage(reason),
      checkedAt: IdeTimestampSchema.make(now().toISOString()),
    },
    lastError: safeMessage(reason),
  })

const loadSnapshot = (persistencePath: string | null, now: () => Date): IdeManagedSandboxSnapshot => {
  if (persistencePath === null) return emptyIdeManagedSandboxSnapshot(now().toISOString())
  try {
    return (
      decodeIdeManagedSandboxSnapshot(JSON.parse(readFileSync(persistencePath, "utf8"))) ??
      unavailableSnapshot("Persisted managed-sandbox state is invalid.", now)
    )
  } catch (error) {
    return (error as NodeJS.ErrnoException | null)?.code === "ENOENT"
      ? emptyIdeManagedSandboxSnapshot(now().toISOString())
      : unavailableSnapshot("Persisted managed-sandbox state is unavailable.", now)
  }
}

const persistSnapshot = (persistencePath: string | null, snapshot: IdeManagedSandboxSnapshot): void => {
  if (persistencePath === null) return
  mkdirSync(path.dirname(persistencePath), { recursive: true, mode: 0o700 })
  const temporary = `${persistencePath}.${process.pid}.${snapshot.revision}.tmp`
  try {
    writeFileSync(temporary, JSON.stringify(snapshot), {
      encoding: "utf8",
      flag: "wx",
      mode: 0o600,
    })
    renameSync(temporary, persistencePath)
  } catch {
    try {
      unlinkSync(temporary)
    } catch {
      // Best-effort temporary cleanup.
    }
  }
}

const fetchJson = (
  fetchImpl: typeof fetch,
  url: string,
  accessToken: string,
  body: unknown,
): Effect.Effect<unknown, Error> =>
  Effect.tryPromise({
    try: async () => {
      const response = await fetchImpl(url, {
        method: "POST",
        headers: {
          authorization: `Bearer ${accessToken}`,
          "content-type": "application/json",
          accept: "application/json",
        },
        body: JSON.stringify(body),
      })
      const text = await response.text()
      if (text.length > 2 * 1024 * 1024) throw new Error("managed-sandbox response exceeded 2 MiB")
      if (!response.ok) throw new Error(`managed-sandbox service returned HTTP ${response.status}`)
      return JSON.parse(text)
    },
    catch: () => new Error("managed-sandbox service is unavailable"),
  })

export const makeIdeManagedSandboxHttpGateway = (
  input: Readonly<{
    baseUrl: string
    accessToken: string
    fetchImpl?: typeof fetch
  }>,
): IdeManagedSandboxGateway => {
  const fetchImpl = input.fetchImpl ?? fetch
  const baseUrl = input.baseUrl.replace(/\/+$/u, "")
  return {
    admission: ({ attachment }) =>
      fetchJson(fetchImpl, `${baseUrl}/api/managed-sandboxes/desktop/admission`, input.accessToken, {
        schemaVersion: "openagents.desktop.ide-managed-sandbox.v1",
        attachment,
      }).pipe(
        Effect.flatMap((value) =>
          Effect.try({
            try: () => {
              const result = value
              if (typeof result !== "object" || result === null || !("admission" in result)) {
                throw new Error("missing admission")
              }
              return result.admission
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
      fetchJson(fetchImpl, `${baseUrl}/api/managed-sandboxes/desktop/commands`, input.accessToken, {
        schemaVersion: "openagents.desktop.ide-managed-sandbox.v1",
        command,
        ...(options?.prompt === undefined ? {} : { prompt: options.prompt }),
        ...(options?.attachmentGeneration === undefined
          ? {}
          : { attachmentGeneration: options.attachmentGeneration }),
      }).pipe(
        Effect.flatMap((value) =>
          Effect.try({
            try: () => {
              const result = value
              if (typeof result !== "object" || result === null || !("result" in result)) {
                throw new Error("missing result")
              }
              return result.result
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
  }
}

const fallbackHost = (
  reason: Extract<IdeManagedSandboxCommandResult, { _tag: "Refused" }>["reason"],
  message: string,
  now: () => Date,
): IdeManagedSandboxHost => {
  const snapshot = unavailableSnapshot(message, now)
  return {
    ownerRef: null,
    snapshot: async () => snapshot,
    command: async () =>
      IdeManagedSandboxCommandResultSchema.cases.Refused.make({
        reason,
        message: safeMessage(message),
        snapshot,
      }),
    dispose: async () => undefined,
  }
}

export const openIdeManagedSandboxHost = async (
  input: Readonly<{
    enabled: boolean
    credential: () => DesktopSessionCredential | null
    baseUrl: string
    agentCodeHost: IdeAgentCodeHost
    persistencePath?: string | null
    fetchImpl?: typeof fetch
    now?: () => Date
  }>,
): Promise<IdeManagedSandboxHost> => {
  const now = input.now ?? (() => new Date())
  if (!input.enabled)
    return fallbackHost("not_configured", "OpenAgents-managed placement is default-off in this Desktop build.", now)
  const credential = input.credential()
  if (credential === null) return fallbackHost("signed_out", "Sign in before using OpenAgents-managed placement.", now)
  const persistencePath = input.persistencePath ?? null
  const initialSnapshot = loadSnapshot(persistencePath, now)
  const gateway = makeIdeManagedSandboxHttpGateway({
    baseUrl: input.baseUrl,
    accessToken: credential.accessToken,
    fetchImpl: input.fetchImpl,
  })
  const scope = await Effect.runPromise(Scope.make())
  const layer = makeIdeManagedSandboxLayer({
    principal: {
      ownerRef: credential.ownerUserId,
      tenantRef: credential.ownerUserId,
      requestedByRef: "principal.desktop",
    },
    gateway,
    currentAgentSnapshot: () =>
      Effect.tryPromise({
        try: () => input.agentCodeHost.snapshot(),
        catch: () => new Error("agent graph unavailable"),
      }),
    initialSnapshot,
  })
  const context = await Effect.runPromise(Layer.buildWithScope(layer, scope))
  const service = Context.get(context, IdeManagedSandboxService)
  let disposed = false

  const snapshot = async (): Promise<IdeManagedSandboxSnapshot> => {
    if (disposed) return unavailableSnapshot("The managed-sandbox scope is closed.", now)
    return Effect.runPromise(service.snapshot()).catch(() =>
      unavailableSnapshot("The managed-sandbox snapshot is unavailable.", now),
    )
  }

  const refused = async (
    reason: Extract<IdeManagedSandboxCommandResult, { _tag: "Refused" }>["reason"],
    message: string,
  ): Promise<IdeManagedSandboxCommandResult> =>
    IdeManagedSandboxCommandResultSchema.cases.Refused.make({
      reason,
      message: safeMessage(message),
      snapshot: await snapshot(),
    })

  const command = async (value: unknown): Promise<IdeManagedSandboxCommandResult> => {
    if (disposed) return refused("gateway_unavailable", "The managed-sandbox scope is closed.")
    const decoded = decodeIdeManagedSandboxCommand(value)
    if (decoded === null)
      return refused("invalid_input", "The managed-sandbox command did not match the schema boundary.")
    const settled = await Effect.runPromise(
      service.command(decoded).pipe(
        Effect.match({
          onFailure: (error) => ({ ok: false as const, error }),
          onSuccess: (next) => ({ ok: true as const, next }),
        }),
      ),
    )
    if (!settled.ok) return refused(settled.error.reason, settled.error.message)
    persistSnapshot(persistencePath, settled.next)
    return IdeManagedSandboxCommandResultSchema.cases.Succeeded.make({ snapshot: settled.next })
  }

  const dispose = async (): Promise<void> => {
    if (disposed) return
    persistSnapshot(persistencePath, await snapshot())
    disposed = true
    await Effect.runPromise(Scope.close(scope, Exit.void))
  }

  return { ownerRef: credential.ownerUserId, snapshot, command, dispose }
}
