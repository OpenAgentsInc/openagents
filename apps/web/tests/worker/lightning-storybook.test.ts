import { renderToString } from "@openagentsinc/effuse"
import { describe, expect, it } from "vitest"

import { lightningStories } from "../../src/storybook/stories/lightning"

const byId = (id: string) => lightningStories.find((story) => story.id === id)

const renderStory = (id: string): string => {
  const story = byId(id)
  expect(story, `missing story ${id}`).toBeDefined()
  return renderToString(story!.render())
}

describe("apps/web lightning storybook coverage", () => {
  it("registers required EP212 Lightning UI states", () => {
    const ids = new Set(lightningStories.map((story) => story.id))

    expect(ids.has("lightning-l402-payment-card-intent")).toBe(true)
    expect(ids.has("lightning-l402-payment-card-paying")).toBe(true)
    expect(ids.has("lightning-l402-payment-card-paid")).toBe(true)
    expect(ids.has("lightning-l402-payment-card-cached")).toBe(true)
    expect(ids.has("lightning-l402-payment-card-blocked")).toBe(true)
    expect(ids.has("lightning-l402-payment-card-failed")).toBe(true)
    expect(ids.has("lightning-l402-wallet-pane-offline")).toBe(true)
    expect(ids.has("lightning-l402-wallet-pane-mixed")).toBe(true)
    expect(ids.has("lightning-l402-transactions-pane-mixed")).toBe(true)
    expect(ids.has("lightning-l402-payment-detail-pane-paid")).toBe(true)
    expect(ids.has("lightning-chat-l402-approval")).toBe(true)
    expect(ids.has("lightning-chat-l402-paid-and-cached")).toBe(true)
  })

  it("renders wallet pane offline state with explicit status", () => {
    const html = renderStory("lightning-l402-wallet-pane-offline")

    expect(html).toContain("L402 Wallet Summary")
    expect(html).toContain("wallet status")
    expect(html).toContain("offline")
    expect(html).toContain("desktop executor heartbeat stale")
    expect(html).toContain("balance")
  })

  it("renders payment states for intent, paying, paid, cached, blocked, and failed", () => {
    const intent = renderStory("lightning-l402-payment-card-intent")
    expect(intent).toContain("Preparing L402 payment request")
    expect(intent).toContain("data-payment-state=\"payment.intent\"")

    const paying = renderStory("lightning-l402-payment-card-paying")
    expect(paying).toContain("Preparing L402 payment request")
    expect(paying).toContain("status")
    expect(paying).toContain("paying")

    const paid = renderStory("lightning-l402-payment-card-paid")
    expect(paid).toContain("L402 payment sent")
    expect(paid).toContain("proofReference")

    const cached = renderStory("lightning-l402-payment-card-cached")
    expect(cached).toContain("Reused cached L402 credential")
    expect(cached).toContain("data-payment-state=\"payment.cached\"")

    const blocked = renderStory("lightning-l402-payment-card-blocked")
    expect(blocked).toContain("Payment blocked by policy")
    expect(blocked).toContain("amount_over_cap")

    const failed = renderStory("lightning-l402-payment-card-failed")
    expect(failed).toContain("L402 payment failed")
    expect(failed).toContain("request_failed")
  })

  it("renders transactions + payment detail artifacts for demo rehearsal", () => {
    const txHtml = renderStory("lightning-l402-transactions-pane-mixed")
    expect(txHtml).toContain("Recent L402 Attempts")
    expect(txHtml).toContain("proof:")
    expect(txHtml).toContain("sha:")

    const detailHtml = renderStory("lightning-l402-payment-detail-pane-paid")
    expect(detailHtml).toContain("responseSha")
    expect(detailHtml).toContain("response preview")
    expect(detailHtml).toContain("preimage:aa11bb22cc33dd44")
  })

  it("renders chat stories with approval and cached follow-up", () => {
    const approval = renderStory("lightning-chat-l402-approval")
    expect(approval).toContain("Please approve the spend")
    expect(approval).toContain("Approve payment")

    const paidThenCached = renderStory("lightning-chat-l402-paid-and-cached")
    expect(paidThenCached).toContain("Paid once and fetched the premium payload")
    expect(paidThenCached).toContain("Cache hit: reused the credential")
  })
})
