import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import path from "node:path"

import config, { desktopDevServerHost, desktopDevServerPort } from "../vite.config.ts"

const root = path.resolve(import.meta.dirname, "..")

describe("OpenAgents Desktop renderer dev loop", () => {
  test("uses one strict loopback Vite server and HMR endpoint", () => {
    expect(desktopDevServerHost).toBe("127.0.0.1")
    expect(desktopDevServerPort).toBe(5734)
    expect(config.server).toMatchObject({
      host: "127.0.0.1",
      port: 5734,
      strictPort: true,
      hmr: { host: "127.0.0.1", port: 5734, clientPort: 5734 },
    })
  })

  test("keeps the production renderer origin while proxying only development assets", () => {
    const main = readFileSync(path.join(root, "src", "main.ts"), "utf8")
    expect(main).toContain("OPENAGENTS_DESKTOP_DEV_SERVER_URL")
    expect(main).toContain('url.hostname !== "127.0.0.1"')
    expect(main).toContain('asset === "index.html" ? "/index.dev.html"')
    expect(main).toContain("bypassCustomProtocolHandlers: true")
    expect(main).toContain("if (app.isPackaged) return null")
    expect(main).toContain('path.join(app.getPath("appData"), "OpenAgents Dev")')
    expect(main).toContain("desktopDevServerUrl !== null")
  })

  test("launches Electron only after Vite listens and retains the typed React entry", () => {
    const runner = readFileSync(path.join(root, "scripts", "dev.ts"), "utf8")
    const html = readFileSync(path.join(root, "index.dev.html"), "utf8")
    expect(runner.indexOf("await server.listen()")).toBeLessThan(runner.indexOf("Runtime.spawn"))
    expect(runner).toContain('OPENAGENTS_DESKTOP_DEV_SERVER_URL: devServerUrl')
    expect(html).toContain('src="/src/renderer/boot.ts"')
    expect(html).toContain("ws://127.0.0.1:5734")
  })
})
