import { describe, expect, test } from "bun:test"

import { negotiateProtocolVersion } from "./expo-protocol-version.js"

describe("Expo protocol version negotiation", () => {
  test("defaults absent headers to protocol version 0 without directives", () => {
    expect(negotiateProtocolVersion(null)).toEqual({
      version: 0,
      supportsDirectives: false,
    })
  })

  test("defaults blank headers to protocol version 0 without directives", () => {
    expect(negotiateProtocolVersion("   ")).toEqual({
      version: 0,
      supportsDirectives: false,
    })
  })

  test("negotiates version 1 with directive support", () => {
    expect(negotiateProtocolVersion("1")).toEqual({
      version: 1,
      supportsDirectives: true,
    })
  })

  test("trims version 1 headers before negotiation", () => {
    expect(negotiateProtocolVersion("  1\t")).toEqual({
      version: 1,
      supportsDirectives: true,
    })
  })

  test("keeps explicit version 0 without directive support", () => {
    expect(negotiateProtocolVersion("0")).toEqual({
      version: 0,
      supportsDirectives: false,
    })
  })

  test("falls back to version 0 for unsupported header values", () => {
    expect(negotiateProtocolVersion("2")).toEqual({
      version: 0,
      supportsDirectives: false,
    })
    expect(negotiateProtocolVersion("1, 0")).toEqual({
      version: 0,
      supportsDirectives: false,
    })
  })
})
