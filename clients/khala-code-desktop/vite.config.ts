import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// The Khala Code Desktop webview is built with Vite so Tailwind (and Basecoat's
// @apply) compile, matching apps/openagents.com/apps/web and the electrobun
// starter. Electrobun copies dist/ into views/khala-code-desktop/.
export default defineConfig({
  root: "src/ui",
  plugins: [react(), tailwindcss()],
  build: {
    outDir: "../../dist",
    emptyOutDir: true,
  },
})
