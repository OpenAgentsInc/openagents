// Firm reason: 1 (Vite 8 + @effect/ui HtmlServerVite + DevToolsVite browser/web-server build config for docs/effect/half-day-sprint.md)
import { DevToolsVite } from "@effect/ui-devtools/node"
import { plugin as htmlServer } from "@effect/ui/html/HtmlServerVite"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from "vite"

export default defineConfig({
  build: {
    outDir: "dist/client"
  },
  plugins: [
    tailwindcss(),
    htmlServer({
      entry: "/src/server/entry.server.ts",
      serverOutDir: "dist/server"
    }),
    DevToolsVite.plugin({ dispatch: true })
  ]
})
