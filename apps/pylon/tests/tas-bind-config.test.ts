import { describe, expect, test } from "bun:test"

import {
  isAuthRequiredForRemote,
  resolveBindAddresses,
} from "../src/node/bind-config"

describe("tas bind config", () => {
  test("loopback is always present and does not require auth", () => {
    expect(
      resolveBindAddresses({
        interfaces: {
          loopback: "127.0.0.1",
          lan: "192.168.1.10",
          tailnet: "100.64.0.10",
        },
        enableLan: false,
        enableTailnet: false,
      }),
    ).toEqual({
      binds: [
        {
          address: "127.0.0.1",
          requiresAuth: false,
        },
      ],
    })
  })

  test("lan and tailnet binds are present only when enabled and require auth", () => {
    expect(
      resolveBindAddresses({
        interfaces: {
          loopback: "127.0.0.1",
          lan: "192.168.1.10",
          tailnet: "100.64.0.10",
        },
        enableLan: true,
        enableTailnet: false,
      }),
    ).toEqual({
      binds: [
        {
          address: "127.0.0.1",
          requiresAuth: false,
        },
        {
          address: "192.168.1.10",
          requiresAuth: true,
        },
      ],
    })

    expect(
      resolveBindAddresses({
        interfaces: {
          loopback: "127.0.0.1",
          lan: "192.168.1.10",
          tailnet: "100.64.0.10",
        },
        enableLan: false,
        enableTailnet: true,
      }),
    ).toEqual({
      binds: [
        {
          address: "127.0.0.1",
          requiresAuth: false,
        },
        {
          address: "100.64.0.10",
          requiresAuth: true,
        },
      ],
    })

    expect(
      resolveBindAddresses({
        interfaces: {
          loopback: "127.0.0.1",
          lan: "192.168.1.10",
          tailnet: "100.64.0.10",
        },
        enableLan: true,
        enableTailnet: true,
      }),
    ).toEqual({
      binds: [
        {
          address: "127.0.0.1",
          requiresAuth: false,
        },
        {
          address: "192.168.1.10",
          requiresAuth: true,
        },
        {
          address: "100.64.0.10",
          requiresAuth: true,
        },
      ],
    })
  })

  test("remote addresses require auth", () => {
    expect(isAuthRequiredForRemote("127.0.0.1", "127.0.0.1")).toBe(false)
    expect(isAuthRequiredForRemote("192.168.1.10", "127.0.0.1")).toBe(true)
    expect(isAuthRequiredForRemote("100.64.0.10", "127.0.0.1")).toBe(true)
  })
})
