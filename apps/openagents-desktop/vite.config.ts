import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import path from "node:path"
import { defineConfig, type Plugin, type PluginOption, type UserConfig } from "vite"
import { classifyDesktopPreviewChange } from "./src/dev-preview-contract.ts"

export const desktopRendererPlugins = (): Array<PluginOption> => [react(), tailwindcss()]

export const desktopRendererResolve: NonNullable<UserConfig["resolve"]> = {
  dedupe: ["react", "react-dom"],
}

export const desktopDevServerHost = "127.0.0.1"
export const desktopDevServerPort = 5734

export const desktopPreviewHmrPlugin = (repositoryRoot: string): Plugin => ({
  name: "openagents-desktop-preview-hmr-boundary",
  handleHotUpdate(context) {
    const pathRef = path.relative(repositoryRoot, path.resolve(context.file)).split(path.sep).join("/")
    const kind = classifyDesktopPreviewChange(pathRef)
    if (kind === "css_hmr" || kind === "react_fast_refresh") return
    context.server.ws.send({ type: "custom", event: "openagents:preview-change", data: { kind, pathRef } })
    return []
  },
})

/** Conventional Vite configuration shared by the build and component tooling. */
export default defineConfig(({ mode }) => {
  const preview = mode === "openagents-preview"
  const repositoryRoot = path.resolve(import.meta.dirname, "../..")
  return {
    plugins: [
      ...desktopRendererPlugins(),
      ...(preview ? [desktopPreviewHmrPlugin(repositoryRoot)] : []),
    ],
    resolve: desktopRendererResolve,
    server: {
      host: desktopDevServerHost,
      port: preview ? 0 : desktopDevServerPort,
      strictPort: !preview,
      hmr: preview
        ? { host: desktopDevServerHost }
        : {
            host: desktopDevServerHost,
            port: desktopDevServerPort,
            clientPort: desktopDevServerPort,
          },
    },
  }
})
