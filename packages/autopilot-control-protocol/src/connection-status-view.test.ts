import { describe, expect, test } from "bun:test"

import { connectionStatusView } from "./connection-status-view.js"

describe("connection status view", () => {
  test("projects discovering without a known target", () => {
    expect(connectionStatusView({
      status: "discovering",
      nodeName: null,
      baseUrl: null,
    })).toEqual({
      label: "Discovering connection",
      tone: "pending",
      detail: "Looking for an Autopilot node",
    })
  })

  test("projects connecting to a node name", () => {
    expect(connectionStatusView({
      status: "connecting",
      nodeName: "Local node",
      baseUrl: "https://ignored.example.com",
    })).toEqual({
      label: "Connecting",
      tone: "pending",
      detail: "Opening Local node",
    })
  })

  test("projects connected to a base url", () => {
    expect(connectionStatusView({
      status: "connected",
      nodeName: null,
      baseUrl: "https://node.example.com",
    })).toEqual({
      label: "Connected",
      tone: "ok",
      detail: "Connected to https://node.example.com",
    })
  })

  test("projects error tone and detail", () => {
    expect(connectionStatusView({
      status: "error",
      nodeName: "Bridge node",
      baseUrl: null,
    })).toEqual({
      label: "Connection error",
      tone: "error",
      detail: "Could not reach Bridge node",
    })
  })

  test("projects offline as warning", () => {
    expect(connectionStatusView({
      status: "offline",
      nodeName: null,
      baseUrl: "http://localhost:8787",
    })).toEqual({
      label: "Offline",
      tone: "warn",
      detail: "http://localhost:8787 unavailable",
    })
  })

  test("strips token-bearing url parts from details", () => {
    const view = connectionStatusView({
      status: "connected",
      nodeName: null,
      baseUrl: "https://user:secret@example.com/bridge?token=abc123#access_token=def456",
    })

    expect(view).toEqual({
      label: "Connected",
      tone: "ok",
      detail: "Connected to https://example.com/bridge",
    })
    expect(JSON.stringify(view)).not.toContain("secret")
    expect(JSON.stringify(view)).not.toContain("token")
    expect(JSON.stringify(view)).not.toContain("abc123")
    expect(JSON.stringify(view)).not.toContain("def456")
  })

  test("trims blank node names and falls back to sanitized base url", () => {
    expect(connectionStatusView({
      status: "connecting",
      nodeName: "  ",
      baseUrl: "https://node.example.com/control?token=hidden",
    })).toEqual({
      label: "Connecting",
      tone: "pending",
      detail: "Opening https://node.example.com/control",
    })
  })
})
