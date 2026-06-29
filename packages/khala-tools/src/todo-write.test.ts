import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createTodoWriteTool,
  executeKhalaTool,
  inMemoryKhalaTodoService,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaTodoStatus,
  type KhalaToolServices,
} from "./index.js"

type TodoUi = Readonly<{
  events: ReadonlyArray<Readonly<{ kind: string; payload: unknown }>>
  nonAuthoritative: boolean
  revision: number
  sessionId: string
  todos: ReadonlyArray<Readonly<{
    blockerReason?: string
    content: string
    id: string
    order: number
    status: KhalaTodoStatus
  }>>
}>

function makeServices(): KhalaToolServices {
  return makeKhalaToolServices({ todo: inMemoryKhalaTodoService() })
}

function runTodo(
  args: Readonly<Record<string, unknown>>,
  services: KhalaToolServices = makeServices(),
  sessionId = "s1",
) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createTodoWriteTool()]),
      { arguments: args, id: `call_${sessionId}`, name: "todo_write", sessionId },
      services,
    ),
  )
}

function uiOf(result: Awaited<ReturnType<typeof runTodo>>): TodoUi {
  return result.ui as TodoUi
}

describe("todo_write tool", () => {
  test("creates a session todo list and emits renderer events", async () => {
    const result = await runTodo({
      todos: [
        { content: "Read the package shape", id: "read", status: "completed" },
        { content: "Add todo_write", id: "tool", status: "in_progress" },
      ],
    })
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(result.publicSummary).toContain("Non-authoritative planning state only")
    expect(result.modelOutput.text).toContain("not proof of implementation")
    expect(ui).toMatchObject({
      nonAuthoritative: true,
      revision: 1,
      sessionId: "s1",
    })
    expect(ui.todos.map(todo => todo.id)).toEqual(["read", "tool"])
    expect(ui.events).toEqual([
      expect.objectContaining({ kind: "todo_list_updated" }),
    ])
  })

  test("updates, reorders, blocks, and completes existing todos", async () => {
    const services = makeServices()
    await runTodo({
      todos: [
        { content: "Plan the state service", id: "plan", status: "in_progress" },
        { content: "Write tests", id: "tests", status: "pending" },
      ],
    }, services)

    const result = await runTodo({
      todos: [
        { content: "Write tests", id: "tests", status: "blocked", blocker_reason: "waiting for contract shape" },
        { content: "Plan the state service", id: "plan", status: "completed" },
      ],
    }, services)
    const ui = uiOf(result)

    expect(ui.revision).toBe(2)
    expect(ui.todos.map(todo => `${todo.order}:${todo.id}:${todo.status}`)).toEqual([
      "0:tests:blocked",
      "1:plan:completed",
    ])
    expect(ui.todos[0]?.blockerReason).toBe("waiting for contract shape")
    expect(result.modelOutput.text).toContain("[!] Write tests")
    expect(result.modelOutput.text).toContain("[x] Plan the state service")
  })

  test("rejects malformed todo payloads", async () => {
    expect((await runTodo({})).status).toBe("failed")
    expect((await runTodo({ todos: [{ content: "Missing id", status: "pending" }] })).publicSummary).toContain(
      "id is required",
    )
    expect((await runTodo({
      todos: [
        { content: "First", id: "same", status: "pending" },
        { content: "Second", id: "same", status: "pending" },
      ],
    })).publicSummary).toContain("duplicate todo id")
    expect((await runTodo({ todos: [{ content: "Blocked", id: "blocked", status: "blocked" }] })).publicSummary)
      .toContain("requires blocker_reason")
    expect((await runTodo({
      todos: [
        { content: "One", id: "one", status: "in_progress" },
        { content: "Two", id: "two", status: "in_progress" },
      ],
    })).publicSummary).toContain("at most one in_progress")
  })

  test("isolates todo revisions by Khala session", async () => {
    const services = makeServices()
    const s1 = await runTodo({ todos: [{ content: "Session one", id: "one", status: "pending" }] }, services, "s1")
    const s2 = await runTodo({ todos: [{ content: "Session two", id: "two", status: "pending" }] }, services, "s2")
    const s1Update = await runTodo({
      todos: [{ content: "Session one", id: "one", status: "completed" }],
    }, services, "s1")

    expect(uiOf(s1).revision).toBe(1)
    expect(uiOf(s2).revision).toBe(1)
    expect(uiOf(s2).todos.map(todo => todo.id)).toEqual(["two"])
    expect(uiOf(s1Update).revision).toBe(2)
    expect(uiOf(s1Update).todos.map(todo => `${todo.id}:${todo.status}`)).toEqual(["one:completed"])
  })
})
