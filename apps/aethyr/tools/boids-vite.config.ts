import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig({
  base: "./",
  publicDir: false,
  server: { host: "127.0.0.1", port: 55336, strictPort: true },
  build: {
    outDir: "dist/boids",
    rollupOptions: { input: resolve("boids.html") },
  },
});
