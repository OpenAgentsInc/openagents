import { describe, expect, it } from "vitest"
import { cn } from "../src/core/utils/cn"

describe("cn utility", () => {
  it("combines class names", () => {
    expect(cn("foo", "bar")).toBe("foo bar")
  })

  it("handles conditional classes", () => {
    expect(cn("foo", false && "bar", "baz")).toBe("foo baz")
    expect(cn("foo", true && "bar", "baz")).toBe("foo bar baz")
  })

  it("handles undefined and null values", () => {
    expect(cn("foo", undefined, null, "bar")).toBe("foo bar")
  })

  it("merges Tailwind classes correctly", () => {
    expect(cn("text-red-500", "text-blue-500")).toContain("text-blue-500")
    expect(cn("text-red-500", "text-blue-500")).not.toContain("text-red-500")
  })

  it("handles arrays of classes", () => {
    expect(cn(["foo", "bar"], "baz")).toBe("foo bar baz")
  })

  it("handles empty inputs", () => {
    expect(cn()).toBe("")
    expect(cn("")).toBe("")
  })
})