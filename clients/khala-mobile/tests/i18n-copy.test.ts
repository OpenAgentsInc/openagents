import { describe, expect, test } from "bun:test"

import { enCopy, khalaCopyKeys, tx, type KhalaCopyKey } from "../src/i18n/copy"

describe("Khala mobile i18n copy keys", () => {
  test("English copy covers exactly the typed key list", () => {
    expect(Object.keys(enCopy).sort()).toEqual([...khalaCopyKeys].sort())
  })

  test("throws for a missing copy key at runtime", () => {
    expect(() => tx("missing.key" as KhalaCopyKey)).toThrow("Missing Khala mobile copy key")
  })

  test("returns the GitHub sign-in copy through typed keys", () => {
    expect(tx("signIn.github.primary")).toBe("Log in with GitHub")
  })

  test("copy table does not contain private examples or raw sync payload placeholders", () => {
    const joined = Object.values(enCopy).join("\n")
    expect(joined).not.toContain("oa_agent_")
    expect(joined).not.toContain("Bearer ")
    expect(joined).not.toContain("postImageJson")
    expect(joined).not.toContain("chat_message")
  })
})
