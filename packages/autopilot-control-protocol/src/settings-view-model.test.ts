import { describe, expect, test } from "bun:test"

import { buildSettingsView } from "./settings-view-model.js"

describe("settings view model", () => {
  test("projects configured settings in stable display order", () => {
    expect(buildSettingsView({
      baseUrl: "https://bridge.openagents.com",
      owner: "openagents",
      nodeName: "node-a",
      connected: true,
      version: "1.2.3",
    })).toEqual({
      rows: [
        { label: "Connection", value: "https://bridge.openagents.com" },
        { label: "Owner", value: "openagents" },
        { label: "Node", value: "node-a" },
        { label: "Version", value: "1.2.3" },
        { label: "Status", value: "Connected" },
      ],
    })
  })

  test("projects disconnected status independently of configured connection", () => {
    expect(buildSettingsView({
      baseUrl: "https://bridge.openagents.com",
      owner: "openagents",
      nodeName: "node-a",
      connected: false,
      version: "1.2.3",
    }).rows.at(-1)).toEqual({ label: "Status", value: "Disconnected" })
  })

  test("falls back for null optional settings", () => {
    expect(buildSettingsView({
      baseUrl: null,
      owner: null,
      nodeName: null,
      connected: false,
      version: null,
    })).toEqual({
      rows: [
        { label: "Connection", value: "Not configured" },
        { label: "Owner", value: "Unknown" },
        { label: "Node", value: "Unknown" },
        { label: "Version", value: "Unknown" },
        { label: "Status", value: "Disconnected" },
      ],
    })
  })

  test("trims display values before projecting rows", () => {
    expect(buildSettingsView({
      baseUrl: "  http://127.0.0.1:8787  ",
      owner: "  owner-id  ",
      nodeName: "  local-node  ",
      connected: true,
      version: "  0.0.1-dev  ",
    })).toEqual({
      rows: [
        { label: "Connection", value: "http://127.0.0.1:8787" },
        { label: "Owner", value: "owner-id" },
        { label: "Node", value: "local-node" },
        { label: "Version", value: "0.0.1-dev" },
        { label: "Status", value: "Connected" },
      ],
    })
  })

  test("treats blank optional settings as missing", () => {
    expect(buildSettingsView({
      baseUrl: "",
      owner: "  ",
      nodeName: "\t",
      connected: true,
      version: "\n",
    })).toEqual({
      rows: [
        { label: "Connection", value: "Not configured" },
        { label: "Owner", value: "Unknown" },
        { label: "Node", value: "Unknown" },
        { label: "Version", value: "Unknown" },
        { label: "Status", value: "Connected" },
      ],
    })
  })

  test("does not redact owner or node display values", () => {
    expect(buildSettingsView({
      baseUrl: null,
      owner: "owner:public-id",
      nodeName: "node:exact-name",
      connected: true,
      version: null,
    }).rows).toContainEqual({ label: "Owner", value: "owner:public-id" })
    expect(buildSettingsView({
      baseUrl: null,
      owner: "owner:public-id",
      nodeName: "node:exact-name",
      connected: true,
      version: null,
    }).rows).toContainEqual({ label: "Node", value: "node:exact-name" })
  })
})
