import { describe, expect, test } from "bun:test"
import type { Html } from "foldkit/html"
import type { AccountSummary } from "../src/accounts"

Object.assign(globalThis, {
  window: {
    requestAnimationFrame: (callback: FrameRequestCallback): number => {
      callback(0)
      return 0
    },
  },
})

const { AccountList, rateLimitCountdownProjection } = await import("../src/accounts")

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === "object" && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(" ")
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [["class", classes] as const]),
  ]

  return pairs
    .filter(([, value]) => value !== false && value !== undefined && value !== null)
    .map(([name, value]) => (value === true ? ` ${name}` : ` ${name}="${String(value)}"`))
    .join("")
}

const renderHtml = (html: Html): string => {
  if (html === null) return ""
  if (!isVNodeLike(html)) return ""
  const tag = html.sel ?? "node"
  const children = (html.children ?? [])
    .map((child) => (typeof child === "string" ? child : renderHtml(child)))
    .join("")
  const text = html.text ?? ""

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

describe("Autopilot account components", () => {
  test("renders account refs, state chips, and quota chips", () => {
    const accounts = [
      {
        accountRefHash: "acct.hash.ready0001",
        provider: "codex",
        state: "ready",
        usage: { used: 4, limit: 10 },
      },
      {
        accountRefHash: "acct.hash.blocked0002",
        provider: "claude",
        state: "quota_blocked",
        usage: { used: 10, limit: 10 },
        rateLimitResetAt: "2026-06-28T16:45:00Z",
      },
    ] satisfies ReadonlyArray<AccountSummary>

    const rendered = renderHtml(
      AccountList({ accounts, now: "2026-06-28T15:00:00Z" }),
    )

    expect(rendered).toContain('data-autopilot-account-list=""')
    expect(rendered).toContain("acct.hash.ready0001")
    expect(rendered).toContain("acct.hash.blocked0002")
    expect(rendered).toContain('data-autopilot-account-state="ready"')
    expect(rendered).toContain('data-autopilot-account-state="quota_blocked"')
    expect(rendered).toContain("quota: 4/10")
    expect(rendered).toContain("quota: 10/10")
    expect(rendered).toContain('datetime="2026-06-28T16:45:00Z"')
    expect(rendered).toContain('data-autopilot-rate-limit-reset-at="2026-06-28T16:45:00Z"')
    expect(rendered).toContain("resets in 1h 45m")
  })

  test("renders an honest unknown countdown for blocked accounts without reset evidence", () => {
    const accounts = [
      {
        accountRefHash: "acct.hash.blocked0003",
        provider: "codex",
        state: "quota_blocked",
      },
    ] satisfies ReadonlyArray<AccountSummary>

    const rendered = renderHtml(AccountList({ accounts, now: "2026-06-28T15:00:00Z" }))

    expect(rendered).toContain('data-autopilot-rate-limit-countdown="reset: unknown"')
    expect(rendered).toContain("reset: unknown")
  })

  test("projects rate-limit countdown labels without wall-clock dependency", () => {
    expect(rateLimitCountdownProjection({
      resetAt: "2026-06-28T17:00:00Z",
      now: "2026-06-28T15:00:00Z",
    })).toMatchObject({
      label: "resets in 2h",
      tone: "warning",
      resetAt: "2026-06-28T17:00:00Z",
      remainingMs: 7_200_000,
    })

    expect(rateLimitCountdownProjection({
      resetAt: "2026-06-28T14:59:00Z",
      now: "2026-06-28T15:00:00Z",
    })).toMatchObject({
      label: "reset due",
      tone: "info",
    })

    expect(rateLimitCountdownProjection({
      resetAt: "not-a-date",
      now: "2026-06-28T15:00:00Z",
    })).toMatchObject({
      label: "reset: invalid",
      tone: "danger",
      remainingMs: null,
    })
  })
})
