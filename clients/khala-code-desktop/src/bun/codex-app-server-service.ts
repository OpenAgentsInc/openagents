import { Context, Effect, Layer, Queue, Schema as S, Scope, Stream } from "effect"

import {
  createCodexAppServerHost,
  type CodexAppServerHost,
  type CodexAppServerNotification,
  type CreateCodexAppServerHostOptions,
} from "./codex-app-server-client.js"

type JsonRpcId = number | string

type CodexAppServerRequestOutcome<Result> =
  | { readonly ok: true; readonly value: Result }
  | {
      readonly error: CodexAppServerUnavailable | CodexAppServerRpcFailure | CodexAppServerRpcTimeout
      readonly ok: false
    }

export const CodexAppServerRpcIdSchema = S.Union([S.Number, S.String])
export type CodexAppServerRpcId = typeof CodexAppServerRpcIdSchema.Type

export const CodexAppServerNotificationSchema = S.Struct({
  id: S.optional(CodexAppServerRpcIdSchema),
  method: S.String,
  params: S.Unknown,
  receivedAt: S.String,
})
export type CodexAppServerDecodedNotification = typeof CodexAppServerNotificationSchema.Type

export class CodexAppServerUnavailable extends S.TaggedErrorClass<CodexAppServerUnavailable>()(
  "CodexAppServerUnavailable",
  {
    message: S.String,
  },
) {}

export class CodexAppServerRpcFailure extends S.TaggedErrorClass<CodexAppServerRpcFailure>()(
  "CodexAppServerRpcFailure",
  {
    message: S.String,
    method: S.String,
  },
) {}

export class CodexAppServerRpcTimeout extends S.TaggedErrorClass<CodexAppServerRpcTimeout>()(
  "CodexAppServerRpcTimeout",
  {
    interruptAttempted: S.Boolean,
    interruptOk: S.Boolean,
    message: S.String,
    method: S.String,
    timeoutMs: S.Number,
  },
) {}

export class CodexAppServerDecodeFailure extends S.TaggedErrorClass<CodexAppServerDecodeFailure>()(
  "CodexAppServerDecodeFailure",
  {
    boundary: S.Literals(["response", "notification"]),
    message: S.String,
    method: S.String,
  },
) {}

export class CodexAppServerControlFailure extends S.TaggedErrorClass<CodexAppServerControlFailure>()(
  "CodexAppServerControlFailure",
  {
    action: S.Literals(["start", "stop", "restart"]),
    message: S.String,
  },
) {}

export type CodexAppServerFailure =
  | CodexAppServerUnavailable
  | CodexAppServerRpcFailure
  | CodexAppServerRpcTimeout
  | CodexAppServerDecodeFailure
  | CodexAppServerControlFailure

export type CodexAppServerInterruptOnTimeout = Readonly<{
  readonly threadId: string
  readonly turnId?: string
}>

export type CodexAppServerRequestOptions = Readonly<{
  readonly interruptOnTimeout?: CodexAppServerInterruptOnTimeout
  readonly interruptTimeoutMs?: number
  readonly timeoutMs?: number
}>

export type CodexAppServerServiceShape = Readonly<{
  readonly dispose: Effect.Effect<void>
  readonly notifications: () => Stream.Stream<CodexAppServerDecodedNotification, CodexAppServerDecodeFailure>
  readonly request: <Result = unknown>(
    method: string,
    params?: unknown,
    options?: CodexAppServerRequestOptions,
  ) => Effect.Effect<Result, CodexAppServerUnavailable | CodexAppServerRpcFailure | CodexAppServerRpcTimeout>
  readonly requestDecoded: <Result>(
    schema: S.Decoder<Result>,
    method: string,
    params?: unknown,
    options?: CodexAppServerRequestOptions,
  ) => Effect.Effect<Result, CodexAppServerUnavailable | CodexAppServerRpcFailure | CodexAppServerRpcTimeout | CodexAppServerDecodeFailure>
  readonly respondToServerRequest: (
    id: JsonRpcId,
    result: unknown,
  ) => Effect.Effect<void, CodexAppServerUnavailable | CodexAppServerRpcFailure>
  readonly restart: Effect.Effect<void, CodexAppServerControlFailure>
  readonly start: Effect.Effect<void, CodexAppServerControlFailure>
  readonly status: CodexAppServerHost["status"]
  readonly stop: Effect.Effect<void, CodexAppServerControlFailure>
}>

const errorMessage = (cause: unknown): string =>
  cause instanceof Error ? cause.message : String(cause)

const isTimeout = (method: string, cause: unknown): boolean =>
  errorMessage(cause).includes(`Codex app-server request timed out: ${method}`)

const controlFailure = (
  action: "start" | "stop" | "restart",
  message: string,
): CodexAppServerControlFailure =>
  new CodexAppServerControlFailure({ action, message })

const decodeFailure = (
  boundary: "response" | "notification",
  method: string,
  cause: unknown,
): CodexAppServerDecodeFailure =>
  new CodexAppServerDecodeFailure({
    boundary,
    message: errorMessage(cause),
    method,
  })

const interruptParams = (
  input: CodexAppServerInterruptOnTimeout,
): Record<string, string> => ({
  threadId: input.threadId,
  ...(input.turnId === undefined ? {} : { turnId: input.turnId }),
})

const mapRequestFailure = (
  method: string,
  cause: unknown,
): CodexAppServerUnavailable | CodexAppServerRpcFailure => {
  const message = errorMessage(cause)
  return message === "Codex app-server is not running"
    ? new CodexAppServerUnavailable({ message })
    : new CodexAppServerRpcFailure({ message, method })
}

const decodeNotification = (
  notification: CodexAppServerNotification,
): Effect.Effect<CodexAppServerDecodedNotification, CodexAppServerDecodeFailure> =>
  S.decodeUnknownEffect(CodexAppServerNotificationSchema)(notification).pipe(
    Effect.mapError(cause => decodeFailure("notification", notification.method, cause)),
  )

const requestEffect = <Result>(
  host: CodexAppServerHost,
  method: string,
  params: unknown,
  options: CodexAppServerRequestOptions,
): Effect.Effect<Result, CodexAppServerUnavailable | CodexAppServerRpcFailure | CodexAppServerRpcTimeout> =>
  Effect.promise(async (): Promise<CodexAppServerRequestOutcome<Result>> => {
    try {
      return {
        ok: true as const,
        value: await host.request<Result>(
          method,
          params,
          options.timeoutMs === undefined ? {} : { timeoutMs: options.timeoutMs },
        ),
      }
    } catch (cause) {
      const error = mapRequestFailure(method, cause)
      if (!(error instanceof CodexAppServerRpcFailure) || !isTimeout(method, error.message)) {
        return { error, ok: false as const }
      }
      const interruptOnTimeout = options.interruptOnTimeout
      if (interruptOnTimeout === undefined) {
        return {
          error: new CodexAppServerRpcTimeout({
            interruptAttempted: false,
            interruptOk: false,
            message: error.message,
            method,
            timeoutMs: options.timeoutMs ?? 0,
          }),
          ok: false as const,
        }
      }
      const interruptOk = await host.request(
        "turn/interrupt",
        interruptParams(interruptOnTimeout),
        { timeoutMs: options.interruptTimeoutMs ?? 5_000 },
      ).then(
        () => true,
        () => false,
      )
      return {
        error: new CodexAppServerRpcTimeout({
          interruptAttempted: true,
          interruptOk,
          message: error.message,
          method,
          timeoutMs: options.timeoutMs ?? 0,
        }),
        ok: false as const,
      }
    }
  }).pipe(
    Effect.flatMap(result =>
      result.ok
        ? Effect.succeed(result.value)
        : Effect.fail(result.error)
    ),
  )

export const makeCodexAppServerServiceFromHost = (
  host: CodexAppServerHost,
): CodexAppServerServiceShape => {
  const request: CodexAppServerServiceShape["request"] = <Result = unknown>(
    method: string,
    params: unknown = {},
    options: CodexAppServerRequestOptions = {},
  ) => requestEffect<Result>(host, method, params, options)

  const requestDecoded: CodexAppServerServiceShape["requestDecoded"] = <Result>(
    schema: S.Decoder<Result>,
    method: string,
    params: unknown = {},
    options: CodexAppServerRequestOptions = {},
  ) =>
    request<unknown>(method, params, options).pipe(
      Effect.flatMap(value =>
        S.decodeUnknownEffect(schema)(value).pipe(
          Effect.mapError(cause => decodeFailure("response", method, cause)),
        )
      ),
    )

  const notifications = (): Stream.Stream<CodexAppServerDecodedNotification, CodexAppServerDecodeFailure> =>
    Stream.unwrap(
      Effect.map(Queue.unbounded<CodexAppServerNotification>(), queue => {
        const unsubscribe = host.subscribe(notification => {
          Queue.offerUnsafe(queue, notification)
        })
        return Stream.fromQueue(queue).pipe(
          Stream.mapEffect(decodeNotification),
          Stream.ensuring(Effect.all([
            Effect.sync(unsubscribe),
            Queue.shutdown(queue),
          ], { discard: true })),
        )
      }),
    )

  const control = (
    action: "start" | "stop" | "restart",
    run: () => Promise<{ readonly ok: boolean; readonly error?: string | undefined }>,
  ): Effect.Effect<void, CodexAppServerControlFailure> =>
    Effect.tryPromise({
      catch: cause => controlFailure(action, errorMessage(cause)),
      try: run,
    }).pipe(
      Effect.flatMap(result =>
        result.ok
          ? Effect.void
          : Effect.fail(controlFailure(action, result.error ?? `Codex app-server ${action} failed`))
      ),
    )

  return {
    dispose: Effect.sync(() => host.dispose()),
    notifications,
    request,
    requestDecoded,
    respondToServerRequest: (id, result) =>
      Effect.try({
        catch: cause => mapRequestFailure("respondToServerRequest", cause),
        try: () => host.respondToServerRequest(id, result),
      }),
    restart: control("restart", host.restart),
    start: control("start", host.start),
    status: host.status,
    stop: control("stop", host.stop),
  }
}

export const makeCodexAppServerService = (
  options: CreateCodexAppServerHostOptions = {},
): CodexAppServerServiceShape =>
  makeCodexAppServerServiceFromHost(createCodexAppServerHost(options))

export const makeCodexAppServerScoped = (
  options: CreateCodexAppServerHostOptions = {},
): Effect.Effect<CodexAppServerServiceShape, never, Scope.Scope> =>
  Effect.acquireRelease(
    Effect.sync(() => makeCodexAppServerService(options)),
    service => service.dispose,
  )

export class CodexAppServer extends Context.Service<CodexAppServer, CodexAppServerServiceShape>()(
  "CodexAppServer",
  { make: Effect.sync(makeCodexAppServerService) },
) {}

export const CodexAppServerLive = (
  options: CreateCodexAppServerHostOptions = {},
) =>
  Layer.effect(CodexAppServer, makeCodexAppServerScoped(options))
