import { describe, expect, test } from "bun:test"

import {
  parseNodesResponse,
  pickConnect,
  type NodeRegistration,
} from "./discovery-client"

describe("discovery client", () => {
  test("picks tailnet address first", () => {
    const registration: NodeRegistration = {
      addresses: {
        loopback: "http://127.0.0.1:8787",
        lan: "http://192.168.1.50:8787",
        tailnet: "https://pylon.tailnet.test",
      },
      controlToken: "control-token.mobile.test",
    }

    expect(pickConnect(registration)).toEqual({
      baseUrl: "https://pylon.tailnet.test",
      token: "control-token.mobile.test",
    })
  })

  test("parses valid nodes response", () => {
    expect(parseNodesResponse({
      nodes: [
        {
          id: "node.mobile.test",
          name: "Mobile Test Node",
          addresses: {
            lan: "http://192.168.1.50:8787",
            tailnet: "https://pylon.tailnet.test",
          },
          controlToken: "control-token.mobile.test",
        },
      ],
    })).toEqual([
      {
        id: "node.mobile.test",
        name: "Mobile Test Node",
        addresses: {
          lan: "http://192.168.1.50:8787",
          tailnet: "https://pylon.tailnet.test",
        },
        controlToken: "control-token.mobile.test",
      },
    ])
  })

  test("rejects invalid nodes responses", () => {
    expect(() => parseNodesResponse({})).toThrow("bad nodes response")
    expect(() => parseNodesResponse({ nodes: "not an array" })).toThrow("bad nodes response")
    expect(() => parseNodesResponse({ nodes: [null] })).toThrow("bad node registration")
    expect(() =>
      parseNodesResponse({
        nodes: [
          {
            addresses: { tailnet: "https://pylon.tailnet.test" },
          },
        ],
      })
    ).toThrow("bad node registration")
    expect(() =>
      parseNodesResponse({
        nodes: [
          {
            addresses: {},
            controlToken: "control-token.mobile.test",
          },
        ],
      })
    ).toThrow("bad node registration")
  })

  test("carries control token into connect info", () => {
    const [registration] = parseNodesResponse({
      nodes: [
        {
          addresses: {
            lan: "http://192.168.1.50:8787",
          },
          controlToken: "control-token.mobile.test",
        },
      ],
    })

    expect(pickConnect(registration)).toEqual({
      baseUrl: "http://192.168.1.50:8787",
      token: "control-token.mobile.test",
    })
  })
})
