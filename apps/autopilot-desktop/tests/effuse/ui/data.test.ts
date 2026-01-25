import { describe, expect, it } from "vitest"
import { getByPath, resolveDynamicObject, setByPath } from "../../../src/effuse/ui/data.js"

describe("data helpers", () => {
  it("gets values by path", () => {
    const model = { user: { name: "Ada" }, flags: { ready: true } }
    expect(getByPath(model, "/user/name")).toBe("Ada")
    expect(getByPath(model, "flags/ready")).toBe(true)
  })

  it("sets values by path", () => {
    const model: Record<string, unknown> = {}
    setByPath(model, "/session/id", "abc")
    setByPath(model, "/session/count", 2)
    expect(model).toEqual({ session: { id: "abc", count: 2 } })
  })

  it("resolves dynamic values recursively", () => {
    const model = { profile: { name: "Tess" }, count: 3 }
    const resolved = resolveDynamicObject(
      {
        label: { path: "/profile/name" },
        items: [{ path: "/count" }, 1],
      },
      model
    )
    expect(resolved).toEqual({ label: "Tess", items: [3, 1] })
  })
})
