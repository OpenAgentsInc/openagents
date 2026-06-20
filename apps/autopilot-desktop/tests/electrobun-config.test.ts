import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

import { DESKTOP_RPC_MAX_REQUEST_TIME_MS } from "../src/shared/rpc"
import config from "../electrobun.config"

const mokshaAssetSource =
  "node_modules/@openagentsinc/three-effect/packages/core/src/assets/moksha"

describe("ElectroBun packaging", () => {
  test("uses the concise Autopilot app title", () => {
    expect(config.app.name).toBe("Autopilot")
  })

  test("copies shared three-effect Moksha assets into the desktop view bundle", () => {
    expect(config.build.copy[mokshaAssetSource]).toBe(
      "views/autopilot-desktop/assets/moksha",
    )
    expect(existsSync(join(process.cwd(), mokshaAssetSource, "diamond.glb"))).toBe(
      true,
    )
  })

  test("uses an explicit RPC timeout long enough for live shell model turns", () => {
    expect(DESKTOP_RPC_MAX_REQUEST_TIME_MS).toBeGreaterThanOrEqual(30_000)

    const bunEntry = readFileSync(join(process.cwd(), "src/bun/index.ts"), "utf8")
    const viewEntry = readFileSync(join(process.cwd(), "src/ui/main.ts"), "utf8")

    expect(bunEntry).toContain("maxRequestTime: DESKTOP_RPC_MAX_REQUEST_TIME_MS")
    expect(viewEntry).toContain("maxRequestTime: DESKTOP_RPC_MAX_REQUEST_TIME_MS")
  })
})
