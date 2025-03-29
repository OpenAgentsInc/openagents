import { defineConfig } from "vite";
import { cloudflare } from "@cloudflare/vite-plugin";

export default defineConfig({
  plugins: [cloudflare()],
  build: {
    sourcemap: true,
    outDir: "dist",
    emptyOutDir: true,
  }
});
