import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vite-plus/test"

import { ContextMeter } from "./context-meter.tsx"

const NOW = Date.parse("2026-07-16T00:00:00Z")

describe("ContextMeter", () => {
  it("renders an honest NO DATA state when neither usage nor rate limits are known", () => {
    const html = renderToStaticMarkup(<ContextMeter itemKey="k1" />)
    expect(html).toContain("NO DATA")
    expect(html).toContain('data-timeline-key="k1"')
    expect(html).not.toContain("TOKENS")
  })

  it("renders exact totals with no ceiling when contextWindowTokens is unknown (mid fill level)", () => {
    const html = renderToStaticMarkup(<ContextMeter usage={{
      inputTokens: 8_000,
      cachedInputTokens: 2_000,
      outputTokens: 400,
      reasoningTokens: 447,
      totalTokens: 8_847,
    }} />)
    expect(html).toContain("8,847")
    expect(html).toContain("TOKENS")
    // No ceiling was provided, so no "/ N" denominator may appear.
    expect(html).not.toContain("/ ")
    expect(html).toContain("8,000")
    expect(html).toContain("2,000")
    expect(html).toContain("400")
    expect(html).toContain("447")
  })

  it("renders a used/ceiling fill and flags near-limit at >=85% of a known context window", () => {
    const html = renderToStaticMarkup(<ContextMeter usage={{
      totalTokens: 190_000,
      inputTokens: 180_000,
      outputTokens: 10_000,
      contextWindowTokens: 200_000,
    }} />)
    expect(html).toContain("190,000 / 200,000")
    expect(html).toContain('data-near-limit="true"')
  })

  it("never fabricates a fake zero for an absent field — renders — instead", () => {
    const html = renderToStaticMarkup(<ContextMeter usage={{ totalTokens: 500 }} />)
    expect(html).toContain("500")
    expect(html).toContain("—") // em dash placeholders for the unknown breakdown fields
    expect(html).not.toMatch(/INPUT<\/small>0/)
  })

  it("renders a rate-limit window with an honest reset countdown from resetsAt", () => {
    const html = renderToStaticMarkup(<ContextMeter now={NOW} rateLimits={[{
      label: "primary",
      usedPercent: 42,
      resetsAt: NOW / 1000 + 3 * 60 * 60,
    }]} />)
    expect(html).toContain("PRIMARY")
    expect(html).toContain("42% USED")
    expect(html).toContain("RESETS IN 3H")
  })

  it("normalizes a weekly countdown sampled just shy of seven days to 7D", () => {
    const html = renderToStaticMarkup(<ContextMeter now={NOW} rateLimits={[{
      label: "weekly",
      usedPercent: 12,
      resetsAt: NOW / 1000 + 7 * 24 * 60 * 60 - 30,
    }]} />)
    expect(html).toContain("RESETS IN 7D")
    expect(html).not.toContain("168.0H")
  })

  it("flags a fully rate-limited window", () => {
    const html = renderToStaticMarkup(<ContextMeter rateLimits={[{ label: "secondary", usedPercent: 100 }]} />)
    expect(html).toContain('data-rate-limited="true"')
    expect(html).toContain("100% USED")
  })

  it("renders both usage and rate limits together when both are known", () => {
    const html = renderToStaticMarkup(<ContextMeter
      rateLimits={[{ label: "primary", usedPercent: 10 }, { label: "secondary", usedPercent: 5 }]}
      usage={{ totalTokens: 1_200 }}
    />)
    expect(html).toContain("1,200")
    expect(html).toContain("PRIMARY")
    expect(html).toContain("SECONDARY")
  })

  it("marks a historical (dispatch/inspector) snapshot distinctly from the live mount", () => {
    const html = renderToStaticMarkup(<ContextMeter historical itemKey="hist-1" usage={{ totalTokens: 42 }} />)
    expect(html).toContain('data-historical="true"')
  })

  it("defaults to a non-historical (live) rendering", () => {
    const html = renderToStaticMarkup(<ContextMeter usage={{ totalTokens: 42 }} />)
    expect(html).toContain('data-historical="false"')
  })
})
