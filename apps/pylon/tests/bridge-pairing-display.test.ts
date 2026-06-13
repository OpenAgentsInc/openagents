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

    expect(uri).toBe(
      "autopilot://pair?host=https%3A%2F%2Fopenagents.com%2Fpylon%20bridge%3Fteam%3Da%26mode%3Dpair&bid=bootstrap%20id%2F1&s=secret%2Bvalue%3D1%26two",
    )
    expect(parsePairingUri(uri)).toEqual(input)
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
})
