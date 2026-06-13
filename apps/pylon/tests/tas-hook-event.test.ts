import { describe, expect, test } from "bun:test"

import {
  dispatchEvent,
  type Hook,
  type HookEvent,
  type HookRegistry,
} from "../src/tas/hook-event"

type TestEvent = HookEvent<string> & {
  readonly subject: string
}

const hook = (
  name: string,
  run: Hook<TestEvent>["run"],
): Hook<TestEvent> => ({ name, run })

describe("tas hook event dispatch core", () => {
  test("matching hooks run in registration order", () => {
    const calls: string[] = []
    const registry: HookRegistry<TestEvent> = {
      "tool.before": [
        hook("first", event => {
          calls.push(`first:${event.subject}`)
          return { decision: "continue" }
        }),
        hook("second", event => {
          calls.push(`second:${event.subject}`)
          return { decision: "continue" }
        }),
      ],
    }

    expect(
      dispatchEvent(registry, {
        name: "tool.before",
        subject: "repo.search",
      }),
    ).toEqual({
      blocked: false,
      ranHooks: ["first", "second"],
      mutation: undefined,
    })
    expect(calls).toEqual(["first:repo.search", "second:repo.search"])
  })

  test("block short-circuits remaining hooks", () => {
    const calls: string[] = []
    const registry: HookRegistry<TestEvent> = {
      "tool.before": [
        hook("allow", () => {
          calls.push("allow")
          return { decision: "continue" }
        }),
        hook("deny", () => {
          calls.push("deny")
          return { decision: "block" }
        }),
        hook("after-deny", () => {
          calls.push("after-deny")
          return { decision: "continue" }
        }),
      ],
    }

    expect(
      dispatchEvent(registry, {
        name: "tool.before",
        subject: "repo.write",
      }),
    ).toEqual({
      blocked: true,
      ranHooks: ["allow", "deny"],
      mutation: undefined,
    })
    expect(calls).toEqual(["allow", "deny"])
  })

  test("non-matching event runs nothing", () => {
    const registry: HookRegistry<TestEvent> = {
      "tool.before": [
        hook("only-before", () => {
          throw new Error("non-matching hook should not run")
        }),
      ],
    }

    expect(
      dispatchEvent(registry, {
        name: "tool.after",
        subject: "repo.search",
      }),
    ).toEqual({
      blocked: false,
      ranHooks: [],
      mutation: undefined,
    })
  })

  test("mutation is threaded through matching hooks", () => {
    const seenMutations: Array<string | undefined> = []
    const registry: HookRegistry<TestEvent> = {
      "tool.before": [
        hook("append-a", event => {
          seenMutations.push(event.mutation)
          return { decision: "continue", mutation: `${event.mutation}:a` }
        }),
        hook("append-b", event => {
          seenMutations.push(event.mutation)
          return { decision: "continue", mutation: `${event.mutation}:b` }
        }),
      ],
    }

    expect(
      dispatchEvent(registry, {
        name: "tool.before",
        subject: "repo.search",
        mutation: "input",
      }),
    ).toEqual({
      blocked: false,
      ranHooks: ["append-a", "append-b"],
      mutation: "input:a:b",
    })
    expect(seenMutations).toEqual(["input", "input:a"])
  })
})
