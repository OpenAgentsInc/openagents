export type HookDecision = "continue" | "block"

export type HookEvent<TMutation = unknown> = {
  readonly name: string
  readonly mutation?: TMutation
}

export type HookResult<TMutation = unknown> = {
  readonly decision: HookDecision
  readonly mutation?: TMutation
}

export type Hook<TEvent extends HookEvent = HookEvent> = {
  readonly name: string
  readonly run: (event: TEvent) => HookResult<TEvent["mutation"]>
}

export type HookRegistry<TEvent extends HookEvent = HookEvent> = Readonly<
  Record<string, readonly Hook<TEvent>[]>
>

export type HookOutcome<TMutation = unknown> = {
  readonly blocked: boolean
  readonly ranHooks: string[]
  readonly mutation?: TMutation
}

export function dispatchEvent<TEvent extends HookEvent>(
  registry: HookRegistry<TEvent>,
  event: TEvent,
): HookOutcome<TEvent["mutation"]> {
  const hooks = registry[event.name] ?? []
  const ranHooks: string[] = []
  let currentEvent = event

  for (const hook of hooks) {
    ranHooks.push(hook.name)

    const result = hook.run(currentEvent)
    const nextMutation =
      result.mutation === undefined ? currentEvent.mutation : result.mutation

    if (result.decision === "block") {
      return {
        blocked: true,
        ranHooks,
        mutation: nextMutation,
      }
    }

    currentEvent = {
      ...currentEvent,
      mutation: nextMutation,
    }
  }

  return {
    blocked: false,
    ranHooks,
    mutation: currentEvent.mutation,
  }
}
