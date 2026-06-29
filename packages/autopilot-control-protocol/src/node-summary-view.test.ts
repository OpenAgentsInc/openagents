import { describe, expect, test } from "bun:test"

import { nodeSummary } from "./node-summary-view.js"

describe("node summary view", () => {
  test("summarizes a named node with running sessions accounts and balance", () => {
    expect(nodeSummary({
      nodeName: "garage-mac",
      sessions: [
        { state: "running" },
        { state: "completed" },
        { state: "running" },
      ],
      accountsReady: 2,
      accountsTotal: 3,
      balanceSats: 1_250,
    })).toEqual({
      title: "garage-mac",
      lines: [
        "sessions: 2/3 running",
        "accounts: 2/3 ready",
        "balance: 1250 sats",
      ],
    })
  })

  test("uses a stable fallback title for unnamed nodes", () => {
    expect(nodeSummary({
      nodeName: null,
      sessions: [],
      accountsReady: 0,
      accountsTotal: 0,
      balanceSats: null,
    })).toEqual({
      title: "Autopilot node",
      lines: [
        "sessions: 0/0 running",
        "accounts: 0/0 ready",
        "balance: unknown",
      ],
    })
  })

  test("normalizes node whitespace and running state casing", () => {
    expect(nodeSummary({
      nodeName: "  living-room\nnode  ",
      sessions: [
        { state: " RUNNING " },
        { state: "Running" },
        { state: "queued" },
      ],
      accountsReady: 1,
      accountsTotal: 2,
      balanceSats: 42,
    })).toEqual({
      title: "living-room node",
      lines: [
        "sessions: 2/3 running",
        "accounts: 1/2 ready",
        "balance: 42 sats",
      ],
    })
  })

  test("falls back for blank node names", () => {
    expect(nodeSummary({
      nodeName: " \t\n ",
      sessions: [{ state: "queued" }],
      accountsReady: 0,
      accountsTotal: 1,
      balanceSats: 0,
    })).toEqual({
      title: "Autopilot node",
      lines: [
        "sessions: 0/1 running",
        "accounts: 0/1 ready",
        "balance: 0 sats",
      ],
    })
  })

  test("clamps invalid numeric counters for stable display", () => {
    expect(nodeSummary({
      nodeName: "capacity-node",
      sessions: [{ state: "running" }],
      accountsReady: 5.8,
      accountsTotal: 2.2,
      balanceSats: -100,
    })).toEqual({
      title: "capacity-node",
      lines: [
        "sessions: 1/1 running",
        "accounts: 2/2 ready",
        "balance: 0 sats",
      ],
    })
  })

  test("treats non-finite values as zero", () => {
    expect(nodeSummary({
      nodeName: "numeric-node",
      sessions: [],
      accountsReady: Number.POSITIVE_INFINITY,
      accountsTotal: Number.NaN,
      balanceSats: Number.NEGATIVE_INFINITY,
    })).toEqual({
      title: "numeric-node",
      lines: [
        "sessions: 0/0 running",
        "accounts: 0/0 ready",
        "balance: 0 sats",
      ],
    })
  })
})
