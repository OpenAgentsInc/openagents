/* @vitest-environment node */

import { describe, expect, it } from "vitest"
import { html, joinTemplates, rawHtml, renderToString } from "../src/index.ts"

describe("renderToString", () => {
  it("renders nested templates", () => {
    const inner = html`<span>${"ok"}</span>`
    const outer = html`<div>${inner}</div>`
    expect(renderToString(outer)).toMatchInlineSnapshot(
      `"<div><span>ok</span></div>"`
    )
  })

  it("escapes text insertions (SSR-safe, no DOM)", () => {
    const t = html`<div>${'<>&"\''}</div>`
    expect(renderToString(t)).toMatchInlineSnapshot(
      `"<div>&lt;&gt;&amp;&quot;&#39;</div>"`
    )
  })

  it("does not escape rawHtml()", () => {
    const icon = rawHtml(`<span data-x="1"></span>`)
    const t = html`<div>${icon}</div>`
    expect(renderToString(t)).toMatchInlineSnapshot(
      `"<div><span data-x=\"1\"></span></div>"`
    )
  })

  it("handles arrays and primitive insertions", () => {
    const t = html`<div>${[1, true, "a", null, undefined, html`<b>b</b>`]}</div>`
    expect(renderToString(t)).toMatchInlineSnapshot(`"<div>1truea<b>b</b></div>"`)
  })

  it("supports joining templates", () => {
    const joined = joinTemplates([html`<span>a</span>`, html`<span>b</span>`])
    expect(renderToString(joined)).toMatchInlineSnapshot(
      `"<span>a</span><span>b</span>"`
    )
  })
})
