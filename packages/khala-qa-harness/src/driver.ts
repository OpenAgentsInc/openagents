import { Context, Effect, Stream } from "effect"

import type {
  KhalaCodeQaAction,
  KhalaCodeQaBackendTier,
  KhalaCodeQaDriverMode,
} from "./scenario.js"
import type {
  KhalaCodeQaMetricsSnapshot,
} from "../../../clients/khala-code-desktop/src/shared/qa-metrics.js"
export type {
  KhalaCodeQaMetricsSnapshot,
} from "../../../clients/khala-code-desktop/src/shared/qa-metrics.js"

export type KhalaCodeQaBootOptions = {
  readonly backend: KhalaCodeQaBackendTier
  readonly headless?: boolean
}

export type KhalaCodeQaAppHandle = {
  readonly backend: KhalaCodeQaBackendTier
  readonly mode: KhalaCodeQaDriverMode
  readonly startedAt: string
}

export type KhalaCodeQaObservation = {
  readonly ok: boolean
  readonly action: KhalaCodeQaAction
  readonly label: string
  readonly data?: unknown
  readonly error?: string
}

export type KhalaCodeQaStateSnapshot = {
  readonly label: string
  readonly value: unknown
}

export type KhalaCodeQaAppEvent = {
  readonly kind: string
  readonly observedAt: string
  readonly payload?: unknown
}

export type KhalaCodeQaArtifacts = {
  readonly refs: ReadonlyArray<string>
  readonly summary?: unknown
}

export type KhalaCodeQaDriverFailure = {
  readonly _tag: "KhalaCodeQaDriverFailure"
  readonly message: string
  readonly action?: KhalaCodeQaAction
  readonly cause?: unknown
}

export type KhalaCodeQaDriver = {
  readonly mode: KhalaCodeQaDriverMode
  readonly boot: (
    opts: KhalaCodeQaBootOptions,
  ) => Effect.Effect<KhalaCodeQaAppHandle, KhalaCodeQaDriverFailure>
  readonly act: (
    action: KhalaCodeQaAction,
  ) => Effect.Effect<KhalaCodeQaObservation, KhalaCodeQaDriverFailure>
  readonly read: (
    query: string,
  ) => Effect.Effect<KhalaCodeQaStateSnapshot, KhalaCodeQaDriverFailure>
  readonly events: () => Stream.Stream<KhalaCodeQaAppEvent, KhalaCodeQaDriverFailure>
  readonly metrics: () => Effect.Effect<KhalaCodeQaMetricsSnapshot, KhalaCodeQaDriverFailure>
  readonly shutdown: () => Effect.Effect<KhalaCodeQaArtifacts, KhalaCodeQaDriverFailure>
}

export class KhalaCodeQaDriverService extends Context.Service<
  KhalaCodeQaDriverService,
  KhalaCodeQaDriver
>()("openagents/KhalaCodeQaDriverService") {}

export const khalaCodeQaDriverFailure = (
  message: string,
  options: {
    readonly action?: KhalaCodeQaAction
    readonly cause?: unknown
  } = {},
): KhalaCodeQaDriverFailure => ({
  _tag: "KhalaCodeQaDriverFailure",
  message,
  ...options,
})

export const unsupportedKhalaCodeQaDriver = (
  mode: Exclude<KhalaCodeQaDriverMode, "rpc">,
): KhalaCodeQaDriver => ({
  mode,
  boot: () =>
    Effect.fail(khalaCodeQaDriverFailure(`Khala Code QA ${mode} driver is not implemented in fixture tier yet`)),
  act: (action) =>
    Effect.fail(khalaCodeQaDriverFailure(`Khala Code QA ${mode} driver cannot act yet`, { action })),
  read: () =>
    Effect.fail(khalaCodeQaDriverFailure(`Khala Code QA ${mode} driver cannot read yet`)),
  events: () =>
    Stream.fail(khalaCodeQaDriverFailure(`Khala Code QA ${mode} driver has no event stream yet`)),
  metrics: () =>
    Effect.fail(khalaCodeQaDriverFailure(`Khala Code QA ${mode} driver has no metrics yet`)),
  shutdown: () => Effect.succeed({ refs: [] }),
})
