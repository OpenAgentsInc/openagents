import { describe, expect, it } from "bun:test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  hasSearchedForumWork,
  loadWorkSearchReceipt,
  searchForumWork,
  type WorkSearchFetch,
} from "../src/bun/forum-work-search"

const home = () => mkdtempSync(join(tmpdir(), "fws-"))

const workResponse = (
  states: string[],
) =>
  ({
    status: 200,
    json: async () => ({
      workRequests: states.map((state, i) => ({
        workRequestId: `wr_${i}`,
        title: `work ${i}`,
        state,
      })),
    }),
  }) as const

describe("searchForumWork (AF-4 read-only discovery)", () => {
  it("counts open items by typed state and persists a receipt (read-only GET)", async () => {
    const h = home()
    try {
      let method = ""
      let url = ""
      const res = await searchForumWork({
        home: h,
        baseUrl: "https://openagents.com",
        fetchImpl: (async (u, init) => {
          method = init.method
          url = u
          return workResponse(["open", "open", "running", "quote_received"])
        }) as WorkSearchFetch,
      })
      expect(method).toBe("GET")
      expect(url).toContain("/api/forum/work-requests")
      expect(res.outcome).toBe("searched")
      if (res.outcome === "searched") {
        expect(res.openCount).toBe(2)
        expect(res.totalCount).toBe(4)
      }
      expect(hasSearchedForumWork(h)).toBe(true)
      expect(loadWorkSearchReceipt(h)?.openCount).toBe(2)
    } finally {
      rmSync(h, { recursive: true, force: true })
    }
  })

  it("records an honest empty state (0 open) as a successful search", async () => {
    const h = home()
    try {
      const res = await searchForumWork({
        home: h,
        fetchImpl: (async () => workResponse([])) as WorkSearchFetch,
      })
      expect(res.outcome).toBe("searched")
      expect(hasSearchedForumWork(h)).toBe(true)
      expect(loadWorkSearchReceipt(h)?.openCount).toBe(0)
    } finally {
      rmSync(h, { recursive: true, force: true })
    }
  })

  it("preserves firstSearchedAt across repeated searches", async () => {
    const h = home()
    try {
      await searchForumWork({
        home: h,
        fetchImpl: (async () => workResponse(["open"])) as WorkSearchFetch,
      })
      const first = loadWorkSearchReceipt(h)?.firstSearchedAt
      await searchForumWork({
        home: h,
        fetchImpl: (async () => workResponse(["open", "open"])) as WorkSearchFetch,
      })
      const after = loadWorkSearchReceipt(h)
      expect(after?.firstSearchedAt).toBe(first!)
      expect(after?.openCount).toBe(2)
    } finally {
      rmSync(h, { recursive: true, force: true })
    }
  })

  it("is offline-tolerant: a network error defers and persists nothing", async () => {
    const h = home()
    try {
      const res = await searchForumWork({
        home: h,
        fetchImpl: (async () => {
          throw new Error("offline")
        }) as WorkSearchFetch,
      })
      expect(res.outcome).toBe("deferred")
      expect(hasSearchedForumWork(h)).toBe(false)
    } finally {
      rmSync(h, { recursive: true, force: true })
    }
  })

  it("defers on a non-200 status", async () => {
    const h = home()
    try {
      const res = await searchForumWork({
        home: h,
        fetchImpl: (async () => ({
          status: 503,
          json: async () => ({}),
        })) as WorkSearchFetch,
      })
      expect(res.outcome).toBe("deferred")
    } finally {
      rmSync(h, { recursive: true, force: true })
    }
  })
})
