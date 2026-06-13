import { describe, expect, test } from "bun:test"
import { inspect } from "node:util"

import {
  buildBrokerRegistrationBody,
  buildNodeRegistration,
  postNodeRegistration,
  registrationKey,
} from "../src/node/discovery-register"

const input = {
  nodeRef: "node-1",
  binds: [
    { address: "127.0.0.1" },
    { address: "192.168.1.10" },
    { address: "100.64.0.10" },
  ],
  controlToken: "control-token-1",
  updatedAt: "2026-06-13T12:00:00.000Z",
}

describe("tas discovery register", () => {
  test("classifies bind addresses by address shape", () => {
    expect(buildNodeRegistration(input).addresses).toEqual({
      loopback: "127.0.0.1",
      lan: "192.168.1.10",
      tailnet: "100.64.0.10",
    })
  })

  test("registrationKey is deterministic for owner and node refs", () => {
    expect(registrationKey("owner-1", "node-1")).toBe("owner-1:node-1")
    expect(registrationKey("owner-1", "node-1")).toBe(
      registrationKey("owner-1", "node-1"),
    )
    expect(registrationKey("owner-2", "node-1")).not.toBe(
      registrationKey("owner-1", "node-1"),
    )
  })

  test("carries control token without leaking it through inspect helpers", () => {
    const registration = buildNodeRegistration(input)

    expect(registration.controlToken).toBe("control-token-1")
    expect(String(registration)).not.toContain("control-token-1")
    expect(inspect(registration)).not.toContain("control-token-1")
    expect(JSON.stringify(registration)).not.toContain("control-token-1")
  })

  test("broker registration body uses full base URLs and the REAL token", () => {
    const body = buildBrokerRegistrationBody({
      nodeRef: "node-1",
      name: "chris-mac",
      hosts: { loopback: "127.0.0.1", lan: "192.168.1.10", tailnet: "100.64.0.10" },
      port: 4716,
      controlToken: "control-token-1",
      updatedAt: "2026-06-13T12:00:00.000Z",
    })

    expect(body.addresses).toEqual({
      loopback: "http://127.0.0.1:4716",
      lan: "http://192.168.1.10:4716",
      tailnet: "http://100.64.0.10:4716",
    })
    // The wire body MUST carry the real token (it's the phone's credential),
    // unlike NodeRegistration.toJSON() which redacts for logging.
    expect(body.controlToken).toBe("control-token-1")
    expect(JSON.stringify(body)).toContain("control-token-1")
    expect(body.name).toBe("chris-mac")
  })

  test("postNodeRegistration POSTs JSON to /:owner/nodes and reports ok", async () => {
    let captured: { url: string; method?: string; body?: string } | null = null
    const ok = await postNodeRegistration({
      brokerUrl: "https://broker.example/",
      ownerRef: "chris",
      body: buildBrokerRegistrationBody({
        nodeRef: "n1",
        hosts: { tailnet: "100.64.0.10" },
        port: 4716,
        controlToken: "tok",
        updatedAt: "2026-06-13T12:00:00.000Z",
      }),
      fetchImpl: (async (url: string, init?: RequestInit) => {
        captured = { url, method: init?.method, body: init?.body as string }
        return new Response(JSON.stringify({ ok: true }), { status: 200 })
      }) as unknown as typeof fetch,
    })

    expect(ok).toBe(true)
    expect(captured!.url).toBe("https://broker.example/chris/nodes")
    expect(captured!.method).toBe("POST")
    expect(JSON.parse(captured!.body!).controlToken).toBe("tok")
  })

  test("postNodeRegistration returns false on network failure", async () => {
    const ok = await postNodeRegistration({
      brokerUrl: "https://broker.example",
      ownerRef: "chris",
      body: buildBrokerRegistrationBody({
        nodeRef: "n1",
        hosts: { tailnet: "100.64.0.10" },
        port: 4716,
        controlToken: "tok",
        updatedAt: "2026-06-13T12:00:00.000Z",
      }),
      fetchImpl: (async () => {
        throw new Error("network down")
      }) as unknown as typeof fetch,
    })

    expect(ok).toBe(false)
  })
})
