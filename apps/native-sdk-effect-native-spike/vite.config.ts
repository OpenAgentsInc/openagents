import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";

export default defineConfig({
  root: fileURLToPath(new URL("./frontend", import.meta.url)),
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
