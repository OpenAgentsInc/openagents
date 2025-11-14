import path from "path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  resolve: {
    alias: [
      { find: "@", replacement: path.resolve(__dirname, "./src") },
      { find: "@openagentsinc/assistant-ui-runtime", replacement: path.resolve(__dirname, "../packages/assistant-ui-runtime/src/index.ts") },
      { find: "@openagentsinc/react-markdown/styles/dot.css", replacement: path.resolve(__dirname, "../packages/react-markdown/styles/dot.css") },
      { find: "@openagentsinc/react-markdown", replacement: path.resolve(__dirname, "../packages/react-markdown/src/index.ts") },
      { find: "@openagentsinc/ui/styles/theme.css", replacement: path.resolve(__dirname, "../packages/ui/src/styles/theme.css") },
      { find: "@openagentsinc/ui/styles/fonts.css", replacement: path.resolve(__dirname, "../packages/ui/src/styles/fonts.css") },
      { find: "@openagentsinc/ui", replacement: path.resolve(__dirname, "../packages/ui/src/index.ts") },
      { find: "@assistant-ui/tap/react", replacement: path.resolve(__dirname, "../packages/tap/src/react/index.ts") },
      { find: "@assistant-ui/tap", replacement: path.resolve(__dirname, "../packages/tap/src/index.ts") },
      { find: /^assistant-stream\/(.*)$/, replacement: path.resolve(__dirname, "../packages/assistant-stream/src/$1.ts") },
      { find: "assistant-stream", replacement: path.resolve(__dirname, "../packages/assistant-stream/src/index.ts") },
      { find: "assistant-cloud", replacement: path.resolve(__dirname, "./src/__stubs__/assistant-cloud.ts") },
      { find: "react-markdown", replacement: path.resolve(__dirname, "./node_modules/react-markdown") },
      { find: "classnames", replacement: path.resolve(__dirname, "./node_modules/classnames") },
      { find: "@radix-ui/react-use-callback-ref", replacement: path.resolve(__dirname, "./node_modules/@radix-ui/react-use-callback-ref") },
    ],
    // Allow resolving modules from tauri's node_modules even when importing from packages
    preserveSymlinks: false,
  },

  // Optimize dependency pre-bundling
  optimizeDeps: {
    include: [
      "react-markdown",
      "classnames",
      "@radix-ui/react-use-callback-ref",
    ],
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    // Allow env override for better DX when coordinating with Tauri/mobile.
    // Default remains 1420 to match tauri.conf.json devUrl.
    port: Number(process.env.VITE_PORT ?? 1420),
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: Number(process.env.VITE_HMR_PORT ?? 1421),
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
