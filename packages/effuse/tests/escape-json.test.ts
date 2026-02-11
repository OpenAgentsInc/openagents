import { describe, expect, it } from "@effect/vitest"

import { escapeJsonForHtmlScript } from "../src/template/escape.js"

describe("escapeJsonForHtmlScript", () => {
  it("prevents </script> from appearing in embedded JSON", () => {
    const json = JSON.stringify({ x: "</script><script>alert(1)</script>" })
    const escaped = escapeJsonForHtmlScript(json)

    expect(escaped).not.toContain("</script>")
    expect(JSON.parse(escaped)).toEqual({ x: "</script><script>alert(1)</script>" })
  })

  it("preserves JSON parse semantics for normal strings", () => {
    const json = JSON.stringify({ x: "<hello>" })
    const escaped = escapeJsonForHtmlScript(json)

    expect(JSON.parse(escaped)).toEqual({ x: "<hello>" })
  })
})

