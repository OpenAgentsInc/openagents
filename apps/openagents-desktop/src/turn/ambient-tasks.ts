import { Effect, Fiber, Layer } from "effect"

import {
  AmbientInference,
  AmbientResourceGate,
  AmbientTaskRunner,
  AmbientTaskRunnerLayer,
  type AmbientInferenceInput,
  type AmbientInferenceShape,
  type AmbientResourceGateShape,
  type AmbientResourceSnapshot,
  type AmbientTaskOutcome,
  type AmbientTaskRunInput,
  type AppleFmCompletionTurn,
} from "@openagentsinc/apple-fm-runtime"

import type { AppleFmHost } from "../apple-fm-host.ts"
import type { AppleFmTurnResult } from "../apple-fm-contract.ts"

/**
 * AFS-07 Desktop ambient-task wiring.
 *
 * This thin Desktop adapter composes the neutral `@openagentsinc/apple-fm-runtime`
 * ambient-task runner over the main-owned Apple FM helper. Readiness comes from
 * `host.status()` with NO renderer input; each bounded advisory completion comes
 * from `host.runTurn`, which enforces the 4,000-char read-only prompt contract.
 * The renderer never supplies facts, readiness, or authority.
 *
 * An ambient task is advisory only: it produces a bounded draft, summary, or
 * explanation over DETERMINISTIC host facts and adds NO action authority. It is
 * cancellable and non-blocking, so it never blocks startup, composer send,
 * apply, run, debug, commit, push, or release. Unsupported, slow, cancelled, and
 * resource-limited tasks degrade to a typed non-completion with no failure
 * surface. Local Apple FM inference produces NO provider token row; usage stays
 * estimated or unknown.
 */

/** Map the Desktop IPC turn result into the neutral completion-turn shape. */
const turnResultToCompletion = (result: AppleFmTurnResult): AppleFmCompletionTurn => {
  if (result.outcome !== "completed" || result.text === null) {
    return {
      outcome: "failed",
      usageTruth: result.usageTruth,
      ...(result.failureClass !== null ? { failureClass: result.failureClass } : {}),
    }
  }
  return {
    outcome: "completed",
    text: result.text,
    usageTruth: result.usageTruth,
    ...(result.promptTokens !== null ? { promptTokens: result.promptTokens } : {}),
    ...(result.completionTokens !== null ? { completionTokens: result.completionTokens } : {}),
    ...(result.totalTokens !== null ? { totalTokens: result.totalTokens } : {}),
  }
}

/**
 * Build the ambient inference over the main-owned Apple FM host. The host is
 * resolved lazily because Electron main constructs the supervisor after the
 * composition is installed. It never fails: a missing or refusing host maps to a
 * failed completion turn, which the runner degrades without a failure surface.
 */
export const makeDesktopAmbientInference = (getHost: () => AppleFmHost | null): AmbientInferenceShape => ({
  complete: (input: AmbientInferenceInput) =>
    Effect.promise(async () => {
      const host = getHost()
      if (host === null) {
        return { outcome: "failed", usageTruth: "unknown", failureClass: "not_ready" } satisfies AppleFmCompletionTurn
      }
      return turnResultToCompletion(await host.runTurn(input.prompt))
    }),
})

/** A bounded device resource reading the host supplies (defaults to nominal). */
export interface DesktopThermalProbe {
  readonly thermalState: AmbientResourceSnapshot["thermalState"]
  readonly underMemoryPressure: boolean
}

const defaultThermalProbe: DesktopThermalProbe = { thermalState: "nominal", underMemoryPressure: false }

/**
 * Build the ambient resource gate. Readiness is derived from the main-owned host
 * status; thermal and memory pressure come from an injected probe. The renderer
 * never supplies these facts.
 */
export const makeDesktopAmbientResourceGate = (
  getHost: () => AppleFmHost | null,
  getThermal: () => DesktopThermalProbe = () => defaultThermalProbe,
): AmbientResourceGateShape => ({
  snapshot: Effect.sync(() => {
    const host = getHost()
    const thermal = getThermal()
    return {
      appleFmReady: host !== null && host.status().ready,
      thermalState: thermal.thermalState,
      underMemoryPressure: thermal.underMemoryPressure,
    }
  }),
})

/** The Desktop ambient-task runner layer over the main-owned Apple FM host. */
export const desktopAmbientTaskRunnerLayer = (
  getHost: () => AppleFmHost | null,
  getThermal?: () => DesktopThermalProbe,
): Layer.Layer<AmbientTaskRunner> =>
  AmbientTaskRunnerLayer.pipe(
    Layer.provide(Layer.succeed(AmbientInference, AmbientInference.of(makeDesktopAmbientInference(getHost)))),
    Layer.provide(
      Layer.succeed(AmbientResourceGate, AmbientResourceGate.of(makeDesktopAmbientResourceGate(getHost, getThermal))),
    ),
  )

/**
 * Run one ambient task to its typed outcome over the Desktop host. Never
 * rejects: every gate, timeout, resource, and inference problem resolves to a
 * `Degraded` or `Refused` outcome. An imperative main-process caller invokes
 * this WITHOUT awaiting it, so the ambient task never blocks the host command.
 */
export const runDesktopAmbientTask = <I, O>(
  input: AmbientTaskRunInput<I, O>,
  getHost: () => AppleFmHost | null,
  getThermal?: () => DesktopThermalProbe,
): Promise<AmbientTaskOutcome<O>> =>
  Effect.runPromise(
    AmbientTaskRunner.pipe(
      Effect.flatMap((runner) => runner.run(input)),
      Effect.provide(desktopAmbientTaskRunnerLayer(getHost, getThermal)),
    ),
  )

/** A cancellable, non-blocking imperative dispatch of one ambient task. */
export interface DesktopAmbientDispatch<O> {
  /** Resolves to the terminal outcome, including `Cancelled` on interruption. */
  readonly outcome: Promise<AmbientTaskOutcome<O>>
  /** Interrupts the task; its outcome resolves to `Cancelled`. */
  readonly cancel: () => void
}

/**
 * Fork one ambient task on the default runtime and return a handle immediately.
 * The main-process caller is never blocked: it can start, run, commit, or ship
 * without awaiting the advisory task, and it can cancel it at any time.
 */
export const forkDesktopAmbientTask = <I, O>(
  input: AmbientTaskRunInput<I, O>,
  getHost: () => AppleFmHost | null,
  getThermal?: () => DesktopThermalProbe,
): DesktopAmbientDispatch<O> => {
  const fiber = Effect.runFork(
    AmbientTaskRunner.pipe(
      Effect.flatMap((runner) => runner.run(input)),
      Effect.provide(desktopAmbientTaskRunnerLayer(getHost, getThermal)),
    ),
  )
  return {
    outcome: Effect.runPromise(
      Fiber.await(fiber).pipe(
        Effect.map((exit): AmbientTaskOutcome<O> =>
          exit._tag === "Success" ? exit.value : { _tag: "Cancelled", kind: input.signature.kind },
        ),
      ),
    ),
    cancel: () => {
      Effect.runFork(Fiber.interrupt(fiber))
    },
  }
}
