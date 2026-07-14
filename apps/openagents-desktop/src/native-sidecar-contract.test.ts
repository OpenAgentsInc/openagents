import { describe, expect, test } from "vite-plus/test"

import {
  DesktopNativeSidecarNodeVersion,
  DesktopNativeSidecarProtocol,
  decodeDesktopNativeSidecarBootstrapReceipt,
  decodeDesktopNativeSidecarBootstrapRequest,
  executeDesktopNativeSidecarBootstrap,
} from "./native-sidecar-contract.ts"

describe("Desktop Native sidecar contract", () => {
  test("executes the production runtime gateway v11 bootstrap on exact Node 24", async () => {
    const receipt = await executeDesktopNativeSidecarBootstrap(
      { protocol: DesktopNativeSidecarProtocol, generation: 7, nonce: "proof.native_7" },
      { nodeVersion: DesktopNativeSidecarNodeVersion, pid: 4242 },
    )

    expect(receipt).toMatchObject({
      protocol: DesktopNativeSidecarProtocol,
      generation: 7,
      nonce: "proof.native_7",
      pid: 4242,
      nodeVersion: DesktopNativeSidecarNodeVersion,
      gatewayProtocolVersion: 11,
      requestId: "native-sidecar.bootstrap",
      response: {
        kind: "query_result",
        requestId: "native-sidecar.bootstrap",
        result: { kind: "runtime.bootstrap", lifecycle: "ready", protocolVersion: 11 },
      },
    })
    expect(decodeDesktopNativeSidecarBootstrapReceipt(receipt)).toEqual(receipt)
  })

  test("rejects malformed generation, nonce, and excess protocol values", () => {
    expect(decodeDesktopNativeSidecarBootstrapRequest({
      protocol: DesktopNativeSidecarProtocol,
      generation: 0,
      nonce: "proof",
    })).toBeNull()
    expect(decodeDesktopNativeSidecarBootstrapRequest({
      protocol: DesktopNativeSidecarProtocol,
      generation: 1,
      nonce: "../shared",
    })).toBeNull()
    expect(decodeDesktopNativeSidecarBootstrapRequest({
      protocol: "openagents.desktop.native-sidecar.v2",
      generation: 1,
      nonce: "proof",
    })).toBeNull()
    expect(decodeDesktopNativeSidecarBootstrapRequest({
      protocol: DesktopNativeSidecarProtocol,
      generation: 1,
      nonce: "proof",
      ambientPath: "/private/repository",
    })).toBeNull()
  })

  test("refuses an ambient or mismatched Node runtime", async () => {
    await expect(executeDesktopNativeSidecarBootstrap(
      { protocol: DesktopNativeSidecarProtocol, generation: 1, nonce: "proof" },
      { nodeVersion: "22.0.0", pid: 4242 },
    )).rejects.toThrow("requires Node 24.13.1")
  })
})
