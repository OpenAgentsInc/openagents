import { describe, expect, mock, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

import {
  buildKhalaCrashReport,
  redactCrashDiagnosticText,
  type KhalaCrashReport,
} from "../src/diagnostics/crash-reporting"

mock.module("../src/components/khala-screen", () => ({
  KhalaScreen: ({ children }: { children?: React.ReactNode }) =>
    React.createElement("KhalaScreen", null, children),
}))

mock.module("../src/components/khala-text", () => ({
  KhalaText: ({ children, text }: { children?: React.ReactNode; text?: string }) =>
    React.createElement("KhalaText", null, text ?? children),
}))

mock.module("../src/components/khala-button", () => ({
  KhalaButton: ({ onPress, text }: { onPress?: () => void; text?: string }) =>
    React.createElement("KhalaButton", { accessibilityRole: "button", onPress }, text),
}))

const { KhalaErrorBoundary } = await import("../src/components/khala-error-boundary")

const ThrowingView = () => {
  throw new Error("render failed")
}

describe("Khala mobile crash reporting seam", () => {
  test("redacts and bounds diagnostic text", () => {
    const redacted = redactCrashDiagnosticText(
      "failed at /Users/alice/work/openagents with Bearer oa_agent_real and user@example.com",
    )

    expect(redacted).toBe("failed at [redacted] with [redacted] and [redacted]")
    expect(redacted.length).toBeLessThanOrEqual(240)
  })

  test("builds a public-safe render crash payload", () => {
    const report = buildKhalaCrashReport(
      new TypeError("bad token oa_agent_123 at /Users/alice/private/file.ts"),
      {
        componentStack: "\n    in SecretScreen (/Users/alice/private/file.tsx:10)",
      },
    )

    expect(report).toEqual({
      area: "render",
      componentStackPreview: "in SecretScreen ([redacted])",
      messageSafe: "bad token [redacted] at [redacted]",
      name: "TypeError",
    })
  })

  test("renders the public-safe fallback and sends redacted payload through an injected reporter", async () => {
    const reports: Array<KhalaCrashReport> = []
    let renderer: ReturnType<typeof createTestRenderer> | undefined

    await act(async () => {
      renderer = createTestRenderer(
      <KhalaErrorBoundary crashReporter={report => {
        reports.push(report)
      }}>
          <ThrowingView />
        </KhalaErrorBoundary>,
      )
    })

    const json = renderer?.toJSON()
    const renderedText = JSON.stringify(json)
    expect(renderedText).toContain("Something went wrong in this mobile view.")
    expect(renderedText).toContain("Try again")
    expect(reports).toHaveLength(1)
    expect(reports[0]?.messageSafe).toBe("render failed")
  })
})
