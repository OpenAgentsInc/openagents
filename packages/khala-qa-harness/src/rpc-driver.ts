import { Effect, Stream } from "effect"

import {
  KhalaCodeRpcClient,
  KhalaCodeRpcMethodNames,
  type KhalaCodeRpcCallOk,
  type KhalaCodeRpcClientOptions,
  type KhalaCodeRpcMethodName,
} from "./rpc-client.js"
import {
  khalaCodeQaDriverFailure,
  type KhalaCodeQaAppEvent,
  type KhalaCodeQaAppHandle,
  type KhalaCodeQaArtifacts,
  type KhalaCodeQaBootOptions,
  type KhalaCodeQaDriver,
  type KhalaCodeQaDriverFailure,
  type KhalaCodeQaMetricsSnapshot,
  type KhalaCodeQaObservation,
  type KhalaCodeQaStateSnapshot,
} from "./driver.js"
import { projectKhalaCodeQaModeState } from "./mode-projection.js"
import type { KhalaCodeQaAction } from "./scenario.js"

const isRpcMethodName = (method: string): method is KhalaCodeRpcMethodName =>
  (KhalaCodeRpcMethodNames as ReadonlyArray<string>).includes(method)

const errorMessage = (cause: unknown): string =>
  cause instanceof Error
    ? cause.message
    : typeof cause === "string"
      ? cause
      : (() => {
        try {
          return JSON.stringify(cause)
        } catch {
          return String(cause)
        }
      })()

export type KhalaCodeRpcQaDriverOptions = KhalaCodeRpcClientOptions & {
  readonly mode?: "rpc" | "dom"
  readonly now?: () => string
}

export class KhalaCodeRpcQaDriver implements KhalaCodeQaDriver {
  readonly mode: "rpc" | "dom"
  readonly client: KhalaCodeRpcClient
  readonly now: () => string
  private handle: KhalaCodeQaAppHandle | undefined
  private observations: KhalaCodeQaObservation[] = []
  private rpcOracleByQuery = new Map<string, KhalaCodeRpcCallOk<KhalaCodeRpcMethodName>>()
  private rpcCallCounts = new Map<KhalaCodeRpcMethodName, number>()

  constructor(options: KhalaCodeRpcQaDriverOptions = {}) {
    this.client = new KhalaCodeRpcClient(options)
    this.mode = options.mode ?? "rpc"
    this.now = options.now ?? (() => new Date().toISOString())
  }

  boot(
    opts: KhalaCodeQaBootOptions,
  ): Effect.Effect<KhalaCodeQaAppHandle, KhalaCodeQaDriverFailure> {
    return Effect.sync(() => {
      this.handle = {
        backend: opts.backend,
        mode: this.mode,
        startedAt: this.now(),
      }
      return this.handle
    })
  }

  act(action: KhalaCodeQaAction): Effect.Effect<KhalaCodeQaObservation, KhalaCodeQaDriverFailure> {
    if (action.kind === "boot") {
      const bootOptions = {
        backend: action.backend ?? this.handle?.backend ?? "fixture",
        ...(action.headless === undefined ? {} : { headless: action.headless }),
      }
      return Effect.map(
        this.boot(bootOptions),
        (handle) => this.record({
          action,
          data: handle,
          label: "boot",
          ok: true,
        }),
      )
    }

    if (action.kind === "read" && action.query.startsWith("screenshot:")) {
      return Effect.succeed(
        this.record({
          action,
          data: { value: { backend: this.handle?.backend ?? "fixture", screenshot: action.query.slice("screenshot:".length) } },
          label: `read:${action.query}`,
          ok: true,
        }),
      )
    }

    if (action.kind === "read") {
      return Effect.map(this.read(action.query), (snapshot) =>
        this.record({
          action,
          data: snapshot,
          label: `read:${action.query}`,
          ok: true,
        }),
      )
    }

    if (action.kind !== "rpc_call") {
      // RPC-tier monkey runs record UI intents for coverage only; DOM/browser
      // tiers own selector existence, layout, and interaction side effects.
      if (["click", "hotbar", "slash_command", "approve", "type", "submit_composer", "wait_for", "thread_select"].includes(action.kind)) {
        return Effect.succeed(
          this.record({
            action,
            data: { target: action.target, text: action.text, value: action.value },
            label: `${action.kind}:${action.target ?? action.value ?? ""}`,
            ok: true,
          }),
        )
      }
      return Effect.fail(
        khalaCodeQaDriverFailure(`RPC QA driver does not support ${action.kind} actions`, { action }),
      )
    }
    if (!isRpcMethodName(action.method)) {
      return Effect.fail(
        khalaCodeQaDriverFailure(`Unknown Khala Code RPC method: ${action.method}`, { action }),
      )
    }

    const occurrence = (this.rpcCallCounts.get(action.method) ?? 0) + 1
    this.rpcCallCounts.set(action.method, occurrence)
    const modePrefix = this.mode === "rpc" ? "rpc" : "dom:rpc"
    const occurrenceLabel = `${modePrefix}:${action.method}#${occurrence}`
    const callWithOracle = this.client.callWithOracle.bind(this.client) as (
      method: KhalaCodeRpcMethodName,
      ...args: ReadonlyArray<unknown>
    ) => Effect.Effect<KhalaCodeRpcCallOk<KhalaCodeRpcMethodName>, unknown>
    return Effect.matchEffect(
      callWithOracle(
        action.method,
        ...(action.args ?? []),
      ),
      {
        onFailure: (cause) =>
          Effect.succeed(
            this.record({
              action,
              error: errorMessage(cause),
              label: occurrenceLabel,
              ok: false,
            }),
          ),
        onSuccess: (result) =>
          Effect.sync(() => {
            this.rpcOracleByQuery.set(`rpc:${action.method}`, result)
            this.rpcOracleByQuery.set(`${this.mode}:rpc:${action.method}`, result)
            this.rpcOracleByQuery.set(action.method, result)
            this.rpcOracleByQuery.set(occurrenceLabel, result)
            return this.record({
              action,
              data: result,
              label: occurrenceLabel,
              ok: true,
            })
          }),
      },
    )
  }

  read(query: string): Effect.Effect<KhalaCodeQaStateSnapshot, KhalaCodeQaDriverFailure> {
    const observed = this.rpcOracleByQuery.get(query)
    if (observed !== undefined) {
      return Effect.succeed({ label: query, value: observed.value })
    }
    const projected = projectKhalaCodeQaModeState(query, (method) =>
      this.rpcOracleByQuery.get(method)?.value ??
      this.rpcOracleByQuery.get(`rpc:${method}`)?.value ??
      this.rpcOracleByQuery.get(`${this.mode}:rpc:${method}`)?.value
    )
    if (projected !== undefined) {
      return Effect.succeed({ label: query, value: projected })
    }
    return Effect.fail(khalaCodeQaDriverFailure(`RPC QA driver has no state snapshot for query: ${query}`))
  }

  events(): Stream.Stream<KhalaCodeQaAppEvent, KhalaCodeQaDriverFailure> {
    return Stream.empty
  }

  metrics(): Effect.Effect<KhalaCodeQaMetricsSnapshot, KhalaCodeQaDriverFailure> {
    return this.client.request.qaMetrics().pipe(
      Effect.mapError((cause) =>
        khalaCodeQaDriverFailure("RPC QA driver could not read qaMetrics", { cause })
      ),
    )
  }

  shutdown(): Effect.Effect<KhalaCodeQaArtifacts, KhalaCodeQaDriverFailure> {
    return Effect.succeed({
      refs: [],
      summary: {
        mode: this.mode,
        observations: this.observations.length,
      },
    })
  }

  private record(observation: KhalaCodeQaObservation): KhalaCodeQaObservation {
    this.observations.push(observation)
    return observation
  }
}

export const makeKhalaCodeRpcQaDriver = (
  options?: KhalaCodeRpcQaDriverOptions,
): KhalaCodeRpcQaDriver => new KhalaCodeRpcQaDriver(options)

export const makeKhalaCodeDomFixtureQaDriver = (
  options?: Omit<KhalaCodeRpcQaDriverOptions, "mode">,
): KhalaCodeRpcQaDriver => new KhalaCodeRpcQaDriver({ ...options, mode: "dom" })
