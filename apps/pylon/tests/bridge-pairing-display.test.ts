import { describe, expect, test } from "bun:test"
import {
  buildPairingUri,
  parsePairingUri,
  renderPairingText,
} from "../src/node/bridge-pairing-display"

describe("bridge pairing display", () => {
  test("buildPairingUri and parsePairingUri round-trip encoded values", () => {
    const input = {
      baseUrl: "https://openagents.com/pylon bridge?team=a&mode=pair",
      bootstrapId: "bootstrap id/1",
      secret: "secret+value=1&two",
    }

    const uri = buildPairingUri(input)

    expect(uri).toMatch(/^autopilot:\/\/pair\?/)
    expect(parsePairingUri(uri)).toEqual(input)
  })

  test("buildPairingUri carries optional public E2EE relay metadata", () => {
    const input = {
      baseUrl: "https://openagents.com",
      bootstrapId: "bootstrap-1",
      secret: "secret-1",
      relayUrl: "https://openagents.com/pylon/relay",
      serverPublicKey: "server-public-key.fixture",
    }

    const parsed = parsePairingUri(buildPairingUri(input))

    expect(parsed).toEqual({
      ...input,
      protocol: "openagents.companion.e2ee.v1",
    })
  })

  test("invalid baseUrl throws", () => {
    expect(() =>
      buildPairingUri({
        baseUrl: "ftp://openagents.com",
        bootstrapId: "bootstrap-1",
        secret: "secret-1",
      }),
    ).toThrow("baseUrl must be a non-empty http(s) URL")

    expect(() =>
      buildPairingUri({
        baseUrl: "",
        bootstrapId: "bootstrap-1",
        secret: "secret-1",
      }),
    ).toThrow("baseUrl must be a non-empty http(s) URL")
  })

  test("parsePairingUri returns null on garbage", () => {
    expect(parsePairingUri("garbage")).toBeNull()
    expect(parsePairingUri("autopilot://pair?host=not-a-url&bid=bootstrap-1&s=secret-1")).toBeNull()
    expect(parsePairingUri("autopilot://pair?host=https%3A%2F%2Fopenagents.com&bid=bootstrap-1&s=secret-1&relay=ftp%3A%2F%2Fexample.com")).toBeNull()
    expect(parsePairingUri("https://openagents.com/pair?host=https%3A%2F%2Fopenagents.com")).toBeNull()
    expect(parsePairingUri("autopilot://pair?host=https%3A%2F%2Fopenagents.com&bid=bootstrap-1")).toBeNull()
  })

  test("renderPairingText contains the base URL and pairing code", () => {
    const text = renderPairingText({
      baseUrl: "https://openagents.com",
      bootstrapId: "bootstrap-1",
      secret: "secret-1",
    })

    expect(text).toContain("https://openagents.com")
    expect(text).toContain("bootstrap-1:secret-1")
    expect(text).toContain("one-time-use")
  })

  test("renderPairingText shows public E2EE metadata without repeating the full server key", () => {
    const text = renderPairingText({
      baseUrl: "https://openagents.com",
      bootstrapId: "bootstrap-1",
      secret: "secret-1",
      relayUrl: "https://openagents.com/pylon/relay",
      serverPublicKey: "abcdefghijklmnopqrstuvwx",
    })

    expect(text).toContain("openagents.companion.e2ee.v1")
    expect(text).toContain("https://openagents.com/pylon/relay")
    expect(text).toContain("abcdefghijkl...qrstuvwx")
    expect(text).not.toContain("abcdefghijklmnopqrstuvwx")
  })
})
