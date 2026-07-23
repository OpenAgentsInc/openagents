import { resolve } from "node:path";
import { defineConfig } from "vite-plus/test/config";

export default defineConfig({
  test: {
    environment: "node",
    setupFiles: [resolve(import.meta.dirname, "../../scripts/vp3-vitest-setup.ts")],
  },
});
