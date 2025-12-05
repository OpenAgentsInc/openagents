/**
 * Container Panes Widget Tests
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { ContainerPanesWidget, type ContainerPanesState, type ContainerPane } from "./container-panes.js"
import { mountWidget } from "../widget/mount.js"
import { makeTestLayer } from "../layers/test.js"

describe("ContainerPanesWidget", () => {
  test("renders empty state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "container-test" } as Element

          yield* mountWidget(ContainerPanesWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toBeDefined()
          expect(html).toContain("No container executions")
          expect(html).toContain("Containers")
        })
      )
    )
  })

  test("renders with running container", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "container-test" } as Element

          const mockPane: ContainerPane = {
            executionId: "exec-12345",
            image: "ubuntu:22.04",
            command: ["bash", "-c", "echo hello"],
            context: "verification",
            sandboxed: true,
            workdir: "/app",
            status: "running",
            outputLines: [
              { text: "hello", stream: "stdout", sequence: 1 },
            ],
            startedAt: "2024-12-05T10:00:00Z",
          }

          const customWidget = {
            ...ContainerPanesWidget,
            initialState: (): ContainerPanesState => ({
              panes: new Map([["exec-12345", mockPane]]),
              maxVisible: 10,
              maxLinesPerPane: 500,
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("ubuntu:22.04")
          expect(html).toContain("sandbox")
          expect(html).toContain("hello")
          expect(html).toContain("▶") // Running icon
        })
      )
    )
  })

  test("renders completed container with exit code", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "container-test" } as Element

          const mockPane: ContainerPane = {
            executionId: "exec-99999",
            image: "alpine:latest",
            command: ["ls", "-la"],
            context: "init",
            sandboxed: false,
            workdir: "/",
            status: "completed",
            exitCode: 0,
            durationMs: 1234,
            outputLines: [
              { text: "total 0", stream: "stdout", sequence: 1 },
              { text: "drwxr-xr-x 1 root root 0 Jan 1 00:00 .", stream: "stdout", sequence: 2 },
            ],
            startedAt: "2024-12-05T09:00:00Z",
          }

          const customWidget = {
            ...ContainerPanesWidget,
            initialState: (): ContainerPanesState => ({
              panes: new Map([["exec-99999", mockPane]]),
              maxVisible: 10,
              maxLinesPerPane: 500,
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("alpine:latest")
          expect(html).toContain("host") // Not sandboxed
          expect(html).toContain("1.2s") // Duration
          expect(html).toContain("✓") // Success icon
        })
      )
    )
  })

  test("renders failed container with stderr", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "container-test" } as Element

          const mockPane: ContainerPane = {
            executionId: "exec-fail",
            image: "node:18",
            command: ["npm", "test"],
            context: "verification",
            sandboxed: true,
            workdir: "/app",
            status: "completed",
            exitCode: 1,
            durationMs: 5000,
            outputLines: [
              { text: "Error: Test failed", stream: "stderr", sequence: 1 },
            ],
            startedAt: "2024-12-05T08:00:00Z",
          }

          const customWidget = {
            ...ContainerPanesWidget,
            initialState: (): ContainerPanesState => ({
              panes: new Map([["exec-fail", mockPane]]),
              maxVisible: 10,
              maxLinesPerPane: 500,
              collapsed: false,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          expect(html).toContain("node:18")
          expect(html).toContain("✗") // Failed icon
          expect(html).toContain("Error: Test failed")
        })
      )
    )
  })

  test("renders collapsed state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, getRendered } = yield* makeTestLayer()
          const container = { id: "container-test" } as Element

          const mockPane: ContainerPane = {
            executionId: "exec-hidden",
            image: "python:3.11",
            command: ["python", "script.py"],
            context: "subagent",
            sandboxed: true,
            workdir: "/code",
            status: "running",
            outputLines: [],
            startedAt: "2024-12-05T10:00:00Z",
          }

          const customWidget = {
            ...ContainerPanesWidget,
            initialState: (): ContainerPanesState => ({
              panes: new Map([["exec-hidden", mockPane]]),
              maxVisible: 10,
              maxLinesPerPane: 500,
              collapsed: true,
            }),
          }

          yield* mountWidget(customWidget, container).pipe(Effect.provide(layer))

          const html = yield* getRendered(container)
          // When collapsed, should show header but not container details
          expect(html).toContain("+ Containers") // Collapsed indicator
          expect(html).not.toContain("python:3.11") // Content hidden
        })
      )
    )
  })

  test("initialState returns correct defaults", () => {
    const state = ContainerPanesWidget.initialState()

    expect(state.panes.size).toBe(0)
    expect(state.maxVisible).toBe(10)
    expect(state.maxLinesPerPane).toBe(500)
    expect(state.collapsed).toBe(false)
  })
})
