import { Effect, Exit, Scope } from "effect"

import {
  startFleetRunSupervisor,
  type FleetRunSupervisorCapacity,
  type FleetRunSupervisorClock,
  type FleetRunSupervisorHandle,
  type FleetRunSupervisorObservedEvent,
  type FleetRunSupervisorPlanner,
  type FleetRunSupervisorRunner,
  type FleetRunSupervisorTickResult,
} from "./fleet-run-supervisor.js"
import type {
  CreateFleetRunInput,
  FleetRun,
  FleetRunControlVerb,
  FleetRunState,
  PylonOrchestrationStore,
} from "./store.js"

export type PylonFleetRunSnapshot = {
  readonly active: boolean
  readonly lastTick: FleetRunSupervisorTickResult | null
  readonly lifecycle: readonly FleetRunSupervisorObservedEvent[]
  readonly pylonRef: string | null
  readonly run: FleetRun
}

export type PylonFleetRunControlResult = PylonFleetRunSnapshot & {
  readonly verb: FleetRunControlVerb
}

export type StartPylonFleetRunInput = {
  readonly capacity: FleetRunSupervisorCapacity
  readonly clock?: Partial<FleetRunSupervisorClock> | undefined
  readonly onLifecycle?: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
  readonly planner: FleetRunSupervisorPlanner
  readonly pylonRef: string
  readonly run: CreateFleetRunInput
  readonly runner: FleetRunSupervisorRunner
  readonly startImmediately?: boolean | undefined
  readonly tickIntervalMs?: number | undefined
}

export type ResumePylonFleetRunInput = Omit<StartPylonFleetRunInput, "run"> & {
  readonly runRef: string
}

export type PylonFleetRunManagerOptions = {
  readonly now?: (() => Date) | undefined
  readonly store: PylonOrchestrationStore
}

type ActiveFleetRun = {
  readonly handle: FleetRunSupervisorHandle
  lastTick: FleetRunSupervisorTickResult | null
  readonly lifecycle: FleetRunSupervisorObservedEvent[]
  readonly pylonRef: string
  readonly scope: Scope.Scope
}

/**
 * Pylon-owned lifecycle/composition authority for standing FleetRuns.
 *
 * Product hosts supply only adapters (planner, mixed capacity projection, and
 * concrete harness runner). Pylon owns run-record mutation, the one-supervisor
 * guard, scoped loop lifetime, retained lifecycle, control state, and cleanup.
 * Persistence follows the injected store: the standing Pylon composition must
 * pass its Pylon-home `orchestration.sqlite`, while focused unit tests may use
 * memory. `openPylonFleetRunRuntime` owns the durable construction seam; wiring
 * a host to recovery plus durable reactivation goes through
 * `openPylonStandingFleetRunExecutor`.
 */
export class PylonFleetRunManager {
  readonly store: PylonOrchestrationStore
  private readonly active = new Map<string, ActiveFleetRun>()
  private closed = false
  private readonly now: () => Date
  private readonly retainedLifecycle = new Map<string, readonly FleetRunSupervisorObservedEvent[]>()
  private readonly retainedPylonRefs = new Map<string, string>()

  constructor(options: PylonFleetRunManagerOptions) {
    this.store = options.store
    this.now = options.now ?? (() => new Date())
  }

  private assertOpen(): void {
    if (this.closed) throw new Error("fleet run manager is closed")
  }

  async start(input: StartPylonFleetRunInput): Promise<PylonFleetRunSnapshot> {
    this.assertOpen()
    await this.reapTerminalActives()
    if (this.store.getFleetRun(input.run.runRef) !== null) {
      throw new Error(`fleet run already exists: ${input.run.runRef}`)
    }
    const run = this.store.createFleetRun(input.run)
    return await this.activate(run.runRef, input)
  }

  /**
   * Reactivate one durable running FleetRun after a standing Pylon restart.
   * The caller must recover interrupted owner-local work before entering this
   * seam; `openPylonStandingFleetRunExecutor` owns that ordering.
   */
  async resume(input: ResumePylonFleetRunInput): Promise<PylonFleetRunSnapshot> {
    this.assertOpen()
    await this.reapTerminalActives()
    if (this.active.has(input.runRef)) return this.snapshot(input.runRef)
    const run = this.store.getFleetRun(input.runRef)
    if (run === null) throw new Error(`unknown fleet run: ${input.runRef}`)
    if (run.state !== "running") {
      throw new Error(`cannot resume ${run.state} fleet run: ${input.runRef}`)
    }
    return await this.activate(input.runRef, input)
  }

  private async activate(
    runRef: string,
    input: Omit<StartPylonFleetRunInput, "run">,
  ): Promise<PylonFleetRunSnapshot> {
    const scope = Effect.runSync(Scope.make())
    // `startFleetRunSupervisor` may complete a fire-and-forget dispatch before
    // its handle is returned and installed in `this.active`. Buffer those
    // lifecycle events at the composition boundary so the terminal receipt is
    // not lost in that construction race.
    const earlyLifecycle: FleetRunSupervisorObservedEvent[] = []
    let handle: FleetRunSupervisorHandle
    try {
      handle = await Effect.runPromise(Effect.provideService(
        startFleetRunSupervisor({
          store: this.store,
          pylonRef: input.pylonRef,
          runRef,
          planner: input.planner,
          runner: input.runner,
          capacity: input.capacity,
          // The manager releases terminal scopes synchronously and therefore
          // must retain lifecycle projection through dispatch bookkeeping.
          // Direct supervisor callers keep the fire-and-forget default.
          awaitDispatches: true,
          ...(input.clock === undefined ? {} : { clock: input.clock }),
          ...(input.startImmediately === undefined ? {} : { startImmediately: input.startImmediately }),
          ...(input.tickIntervalMs === undefined ? {} : { tickIntervalMs: input.tickIntervalMs }),
          onLifecycle: async event => {
            const existing = this.active.get(runRef)
            if (existing === undefined) earlyLifecycle.push(event)
            else existing.lifecycle.push(event)
            await input.onLifecycle?.(event)
            if (event.kind === "completed" || event.kind === "terminal") {
              void this.releaseActive(runRef)
            }
          },
        }),
        Scope.Scope,
        scope,
      ))
    } catch (error) {
      // A machine/startup failure must not leave a run looking live. Mark it
      // as reconcile-stopped so a later standing process can recover it while
      // preserving operator stop as a distinct authority source.
      this.store.updateFleetRunState(runRef, "stopped", this.now(), "reconcile")
      await Effect.runPromise(Scope.close(scope, Exit.void))
      throw error
    }

    const active: ActiveFleetRun = {
      handle,
      lastTick: null,
      lifecycle: [...earlyLifecycle],
      pylonRef: input.pylonRef,
      scope,
    }
    this.active.set(runRef, active)
    try {
      active.lastTick = await Effect.runPromise(active.handle.tick())
    } catch {
      // The standing loop remains active; status/control retain the durable
      // run record and later ticks may recover from a transient adapter error.
    }
    const reconciled = this.store.reconcileFleetRun(runRef)
    if (reconciled.state === "completed" || reconciled.state === "stopped") {
      await this.reportTerminalThenRelease(runRef, active)
    }
    return this.snapshot(runRef)
  }

  async status(runRef?: string): Promise<PylonFleetRunSnapshot | readonly PylonFleetRunSnapshot[]> {
    this.assertOpen()
    if (runRef !== undefined) {
      const run = this.store.reconcileFleetRun(runRef)
      if (run.state === "completed" || run.state === "stopped") {
        await this.reportTerminalThenRelease(runRef)
      }
      return this.snapshotForRun(runRef)
    }
    await this.reapTerminalActives()
    return this.store.listFleetRuns().map(run => this.snapshotForRun(run.runRef))
  }

  async control(runRef: string, verb: FleetRunControlVerb): Promise<PylonFleetRunControlResult> {
    this.assertOpen()
    const nextState: FleetRunState =
      verb === "pause" ? "paused" :
      verb === "resume" ? "running" :
      verb === "drain" ? "draining" :
      "stopped"
    this.store.updateFleetRunState(runRef, nextState, this.now())
    if (verb === "stop") await this.reportTerminalThenRelease(runRef)
    return { ...this.snapshot(runRef), verb }
  }

  /**
   * Stop owned supervisor loops and release their scopes.
   *
   * This does not claim to drain runner work already launched by a tick: the
   * supervisor dispatch path records that bookkeeping asynchronously. Runtime
   * owners must drain/reconcile in-flight work before closing SQLite; an
   * await-idle shutdown handshake remains an explicit FC-2 wiring residual.
   */
  async close(): Promise<void> {
    if (this.closed) return
    this.closed = true
    for (const [runRef, active] of [...this.active]) {
      await this.releaseActive(runRef, active)
    }
  }

  private async reapTerminalActives(): Promise<void> {
    for (const [runRef, active] of [...this.active]) {
      const run = this.store.reconcileFleetRun(runRef)
      if (run.state === "completed" || run.state === "stopped") {
        await this.reportTerminalThenRelease(runRef, active)
      }
    }
  }

  /**
   * Give the supervisor the only terminal-reporting pass. A stopped run with
   * residual assignments stays active so later ticks can reconcile them; the
   * manager releases the scope only after that pass observes zero residuals.
   */
  private async reportTerminalThenRelease(
    runRef: string,
    knownActive?: ActiveFleetRun,
  ): Promise<boolean> {
    const active = knownActive ?? this.active.get(runRef)
    if (active === undefined || this.active.get(runRef) !== active) return false
    const run = this.store.getFleetRun(runRef)
    if (run === null || (run.state !== "completed" && run.state !== "stopped")) {
      return false
    }
    try {
      active.lastTick = await Effect.runPromise(active.handle.tick())
    } catch {
      // Projection/reporting is retryable. Keeping the scope active lets the
      // standing loop or a later status/reap pass retry exact same bytes.
      return false
    }
    if (!this.active.has(runRef)) return true
    const reconciled = this.store.reconcileFleetRun(runRef)
    if (reconciled.counters.activeAssignments > 0) return false
    await this.releaseActive(runRef, active)
    return true
  }

  private async releaseActive(runRef: string, knownActive?: ActiveFleetRun): Promise<void> {
    const active = knownActive ?? this.active.get(runRef)
    if (active === undefined || this.active.get(runRef) !== active) return
    this.retainedLifecycle.set(runRef, [...active.lifecycle])
    this.retainedPylonRefs.set(runRef, active.pylonRef)
    this.active.delete(runRef)
    await Effect.runPromise(Effect.exit(active.handle.stop()))
    await Effect.runPromise(Scope.close(active.scope, Exit.void))
  }

  private snapshot(runRef: string): PylonFleetRunSnapshot {
    const run = this.store.reconcileFleetRun(runRef)
    return this.snapshotForRun(run.runRef)
  }

  private snapshotForRun(runRef: string): PylonFleetRunSnapshot {
    const run = this.store.getFleetRun(runRef)
    if (run === null) throw new Error(`unknown fleet run: ${runRef}`)
    const active = this.active.get(runRef)
    return {
      active: active !== undefined,
      lastTick: active?.lastTick ?? null,
      lifecycle: [...(active?.lifecycle ?? this.retainedLifecycle.get(runRef) ?? [])],
      pylonRef: active?.pylonRef ?? this.retainedPylonRefs.get(runRef) ?? null,
      run,
    }
  }
}
