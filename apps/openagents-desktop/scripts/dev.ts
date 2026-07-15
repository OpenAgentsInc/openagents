import { Runtime } from "@openagentsinc/runtime-platform"
import path from "node:path"
import { createServer } from "vite"
import { buildDesktop } from "./build.ts"
import { desktopDevServerHost, desktopDevServerPort } from "../vite.config.ts"

const appRoot = path.resolve(import.meta.dirname, "..")

await buildDesktop()

const server = await createServer({
  configFile: path.join(appRoot, "vite.config.ts"),
  root: appRoot,
})
await server.listen()

const devServerUrl = `http://${desktopDevServerHost}:${desktopDevServerPort}`
console.log(`[openagents-desktop] renderer dev server ready at ${devServerUrl}`)

const electron = Runtime.spawn(["electron", "."], {
  cwd: appRoot,
  env: {
    ...process.env,
    OPENAGENTS_DESKTOP_DEV_SERVER_URL: devServerUrl,
  },
  stdin: "inherit",
  stdout: "inherit",
  stderr: "inherit",
})

const stop = (): void => electron.kill("SIGTERM")
process.once("SIGINT", stop)
process.once("SIGTERM", stop)

try {
  process.exitCode = await electron.exited
} finally {
  await server.close()
}
