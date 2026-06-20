import { describe, expect, test } from "bun:test"

import { resolveBaseUrls } from "./address-resolution.js"

describe("pairing address resolution", () => {
  test("orders addresses tailnet first by default", () => {
    expect(resolveBaseUrls({
      loopback: "http://127.0.0.1:8787",
      lan: "http://192.168.1.50:8787",
      tailnet: "https://pylon.tailnet.ts.net",
    })).toEqual([
      "https://pylon.tailnet.ts.net",
      "http://192.168.1.50:8787",
      "http://127.0.0.1:8787",
    ])
  })

  test("falls back to LAN when tailnet is missing", () => {
    expect(resolveBaseUrls({
      loopback: "http://127.0.0.1:8787",
      lan: "http://192.168.1.50:8787",
    })).toEqual([
      "http://192.168.1.50:8787",
      "http://127.0.0.1:8787",
    ])
  })

  test("keeps loopback last in the default order", () => {
    expect(resolveBaseUrls({
      loopback: "http://127.0.0.1:8787",
      tailnet: "https://pylon.tailnet.ts.net",
    })).toEqual([
      "https://pylon.tailnet.ts.net",
      "http://127.0.0.1:8787",
    ])
  })

  test("skips missing entries", () => {
    expect(resolveBaseUrls({
      tailnet: "https://pylon.tailnet.ts.net",
    })).toEqual(["https://pylon.tailnet.ts.net"])
  })

  test("honors custom preferences", () => {
    expect(resolveBaseUrls(
      {
        loopback: "http://127.0.0.1:8787",
        lan: "http://192.168.1.50:8787",
        tailnet: "https://pylon.tailnet.ts.net",
      },
      ["loopback", "lan"],
    )).toEqual([
      "http://127.0.0.1:8787",
      "http://192.168.1.50:8787",
    ])
  })

  test("dedupes repeated base URLs", () => {
    expect(resolveBaseUrls({
      loopback: "http://pylon.local:8787",
      lan: "http://pylon.local:8787",
      tailnet: "https://pylon.tailnet.ts.net",
    })).toEqual([
      "https://pylon.tailnet.ts.net",
      "http://pylon.local:8787",
    ])
  })
})
