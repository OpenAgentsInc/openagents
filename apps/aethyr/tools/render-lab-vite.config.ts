import { defineConfig } from "vite";
import { resolve } from "node:path";

// The render and job labs are served cross-origin isolated (COOP + COEP), the
// primary web profile in decision 0006. That also gives finer timers.
const isolation = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

export default defineConfig({
  base: "./",
  publicDir: false,
  server: { host: "127.0.0.1", port: 55337, strictPort: true, headers: isolation },
  preview: { host: "127.0.0.1", port: 55338, strictPort: true, headers: isolation },
  build: {
    outDir: "dist/render-lab",
    target: "es2022",
    rollupOptions: { input: { "render-lab": resolve("render-lab.html"), "job-lab": resolve("job-lab.html") } },
  },
});
