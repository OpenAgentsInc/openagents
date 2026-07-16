import { Runtime } from "@openagentsinc/runtime-platform"
import type { AddressInfo } from "node:net"
import path from "node:path"
import { createServer } from "vite"
import { buildDesktop } from "./build.ts"
import { desktopDevServerHost, desktopDevServerPort } from "../vite.config.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const preview = process.env.OPENAGENTS_DESKTOP_PREVIEW === "1"

await buildDesktop()

const server = await createServer({
  configFile: path.join(appRoot, "vite.config.ts"),
  root: appRoot,
  mode: preview ? "openagents-preview" : "development",
})
await server.listen()

const listeningAddress = server.httpServer?.address()
const listeningPort = typeof listeningAddress === "object" && listeningAddress !== null
  ? (listeningAddress as AddressInfo).port
  : desktopDevServerPort
const devServerUrl = `http://${desktopDevServerHost}:${listeningPort}`
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
