import { describe, expect, test } from "bun:test"

import { accountRowsViewModel } from "./accounts-view-model"
import {
  nodeStatusRowsViewModel,
  providerHealthRowsViewModel,
} from "./node-status-view-model"

describe("mobile parity status view models", () => {
  test("maps account state and usage tones with autopilot-ui semantics", () => {
    const rows = accountRowsViewModel([
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
      },
    ])

    expect(rows).toEqual([
      {
        accountRefHash: "acct.hash.ready0001",
        provider: "codex",
        label: "acct.hash.ready0001",
        statusLabel: "ready",
        tone: "success",
        usageText: "quota: 4/10",
        usageTone: "info",
      },
      {
        accountRefHash: "acct.hash.blocked0002",
        provider: "claude",
        label: "acct.hash.blocked0002",
        statusLabel: "quota_blocked",
        tone: "warning",
        usageText: "quota: 10/10",
        usageTone: "danger",
      },
    ])
  })

  test("maps node and provider online status with autopilot-ui semantics", () => {
    expect(
      nodeStatusRowsViewModel([
        {
          nodeRef: "node.fixture.online",
          online: true,
          lastHeartbeatAt: "2026-06-13T12:00:00.000Z",
        },
        {
          nodeRef: "node.fixture.offline",
          online: false,
        },
      ]),
    ).toEqual([
      {
        nodeRef: "node.fixture.online",
        label: "node.fixture.online",
        statusLabel: "online",
        tone: "success",
        lastHeartbeatAt: "2026-06-13T12:00:00.000Z",
      },
      {
        nodeRef: "node.fixture.offline",
        label: "node.fixture.offline",
        statusLabel: "offline",
        tone: "danger",
        lastHeartbeatAt: "none",
      },
    ])

    expect(
      providerHealthRowsViewModel([
        { provider: "codex", online: true },
        { provider: "claude", online: false },
      ]),
    ).toEqual([
      {
        provider: "codex",
        label: "codex",
        statusLabel: "online",
        tone: "success",
      },
      {
        provider: "claude",
        label: "claude",
        statusLabel: "offline",
        tone: "danger",
      },
    ])
  })
})
