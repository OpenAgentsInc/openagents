/**
 * ATIF Details Component Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { ATIFDetailsComponent, type ATIFDetailsState, type ATIFStep } from "./atif-details.js"
import { mountComponent } from "../component/mount.js"
import { makeTestLayer } from "../layers/test.js"
import { StateServiceTag } from "../services/state.js"
import { DomServiceTag } from "../services/dom.js"

describe("ATIFDetailsComponent", () => {
  test("renders empty state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "atif-details-test" } as Element

          yield* mountComponent(ATIFDetailsComponent, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toBeDefined()
          expect(html).toContain("ATIF Trajectory")
          expect(html).toContain("No trajectory selected")
        })
      )
    )
  })

  test("US-7.4 displays step-by-step trajectory details", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "atif-details-test" } as Element

          const mockSteps: ATIFStep[] = [
            {
              stepId: 1,
              timestamp: "2024-12-06T10:00:00Z",
              source: "user",
              message: "List all files",
            },
            {
              stepId: 2,
              timestamp: "2024-12-06T10:00:01Z",
              source: "agent",
              message: "I'll list the files for you",
              toolCalls: [
                {
                  toolCallId: "call_123",
                  functionName: "bash",
                  arguments: { command: "ls -la" },
                },
              ],
              observation: {
                results: [
                  {
                    sourceCallId: "call_123",
                    content: "file1.txt\nfile2.txt",
                  },
                ],
              },
            },
          ]

          const customComponent = {
            ...ATIFDetailsComponent,
            initialState: (): ATIFDetailsState => ({
              sessionId: "session-abc123",
              agentName: "mechacoder",
              steps: mockSteps,
              loading: false,
              error: null,
              collapsed: false,
              expandedStepId: null,
            }),
          }

          yield* mountComponent(customComponent, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)

          // Verify header shows agent name
          expect(html).toContain("mechacoder")
          expect(html).toContain("2 steps")

          // Verify steps are listed
          expect(html).toContain("#1")
          expect(html).toContain("#2")

          // Verify source badges (lowercase due to how data is stored)
          expect(html).toContain("user")
          expect(html).toContain("agent")

          // Verify tool call indicator
          expect(html).toContain("🔧 1 tool")
        })
      )
    )
  })

  test("US-7.4 expands step to show tool calls and observations", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer } = yield* makeTestLayer()

          yield* Effect.provide(
            Effect.gen(function* () {
              const stateService = yield* StateServiceTag
              const dom = yield* DomServiceTag
              const container = { id: "atif-details-test" } as Element

              const mockSteps: ATIFStep[] = [
                {
                  stepId: 1,
                  timestamp: "2024-12-06T10:00:00Z",
                  source: "agent",
                  message: "Running command",
                  toolCalls: [
                    {
                      toolCallId: "call_456",
                      functionName: "read_file",
                      arguments: { path: "/tmp/test.txt" },
                    },
                  ],
                  observation: {
                    results: [
                      {
                        sourceCallId: "call_456",
                        content: "Hello, world!",
                      },
                    ],
                  },
                },
              ]

              const state = yield* stateService.cell<ATIFDetailsState>({
                sessionId: "session-xyz",
                agentName: "minimal",
                steps: mockSteps,
                loading: false,
                error: null,
                collapsed: false,
                expandedStepId: 1, // Step is expanded
              })
              const ctx = { state, emit: () => Effect.void, dom, container }

              const html = (yield* ATIFDetailsComponent.render(ctx)).toString()

              // Verify tool call details shown
              expect(html).toContain("Tool Calls:")
              expect(html).toContain("read_file")
              expect(html).toContain("/tmp/test.txt")

              // Verify observation shown
              expect(html).toContain("Observation:")
              expect(html).toContain("Hello, world!")
            }),
            layer
          )
        })
      )
    )
  })

  test("renders collapsed state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "atif-details-test" } as Element

          const customComponent = {
            ...ATIFDetailsComponent,
            initialState: (): ATIFDetailsState => ({
              sessionId: "session-123",
              agentName: "test-agent",
              steps: [
                {
                  stepId: 1,
                  timestamp: "2024-12-06T10:00:00Z",
                  source: "user",
                  message: "test",
                },
              ],
              loading: false,
              error: null,
              collapsed: true,
              expandedStepId: null,
            }),
          }

          yield* mountComponent(customComponent, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("ATIF Trajectory")
          expect(html).toContain("▼") // Collapsed indicator
          // Should not show steps when collapsed
          expect(html).not.toContain("#1")
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = ATIFDetailsComponent.initialState()

    expect(state.sessionId).toBeNull()
    expect(state.agentName).toBeNull()
    expect(state.steps).toEqual([])
    expect(state.loading).toBe(false)
    expect(state.error).toBeNull()
    expect(state.collapsed).toBe(false)
    expect(state.expandedStepId).toBeNull()
  })
})
