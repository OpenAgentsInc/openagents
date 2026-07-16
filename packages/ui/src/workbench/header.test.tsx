import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vite-plus/test"

import { DesktopConversationHeader } from "./header.tsx"

describe("DesktopConversationHeader meter mount (#8868, T11)", () => {
  it("renders no ContextMeter when the host does not pass a meter (back-compat)", () => {
    const html = renderToStaticMarkup(
      <DesktopConversationHeader lifecycle="Ready" title="Session" />,
    )
    expect(html).not.toContain("oa-react-meter")
  })

  it("mounts the live ContextMeter with the host's exact usage/rateLimits when passed", () => {
    const html = renderToStaticMarkup(
      <DesktopConversationHeader
        lifecycle="Running"
        meter={{
          usage: { totalTokens: 12_847 },
          rateLimits: [{ label: "primary", usedPercent: 33 }],
        }}
        title="Session"
      />,
    )
    expect(html).toContain("oa-react-meter")
    expect(html).toContain("12,847")
    expect(html).toContain("PRIMARY")
    expect(html).toContain('data-historical="false"')
  })

  it("renders the honest NO DATA meter state when the host passes an empty meter object", () => {
    const html = renderToStaticMarkup(
      <DesktopConversationHeader lifecycle="Ready" meter={{}} title="Session" />,
    )
    expect(html).toContain("NO DATA")
  })
})
