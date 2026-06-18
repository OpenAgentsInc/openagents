import { existsSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

import config from "../electrobun.config"

const mokshaAssetSource =
  "node_modules/@openagentsinc/three-effect/packages/core/src/assets/moksha"

describe("ElectroBun packaging", () => {
  test("copies shared three-effect Moksha assets into the desktop view bundle", () => {
    expect(config.build.copy[mokshaAssetSource]).toBe(
      "views/autopilot-desktop/assets/moksha",
    )
    expect(existsSync(join(process.cwd(), mokshaAssetSource, "diamond.glb"))).toBe(
      true,
    )
  })
})
