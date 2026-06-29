import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

import { DESKTOP_RPC_MAX_REQUEST_TIME_MS } from "../src/shared/rpc"
import { desktopApplicationMenu } from "../src/bun/application-menu"
import config, {
  mokshaAssetSource,
  threePlayerControllerAssetSource,
} from "../electrobun.config"

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

  test("copies shared three-effect controller GLB assets into the desktop view bundle", () => {
    expect(config.build.copy[threePlayerControllerAssetSource]).toBe(
      "views/autopilot-desktop/assets/three-player-controller",
    )
    expect(
      existsSync(
        join(process.cwd(), threePlayerControllerAssetSource, "UEPerson.glb"),
      ),
    ).toBe(true)
  })

  test("uses an explicit RPC timeout long enough for live shell model turns", () => {
    expect(DESKTOP_RPC_MAX_REQUEST_TIME_MS).toBeGreaterThanOrEqual(30_000)

    const bunEntry = readFileSync(join(process.cwd(), "src/bun/index.ts"), "utf8")
    const viewEntry = readFileSync(join(process.cwd(), "src/ui/main.ts"), "utf8")

    expect(bunEntry).toContain("maxRequestTime: DESKTOP_RPC_MAX_REQUEST_TIME_MS")
    expect(viewEntry).toContain("maxRequestTime: DESKTOP_RPC_MAX_REQUEST_TIME_MS")
  })

  test("installs native edit menu accelerators for WebKit text editing", () => {
    const edit = desktopApplicationMenu.find(
      (item) => "label" in item && item.label === "Edit",
    ) as { submenu?: Array<{ role?: string; accelerator?: string }> } | undefined
    expect(edit).toBeDefined()
    const byRole = new Map(
      (edit?.submenu ?? [])
        .filter((item) => typeof item.role === "string")
        .map((item) => [item.role, item]),
    )

    expect(byRole.get("copy")?.accelerator).toBe("CommandOrControl+C")
    expect(byRole.get("paste")?.accelerator).toBe("CommandOrControl+V")
    expect(byRole.get("cut")?.accelerator).toBe("CommandOrControl+X")
    expect(byRole.get("selectAll")?.accelerator).toBe("CommandOrControl+A")
    expect(byRole.get("undo")?.accelerator).toBe("CommandOrControl+Z")

    const bunEntry = readFileSync(join(process.cwd(), "src/bun/index.ts"), "utf8")
    expect(bunEntry).toContain(
      "ApplicationMenu.setApplicationMenu(desktopApplicationMenu)",
    )
  })
})
