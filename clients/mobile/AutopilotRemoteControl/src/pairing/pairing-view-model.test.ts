import { describe, expect, test } from "bun:test"

import {
  buildPairingExchangeRequest,
  pairingStatusView,
  parseBootstrapInput,
} from "./pairing-view-model"

describe("pairing view model", () => {
  test("parses valid bootstrap input from code, URI, and rendered text", () => {
    expect(parseBootstrapInput("bootstrap-1:secret-1")).toEqual({
      ok: true,
      value: {
        bootstrapId: "bootstrap-1",
        secret: "secret-1",
      },
    })

    expect(
      parseBootstrapInput(
        "autopilot://pair?host=https%3A%2F%2Fopenagents.com&bid=bootstrap-1&s=secret-1",
      ),
    ).toEqual({
      ok: true,
      value: {
        baseUrl: "https://openagents.com",
        bootstrapId: "bootstrap-1",
        secret: "secret-1",
      },
    })

    expect(
      parseBootstrapInput(
        [
          "Pylon bridge pairing",
          "Base URL: http://127.0.0.1:8787/",
          "One-time pairing code: bootstrap_2:secret.value",
        ].join("\n"),
      ),
    ).toEqual({
      ok: true,
      value: {
        baseUrl: "http://127.0.0.1:8787",
        bootstrapId: "bootstrap_2",
        secret: "secret.value",
      },
    })
  })

  test("rejects invalid bootstrap input", () => {
    expect(parseBootstrapInput("")).toEqual({ ok: false, reason: "empty" })
    expect(parseBootstrapInput("bootstrap only")).toEqual({ ok: false, reason: "invalid_format" })
    expect(parseBootstrapInput("bootstrap 1:secret-1")).toEqual({
      ok: false,
      reason: "invalid_format",
    })
    expect(parseBootstrapInput("a".repeat(513))).toEqual({ ok: false, reason: "too_long" })
    expect(
      parseBootstrapInput("autopilot://pair?host=ftp%3A%2F%2Fopenagents.com&bid=bootstrap-1&s=secret-1"),
    ).toEqual({ ok: false, reason: "invalid_format" })
  })

  test("builds a plain pairing exchange request descriptor", () => {
    expect(
      buildPairingExchangeRequest({
        baseUrl: "https://openagents.com/",
        bootstrapId: "bootstrap-1",
        secret: "secret-1",
        clientId: "mobile-client-1",
      }),
    ).toEqual({
      url: "https://openagents.com/bridge/pair/exchange",
      method: "POST",
      headers: {
        Authorization: "Bearer secret-1",
        "content-type": "application/json",
      },
      body: {
        verb: "bridge.pair.exchange",
        bootstrapId: "bootstrap-1",
        clientId: "mobile-client-1",
      },
    })
  })

  test("maps pairing status state to display view-models", () => {
    expect(pairingStatusView({ phase: "unpaired" })).toEqual({
      label: "Not paired",
      tone: "neutral",
    })
    expect(pairingStatusView({ phase: "pairing" })).toEqual({
      label: "Pairing",
      tone: "info",
    })
    expect(pairingStatusView({ phase: "paired", pairingRef: "pairing.fixture.1" })).toEqual({
      label: "Paired: pairing.fixture.1",
      tone: "success",
    })
    expect(pairingStatusView({ phase: "error", error: "expired bootstrap" })).toEqual({
      label: "Pairing failed: expired bootstrap",
      tone: "danger",
    })
  })
})
