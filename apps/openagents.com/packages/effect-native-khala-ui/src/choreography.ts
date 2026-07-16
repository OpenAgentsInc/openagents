import { Deferred, Duration, Effect, Exit, Fiber, Ref, Scope } from "effect"

export const khalaTransitionStates = ["exited", "entering", "entered", "exiting"] as const
export type KhalaTransitionState = (typeof khalaTransitionStates)[number]
export type KhalaStableState = "exited" | "entered"

export const khalaManagerNames = [
  "parallel",
  "sequence",
  "sequenceReverse",
  "stagger",
  "staggerReverse",
  "switch"
] as const
export type KhalaManagerName = (typeof khalaManagerNames)[number]

export interface KhalaChoreographyChild {
  readonly id: string
  readonly enterMillis: number
  readonly exitMillis: number
}

export interface KhalaChoreographyStep {
  readonly id: string
  readonly target: KhalaStableState
  readonly offsetMillis: number
  readonly durationMillis: number
}

export interface KhalaChoreographyPlanInput {
  readonly manager: KhalaManagerName
  readonly target: KhalaStableState
  readonly children: ReadonlyArray<KhalaChoreographyChild>
  readonly staggerMillis?: number
  readonly activeId?: string
}

export type KhalaPlanComposition = "merge" | "combine"

const boundedMillis = (value: number, maximum = 60_000): number =>
  Math.min(maximum, Math.max(0, Number.isFinite(value) ? Math.round(value) : 0))

const childDuration = (child: KhalaChoreographyChild, target: KhalaStableState): number =>
  boundedMillis(target === "entered" ? child.enterMillis : child.exitMillis)

/** Pure manager planner; renderer and Effect runtime independent. */
export const planKhalaChoreography = (input: KhalaChoreographyPlanInput): ReadonlyArray<KhalaChoreographyStep> => {
  const children = input.children.slice(0, 64)
  const reverse = input.manager === "sequenceReverse" || input.manager === "staggerReverse"
  const ordered = reverse ? [...children].reverse() : children

  if (input.manager === "switch") {
    const activeId = input.activeId ?? children[0]?.id
    return children.map((child) => ({
      id: child.id,
      target: input.target === "entered" && child.id === activeId ? "entered" : "exited",
      offsetMillis: 0,
      durationMillis: childDuration(child, input.target === "entered" && child.id === activeId ? "entered" : "exited")
    }))
  }

  if (input.manager === "parallel") {
    return children.map((child) => ({
      id: child.id,
      target: input.target,
      offsetMillis: 0,
      durationMillis: childDuration(child, input.target)
    }))
  }

  const stagger = boundedMillis(input.staggerMillis ?? 50, 5_000)
  let sequenceOffset = 0
  return ordered.map((child, index) => {
    const durationMillis = childDuration(child, input.target)
    const offsetMillis = input.manager === "sequence" || input.manager === "sequenceReverse" ? sequenceOffset : index * stagger
    sequenceOffset += durationMillis
    return { id: child.id, target: input.target, offsetMillis, durationMillis }
  })
}

/**
 * Compose nested manager plans without creating a second runtime graph.
 * `merge` is last-writer-wins by id; `combine` spans duplicate steps so the
 * outer plan cannot finish before either contributing interval converges.
 */
export const composeKhalaChoreographyPlans = (
  plans: ReadonlyArray<ReadonlyArray<KhalaChoreographyStep>>,
  composition: KhalaPlanComposition
): ReadonlyArray<KhalaChoreographyStep> => {
  const byId = new Map<string, KhalaChoreographyStep>()
  for (const step of plans.flat().slice(0, 256)) {
    const current = byId.get(step.id)
    if (current === undefined || composition === "merge") {
      byId.set(step.id, step)
      continue
    }
    const start = Math.min(current.offsetMillis, step.offsetMillis)
    const end = Math.max(current.offsetMillis + current.durationMillis, step.offsetMillis + step.durationMillis)
    byId.set(step.id, {
      id: step.id,
      target: step.target,
      offsetMillis: start,
      durationMillis: end - start
    })
  }
  return [...byId.values()]
}

export interface KhalaChoreographyOptions {
  readonly reducedMotion?: boolean
}

export interface KhalaChoreography {
  readonly transition: (id: string, target: KhalaStableState, durationMillis: number, delayMillis?: number) => Effect.Effect<void>
  readonly runPlan: (plan: ReadonlyArray<KhalaChoreographyStep>) => Effect.Effect<void>
  readonly state: (id: string) => Effect.Effect<KhalaTransitionState>
  readonly snapshot: Effect.Effect<Readonly<Record<string, KhalaTransitionState>>>
  readonly activeDrivers: Effect.Effect<number>
  readonly scheduledWork: Effect.Effect<number>
  readonly awaitIdle: Effect.Effect<void>
  readonly dispose: Effect.Effect<void>
}

const validId = (id: string): boolean => /^[A-Za-z][A-Za-z0-9_-]{0,63}$/.test(id)

/**
 * Scope-owned choreography runtime. Every delayed transition is a child Fiber
 * of a private Scope. Reversal interrupts and joins the prior Fiber before the
 * replacement is registered, so stale completion cannot overwrite the target.
 */
export const makeKhalaChoreography = (
  options: KhalaChoreographyOptions = {}
): Effect.Effect<KhalaChoreography, never, Scope.Scope> =>
  Effect.gen(function* () {
    const parentScope = yield* Scope.Scope
    const runtimeScope = yield* Scope.fork(parentScope)
    const states = yield* Ref.make(new Map<string, KhalaTransitionState>())
    const fibers = yield* Ref.make(new Map<string, Fiber.Fiber<void, never>>())

    const interrupt = (id: string): Effect.Effect<void> =>
      Effect.gen(function* () {
        const current = yield* Ref.modify(fibers, (value) => {
          const next = new Map(value)
          const fiber = next.get(id)
          next.delete(id)
          return [fiber, next] as const
        })
        if (current !== undefined) yield* Fiber.interrupt(current).pipe(Effect.asVoid)
      })

    const transition = (
      id: string,
      target: KhalaStableState,
      durationMillis: number,
      delayMillis = 0
    ): Effect.Effect<void> =>
      Effect.gen(function* () {
        if (!validId(id)) return
        yield* interrupt(id)
        if (options.reducedMotion === true) {
          yield* Ref.update(states, (value) => new Map(value).set(id, target))
          return
        }

        const duration = boundedMillis(durationMillis)
        const delay = boundedMillis(delayMillis)
        yield* Ref.update(states, (value) => new Map(value).set(id, target === "entered" ? "entering" : "exiting"))

        const start = yield* Deferred.make<void>()
        const program = Deferred.await(start).pipe(
          Effect.andThen(Effect.sleep(Duration.millis(delay + duration))),
          Effect.andThen(Ref.update(states, (value) => new Map(value).set(id, target))),
          Effect.ensuring(
            Ref.update(fibers, (value) => {
              const next = new Map(value)
              next.delete(id)
              return next
            })
          )
        )
        const fiber = yield* Scope.provide(runtimeScope)(Effect.forkScoped(program))
        yield* Ref.update(fibers, (value) => new Map(value).set(id, fiber))
        yield* Deferred.succeed(start, undefined)
      })

    const runPlan = (plan: ReadonlyArray<KhalaChoreographyStep>): Effect.Effect<void> =>
      Effect.forEach(
        plan.slice(0, 64),
        (step) => transition(step.id, step.target, step.durationMillis, step.offsetMillis),
        { concurrency: "unbounded", discard: true }
      )

    const awaitIdle = Effect.flatMap(Ref.get(fibers), (value) =>
      Effect.forEach([...value.values()], Fiber.await, { concurrency: "unbounded", discard: true })
    )

    return {
      transition,
      runPlan,
      state: (id) => Effect.map(Ref.get(states), (value) => value.get(id) ?? "exited"),
      snapshot: Effect.map(Ref.get(states), (value) => Object.fromEntries(value)),
      activeDrivers: Effect.map(Ref.get(fibers), (value) => value.size),
      scheduledWork: Effect.map(Ref.get(fibers), (value) => value.size),
      awaitIdle,
      dispose: Scope.close(runtimeScope, Exit.void)
    }
  })

export interface KhalaModelReceipt {
  readonly statesChecked: number
  readonly managersChecked: number
  readonly switchExclusive: boolean
  readonly offsetsBounded: boolean
  readonly stableTargets: boolean
}

/** Exhaustive bounded checker for 0–3 children across every manager/target. */
export const checkKhalaChoreographyModel = (): KhalaModelReceipt => {
  let statesChecked = 0
  let switchExclusive = true
  let offsetsBounded = true
  let stableTargets = true
  for (const manager of khalaManagerNames) {
    for (const target of ["entered", "exited"] as const) {
      for (let count = 0; count <= 3; count += 1) {
        const children = Array.from({ length: count }, (_, index) => ({
          id: `node${index}`,
          enterMillis: 100 + index * 10,
          exitMillis: 80 + index * 10
        }))
        const plan = planKhalaChoreography({ manager, target, children, activeId: "node1", staggerMillis: 20 })
        statesChecked += 1
        offsetsBounded &&= plan.every((step) => Number.isFinite(step.offsetMillis) && step.offsetMillis >= 0)
        stableTargets &&= plan.every((step) => step.target === "entered" || step.target === "exited")
        if (manager === "switch" && target === "entered") {
          switchExclusive &&= plan.filter((step) => step.target === "entered").length <= 1
        }
      }
    }
  }
  return {
    statesChecked,
    managersChecked: khalaManagerNames.length,
    switchExclusive,
    offsetsBounded,
    stableTargets
  }
}
