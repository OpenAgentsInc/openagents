import { describe, expect, test } from "bun:test"

import { createInMemoryTokenStore } from "./token-store"

describe("in-memory token store", () => {
  test("returns null initially", async () => {
    const tokenStore = createInMemoryTokenStore()

    expect(await tokenStore.get()).toBeNull()
  })

  test("returns the stored token after set", async () => {
    const tokenStore = createInMemoryTokenStore()

    await tokenStore.set("dev.fixture.token")

    expect(await tokenStore.get()).toBe("dev.fixture.token")
  })

  test("returns null after clear", async () => {
    const tokenStore = createInMemoryTokenStore()

    await tokenStore.set("dev.fixture.token")
    await tokenStore.clear()

    expect(await tokenStore.get()).toBeNull()
  })
})
