/**
 * Owner-profile counterpart to launch-isolated-app.ts for the explicitly
 * armed FA-QA-01 real-provider batch. This intentionally launches the normal
 * OpenAgents Dev profile: it does not set a userData override, preview mode,
 * mock-keychain switches, smoke mode, or any native-vault migration flag.
 *
 * Callers must treat this as a stateful owner action. The profile is never
 * deleted or copied, and provider credentials are never read by this helper.
 * The only Claude-specific input is the explicit SDK-session arm; the SDK
 * remains responsible for its ordinary authentication custody and returns a
 * typed failure if that session is unusable.
 */
import { existsSync } from "node:fs"
import path from "node:path"

import { _electron as electron, type ElectronApplication, type Page } from "playwright"
import type { ViteDevServer } from "vite"
import { createServer } from "vite"

import { buildDesktop } from "../build.ts"
import { desktopDevServerHost, desktopDevServerPort } from "../../vite.config.ts"
import { appRoot } from "./launch-isolated-app.ts"

export type OwnerDesktopApp = Readonly<{
  app: ElectronApplication
  page: Page
  userDataPath: string
  close: () => Promise<void>
}>

export type LaunchOwnerDesktopOptions = Readonly<{
  launchCwd: string
  armDefaultClaudeSession?: boolean
  extraEnv?: Record<string, string>
}>

export const launchOwnerDesktopApp = async (
  options: LaunchOwnerDesktopOptions,
): Promise<OwnerDesktopApp> => {
  if (!existsSync(options.launchCwd)) {
    throw new Error(`[owner-ui-harness] launchCwd does not exist: ${options.launchCwd}`)
  }

  process.env.OA_DESKTOP_SKIP_DEV_VOICE_HELPER = "1"
  await buildDesktop()
  const viteServer: ViteDevServer = await createServer({
    configFile: path.join(appRoot, "vite.config.ts"),
    root: appRoot,
  })
  await viteServer.listen()
  const address = viteServer.httpServer?.address()
  const listeningPort =
    typeof address === "object" && address !== null ? address.port : desktopDevServerPort
  const devServerUrl = `http://${desktopDevServerHost}:${listeningPort}`
  const electronBinary = path.join(
    appRoot,
    "node_modules",
    "electron",
    "dist",
    "Electron.app",
    "Contents",
    "MacOS",
    "Electron",
  )
  if (!existsSync(electronBinary)) {
    await viteServer.close()
    throw new Error(`[owner-ui-harness] Electron binary missing at ${electronBinary}`)
  }

  const launchEnv = { ...(process.env as Record<string, string>) }
  delete launchEnv.OPENAGENTS_DESKTOP_USER_DATA
  delete launchEnv.OPENAGENTS_DESKTOP_PREVIEW
  delete launchEnv.OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF
  delete launchEnv.OPENAGENTS_DESKTOP_SMOKE
  delete launchEnv.OPENAGENTS_DESKTOP_LIVE_PROOF
  Object.assign(launchEnv, options.extraEnv ?? {}, {
    OPENAGENTS_DESKTOP_DEV_SERVER_URL: devServerUrl,
    OPENAGENTS_DESKTOP_LAUNCH_CWD: options.launchCwd,
  })
  if (options.armDefaultClaudeSession === true) {
    launchEnv.OPENAGENTS_DESKTOP_USE_DEFAULT_CLAUDE_SESSION = "1"
  } else {
    delete launchEnv.OPENAGENTS_DESKTOP_USE_DEFAULT_CLAUDE_SESSION
  }

  let app: ElectronApplication
  try {
    app = await electron.launch({
      executablePath: electronBinary,
      args: ["."],
      cwd: appRoot,
      env: launchEnv,
    })
  } catch (error) {
    await viteServer.close()
    throw error
  }
  const page = await app.firstWindow()
  await page.waitForLoadState("domcontentloaded")
  const userDataPath = await app.evaluate(({ app: electronApp }) =>
    electronApp.getPath("userData"))

  const close = async (): Promise<void> => {
    try {
      await app.close()
    } catch {
      // The batch deliberately exercises complete app shutdown.
    }
    try {
      await viteServer.close()
    } catch {
      // A failed provider turn must not strand the development server.
    }
  }
  return { app, page, userDataPath, close }
}
