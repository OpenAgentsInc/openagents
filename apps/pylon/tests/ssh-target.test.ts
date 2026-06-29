import { describe, expect, test } from "bun:test"

import { classifySshReadiness, normalizeSshTarget } from "../src/ssh-target"

describe("normalizeSshTarget", () => {
  test("defaults omitted SSH target fields", () => {
    expect(normalizeSshTarget({ host: "worker.openagents.test" })).toEqual({
      user: "root",
      host: "worker.openagents.test",
      port: 22,
      fallbackPorts: [],
      knownHostsFile: null,
      proxyCommand: null,
    })
  })

  test("throws on an empty or whitespace host", () => {
    expect(() => normalizeSshTarget({ host: "" })).toThrow()
    expect(() => normalizeSshTarget({ host: "   " })).toThrow()
    expect(() => normalizeSshTarget({ host: "worker openagents.test" })).toThrow()
  })
})

describe("classifySshReadiness", () => {
  test("classifies tcp unreachable probes", () => {
    expect(classifySshReadiness({ tcpOpen: false, authOk: false })).toBe(
      "tcp_unreachable",
    )
  })

  test("classifies failed auth probes", () => {
    expect(classifySshReadiness({ tcpOpen: true, authOk: false })).toBe(
      "auth_failed",
    )
  })

  test("classifies ready probes", () => {
    expect(classifySshReadiness({ tcpOpen: true, authOk: true })).toBe("ready")
  })
})
