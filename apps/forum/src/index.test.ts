import { describe, expect, it } from "bun:test"
import { Effect } from "effect"

import { defaultForumMount, describeForumMount } from "./index.ts"

describe("forum mount contract", () => {
  it("keeps the forum mounted under openagents.com/forum", async () => {
    const description = await Effect.runPromise(
      describeForumMount(defaultForumMount),
    )

    expect(description).toBe("openagents.com/forum")
  })
})
