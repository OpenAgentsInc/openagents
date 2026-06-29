import { describe, expect, test } from "bun:test"

import config from "../electrobun.config.js"

describe("openagents desktop app shell", () => {
  test("registers the blank OpenAgents desktop view", () => {
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
      "src/ui/index.html": "views/openagents-desktop/index.html",
    })
  })
})
