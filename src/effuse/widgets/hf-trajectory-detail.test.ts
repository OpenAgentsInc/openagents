/**
 * HFTrajectoryDetailWidget Tests
 *
 * Tests the HuggingFace trajectory detail widget with mock trajectory data.
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import {
  HFTrajectoryDetailWidget,
  type HFTrajectoryDetailState,
} from "./hf-trajectory-detail.js"
import { mountWidget } from "../widget/mount.js"
import { makeTestLayer } from "../layers/test.js"
import type { Trajectory, Step } from "../../atif/schema.js"

// ============================================================================
// Mock Data
// ============================================================================

/**
 * Create a mock trajectory for testing
 */
const createMockTrajectory = (sessionId: string): Trajectory => ({
  schema_version: "1.0",
  session_id: sessionId,
  agent: {
    name: "test-agent",
    version: "1.0",
    model_name: "test-model",
  },
  steps: [
    {
      step_id: 1,
      timestamp: "2024-01-01T10:00:00Z",
      source: "user",
      message: "Hello, can you help me?",
    },
    {
      step_id: 2,
      timestamp: "2024-01-01T10:00:01Z",
      source: "agent",
      message: "Of course! How can I assist you?",
      reasoning_content: "The user is greeting me and asking for help.",
      tool_calls: [
        {
          id: "call-1",
          function_name: "searchDocs",
          arguments: { query: "help" },
        },
      ],
    },
    {
      step_id: 3,
      timestamp: "2024-01-01T10:00:02Z",
      source: "system",
      observation: {
        results: [
          {
            id: "obs-1",
            content: "Found documentation on help topics.",
          },
        ],
      },
    },
    {
      step_id: 4,
      timestamp: "2024-01-01T10:00:03Z",
      source: "agent",
      message: "I found some helpful documentation.",
      metrics: {
        prompt_tokens: 150,
        completion_tokens: 50,
        cost_usd: 0.0025,
      },
    },
  ],
  extra: {
    task: "test-task",
    episode: "test-episode-1",
    date: "2024-01-01T10:00:00Z",
  },
})

// ============================================================================
// Tests
// ============================================================================

describe("HFTrajectoryDetailWidget", () => {
  test("renders initial empty state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "hf-detail-test" } as Element

          yield* mountWidget(HFTrajectoryDetailWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toBeDefined()
          expect(html).toContain("HF Trajectory Details")
          expect(html).toContain("No trajectory selected")
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = HFTrajectoryDetailWidget.initialState()

    expect(state.trajectory).toBe(null)
    expect(state.sessionId).toBe(null)
    expect(state.loading).toBe(false)
    expect(state.error).toBe(null)
    expect(state.collapsed).toBe(false)
    expect(state.expandedStepId).toBe(null)
    expect(state.viewMode).toBe("formatted")
  })

  test("renders trajectory metadata with loaded state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "hf-detail-test" } as Element

          const trajectory = createMockTrajectory("session-123")

          // Create widget with pre-loaded state
          const loadedWidget = {
            ...HFTrajectoryDetailWidget,
            initialState: (): HFTrajectoryDetailState => ({
              trajectory,
              sessionId: "session-123",
              loading: false,
              error: null,
              collapsed: false,
              expandedStepId: null,
              viewMode: "formatted",
            }),
          }

          yield* mountWidget(loadedWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(50)

          const html = yield* getRendered(container)
          expect(html).toContain("session-123")
          expect(html).toContain("test-agent")
          expect(html).toContain("test-model")
          expect(html).toContain("test-task")
          expect(html).toContain("test-episode-1")
          expect(html).toContain("4") // Step count
        })
      )
    )
  })

  test("renders step list with source badges", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "hf-detail-test" } as Element

          const trajectory = createMockTrajectory("session-456")

          const loadedWidget = {
            ...HFTrajectoryDetailWidget,
            initialState: (): HFTrajectoryDetailState => ({
              trajectory,
              sessionId: "session-456",
              loading: false,
              error: null,
              collapsed: false,
              expandedStepId: null,
              viewMode: "formatted",
            }),
          }

          yield* mountWidget(loadedWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(50)

          const html = yield* getRendered(container)
          // Check for source badges
          expect(html).toContain("user")
          expect(html).toContain("agent")
          expect(html).toContain("system")
          // Check for step IDs
          expect(html).toContain("#1")
          expect(html).toContain("#2")
          expect(html).toContain("#3")
          expect(html).toContain("#4")
        })
      )
    )
  })

  test("shows tool call indicators", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "hf-detail-test" } as Element

          const trajectory = createMockTrajectory("session-789")

          const loadedWidget = {
            ...HFTrajectoryDetailWidget,
            initialState: (): HFTrajectoryDetailState => ({
              trajectory,
              sessionId: "session-789",
              loading: false,
              error: null,
              collapsed: false,
              expandedStepId: null,
              viewMode: "formatted",
            }),
          }

          yield* mountWidget(loadedWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(50)

          const html = yield* getRendered(container)
          // Step 2 has tool calls
          expect(html).toContain("🔧")
          expect(html).toContain("tool")
        })
      )
    )
  })

  test("shows observation indicators", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "hf-detail-test" } as Element

          const trajectory = createMockTrajectory("session-obs")

          const loadedWidget = {
            ...HFTrajectoryDetailWidget,
            initialState: (): HFTrajectoryDetailState => ({
              trajectory,
              sessionId: "session-obs",
              loading: false,
              error: null,
              collapsed: false,
              expandedStepId: null,
              viewMode: "formatted",
            }),
          }

          yield* mountWidget(loadedWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(50)

          const html = yield* getRendered(container)
          // Step 3 has observations
          expect(html).toContain("✓")
          expect(html).toContain("obs")
        })
      )
    )
  })

  test("renders collapsed state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "hf-detail-test" } as Element

          // Create widget with collapsed initial state
          const collapsedWidget = {
            ...HFTrajectoryDetailWidget,
            initialState: (): HFTrajectoryDetailState => ({
              ...HFTrajectoryDetailWidget.initialState(),
              collapsed: true,
            }),
          }

          yield* mountWidget(collapsedWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(50)

          const html = yield* getRendered(container)
          expect(html).toContain("HF Trajectory Details")
          expect(html).toContain("▼") // Collapsed indicator
        })
      )
    )
  })

  test("renders loading state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "hf-detail-test" } as Element

          const loadingWidget = {
            ...HFTrajectoryDetailWidget,
            initialState: (): HFTrajectoryDetailState => ({
              trajectory: null,
              sessionId: null,
              loading: true,
              error: null,
              collapsed: false,
              expandedStepId: null,
              viewMode: "formatted",
            }),
          }

          yield* mountWidget(loadingWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(50)

          const html = yield* getRendered(container)
          expect(html).toContain("Loading trajectory...")
        })
      )
    )
  })

  test("renders with accordion expanded", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "hf-detail-test" } as Element

          const trajectory = createMockTrajectory("session-expanded")

          // Create widget with step 4 expanded (which has metrics)
          const expandedWidget = {
            ...HFTrajectoryDetailWidget,
            initialState: (): HFTrajectoryDetailState => ({
              trajectory,
              sessionId: "session-expanded",
              loading: false,
              error: null,
              collapsed: false,
              expandedStepId: 4, // Expand step with metrics
              viewMode: "formatted",
            }),
          }

          yield* mountWidget(expandedWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(50)

          const html = yield* getRendered(container)
          // Step 4 should be visible
          expect(html).toContain("#4")
          // Expanded step should show more details
          expect(html).toContain("I found some helpful documentation")
        })
      )
    )
  })

  test("renders error message when set", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "hf-detail-test" } as Element

          // Create widget with error initial state
          const errorWidget = {
            ...HFTrajectoryDetailWidget,
            initialState: (): HFTrajectoryDetailState => ({
              ...HFTrajectoryDetailWidget.initialState(),
              error: "Failed to load trajectory",
            }),
          }

          yield* mountWidget(errorWidget, container).pipe(Effect.provide(layer))

          yield* Effect.sleep(50)

          const html = yield* getRendered(container)
          expect(html).toContain("Error loading trajectory")
          expect(html).toContain("Failed to load trajectory")
        })
      )
    )
  })
})
