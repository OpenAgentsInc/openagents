import { describe, expect, test } from "bun:test"

import config from "../electrobun.config.js"

describe("openagents desktop app shell", () => {
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

  test("renders an initial pylon navigation surface", async () => {
    const html = await Bun.file(
      new URL("../src/ui/index.html", import.meta.url),
    ).text()

    expect(html).toContain('<button')
    expect(html).toContain('id="coding-status"')
    expect(html).toContain('id="coding-page"')
    expect(html).toContain('id="coding-active-list"')
    expect(html).toContain('id="coding-transcript"')
    expect(html).toContain('id="coding-transcript-messages"')
    expect(html).toContain("Coding: 0")
    expect(html).toContain('id="pylon-status"')
    expect(html).toContain('id="pylons-page"')
    expect(html).toContain("Create Pylon")
    expect(html).toContain("Pylons: 0")
  })
})
