// Firm reason: 1 (Effect UI Play + TestHost + TestClock end-to-end UI flow test for docs/effect/half-day-sprint.md)
// @vitest-environment happy-dom
import { Router } from "@effect/ui"
import { HtmlLocator, HtmlPlay, ssr } from "@effect/ui/html/testing"
import { Locator, TestHost } from "@effect/ui/testing"
import { assert, describe, it } from "@effect/vitest"
import { Effect, Layer, PubSub, Stream } from "effect"
import { TestClock } from "effect/testing"
import { App } from "../client/App.ts"
import {
  ApproveCommand,
  CommandAck,
  SessionCommandsNoop,
  StartTurnCommand,
  SteerCommand,
  StopCommand
} from "../client/commands/SessionCommands.ts"
import { SessionStream } from "../client/queries/SessionStream.ts"
import { AppRoute } from "../shared/routes.ts"
import type { SessionSnapshot } from "../shared/wire.ts"

const baseSnapshot = (overrides: Partial<SessionSnapshot> = {}): SessionSnapshot => ({
  sessionId: "demo",
  seq: 1,
  status: "ready",
  activeTurn: 0,
  title: "OpenAgents Spike",
  cwd: "/tmp/demo",
  model: "google/gemini-4-neon",
  messages: [],
  pendingPermit: null,
  todos: [],
  stats: null,
  lastError: null,
  workerPid: 4242,
  ...overrides
})

describe("ChatFlow Play & SSR", () => {
  it.effect("drives full prompt -> stream -> approval -> steer -> reply flow on TestHost", () =>
    Effect.gen(function*() {
      const pubsub = yield* PubSub.unbounded<SessionSnapshot>()
      const sentTasks: Array<{ task: string; model: string; gated: boolean }> = []
      const sentApprovals: Array<{ ticket: number; answer: boolean }> = []
      const sentSteers: Array<{ steer: string; turn: number }> = []
      let stopCount = 0

      const testLayer = Layer.mergeAll(
        Router.memory({ routes: AppRoute, url: "/" }),
        TestHost.layer(),
        SessionStream.toLayer(() =>
          Stream.concat(
            Stream.succeed(baseSnapshot({ seq: 1 })),
            Stream.fromPubSub(pubsub)
          )
        ),
        StartTurnCommand.toLayer(({ task, model, gated }) =>
          Effect.sync(() => {
            sentTasks.push({ task, model, gated })
            return CommandAck.Ok()
          })
        ),
        ApproveCommand.toLayer(({ ticket, answer }) =>
          Effect.sync(() => {
            sentApprovals.push({ ticket, answer })
            return CommandAck.Ok()
          })
        ),
        SteerCommand.toLayer(({ steer, turn }) =>
          Effect.sync(() => {
            sentSteers.push({ steer, turn })
            return CommandAck.Ok()
          })
        ),
        StopCommand.toLayer(() =>
          Effect.sync(() => {
            stopCount += 1
            return CommandAck.Ok()
          })
        )
      )

      yield* Effect.gen(function*() {
        const play = yield* HtmlPlay.make(App, Router.props(AppRoute))
        yield* TestClock.adjust("50 millis")

        // 1. Initial empty state & ready status
        yield* play.expect(Locator.text("ready · pid 4242")).toExist()
        yield* play
          .expect(
            Locator.text("Send a prompt below to run a live turn on the compiled Coax oa-worker.")
          )
          .toExist()

        // 2. Type prompt and submit via isolated Composer
        const input = HtmlLocator.role("textbox", { name: "Message" })
        yield* play.type(input, "Inspect README and run ls")
        yield* play.click(HtmlLocator.role("button", { name: "Send" }))
        yield* TestClock.adjust("50 millis")
        assert.deepStrictEqual(sentTasks, [
          {
            task: "Inspect README and run ls",
            model: "google/gemini-4-neon",
            gated: true
          }
        ])

        // 3. Server streams running turn + thinking + todos + pending approval card
        yield* PubSub.publish(
          pubsub,
          baseSnapshot({
            seq: 2,
            status: "waiting_approval",
            activeTurn: 1,
            todos: [{ id: "t1", content: "Inspect workspace", status: "in_progress" }],
            pendingPermit: {
              ticket: 7,
              kind: "permit",
              question: false,
              title: "Run shell command",
              command: "ls -la",
              options: []
            },
            messages: [
              {
                id: "msg-1",
                role: "user",
                turn: 1,
                thinking: "",
                text: "Inspect README and run ls",
                tools: [],
                ending: null,
                streaming: false
              },
              {
                id: "msg-2",
                role: "assistant",
                turn: 1,
                thinking: "Checking directory contents first",
                text: "",
                tools: [],
                ending: null,
                streaming: true
              }
            ]
          })
        )
        yield* TestClock.adjust("50 millis")

        // Verify ApprovalCard, TodosPanel, and running Composer controls (Steer + Stop)
        yield* play.expect(Locator.text("Run shell command (#7)")).toExist()
        yield* play.expect(Locator.text("[~] Inspect workspace")).toExist()
        yield* play.expect(HtmlLocator.role("button", { name: "Stop" })).toExist()

        // 4. Approve ticket #7
        yield* play.click(HtmlLocator.role("button", { name: "Allow" }))
        yield* TestClock.adjust("50 millis")
        assert.deepStrictEqual(sentApprovals, [{ ticket: 7, answer: true }])

        // 5. Send a mid-turn steer while running
        yield* play.type(input, "Keep it concise")
        yield* play.click(HtmlLocator.role("button", { name: "Steer" }))
        yield* TestClock.adjust("50 millis")
        assert.deepStrictEqual(sentSteers, [{ steer: "Keep it concise", turn: 1 }])

        // 6. Stream completed markdown reply + tool result + stats
        yield* PubSub.publish(
          pubsub,
          baseSnapshot({
            seq: 3,
            status: "ready",
            activeTurn: 1,
            pendingPermit: null,
            todos: [{ id: "t1", content: "Inspect workspace", status: "completed" }],
            stats: { ttftMs: 420, decodeMs: 200, outputTokens: 40 },
            messages: [
              {
                id: "msg-1",
                role: "user",
                turn: 1,
                thinking: "",
                text: "Inspect README and run ls",
                tools: [],
                ending: null,
                streaming: false
              },
              {
                id: "msg-2",
                role: "assistant",
                turn: 1,
                thinking: "Checking directory contents first",
                text: "Found **3 files** in the workspace.",
                tools: [
                  {
                    id: "tool-1",
                    kind: "command",
                    command: "ls -la",
                    path: "",
                    found: true,
                    exit: 0,
                    timedOut: false,
                    output: "README.md\npackage.json\n"
                  }
                ],
                ending: "finished",
                streaming: false
              }
            ]
          })
        )
        yield* TestClock.adjust("50 millis")

        yield* play.expect(Locator.text("3 files")).toExist()
        yield* play.expect(Locator.text("TTFT 420ms · 200 tok/s")).toExist()
        yield* play.expect(HtmlLocator.role("button", { name: "Send" })).toExist()
        assert.strictEqual(stopCount, 0)
      }).pipe(Effect.provide(testLayer))
    }))

  it.effect("renders on server and hydrates cleanly with zero mismatch", () =>
    Effect.gen(function*() {
      const ssrLayer = Layer.mergeAll(
        Router.memory({ routes: AppRoute, url: "/" }),
        TestHost.layer(),
        SessionStream.layerSsr,
        SessionCommandsNoop
      )
      const result = yield* ssr(App, Router.props(AppRoute)).pipe(
        Effect.provide(ssrLayer)
      )
      assert.isTrue(result.html.includes("OpenAgents — Effect UI"))
      assert.include(result.adopted, "/App")
      assert.include(result.adopted, "/App/Composer[]")
    }))
})
