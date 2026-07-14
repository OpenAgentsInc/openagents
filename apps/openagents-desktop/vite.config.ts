import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig, type PluginOption, type UserConfig } from "vite"

export const desktopRendererPlugins = (): Array<PluginOption> => [react(), tailwindcss()]

export const desktopRendererResolve: NonNullable<UserConfig["resolve"]> = {
  dedupe: ["react", "react-dom"],
}

/** Conventional Vite configuration shared by the build and component tooling. */
export default defineConfig({
  plugins: desktopRendererPlugins(),
  resolve: desktopRendererResolve,
})
