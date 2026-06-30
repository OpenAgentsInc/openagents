import { describe, expect, test } from "bun:test"

import config from "../electrobun.config.js"

describe("openagents desktop blank shell", () => {
  test("registers the OpenAgents desktop view", () => {
    expect(config.app).toMatchObject({
      identifier: "com.openagents.desktop",
      name: "OpenAgents",
      version: "0.1.0",
    })
    expect(config.build.bun.entrypoint).toBe("src/bun/index.ts")
    expect(config.build.views["openagents-desktop"]).toMatchObject({
      entrypoint: "resources/ui/main.js",
    })
    expect(config.build.copy).toMatchObject({
      "resources/ui/main.css": "views/openagents-desktop/main.css",
      "src/ui/index.html": "views/openagents-desktop/index.html",
    })
  })

  test("renders only a blank application surface", async () => {
    const html = await Bun.file(
      new URL("../src/ui/index.html", import.meta.url),
    ).text()

    expect(html).toContain('id="openagents-blank-screen"')
    expect(html).toContain('<script type="module" src="./main.js"></script>')
    expect(html).not.toContain("<button")
    expect(html).not.toContain("<input")
    expect(html).not.toContain("<textarea")
    expect(html).not.toContain("Pylon")
    expect(html).not.toContain("Khala")
    expect(html).not.toContain("Codex")
  })
})
