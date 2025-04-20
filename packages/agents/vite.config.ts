import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { cloudflare } from "@cloudflare/vite-plugin";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [cloudflare(), react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  build: {
    rollupOptions: {
      // Externalize all the Effect packages for Workers deployment
      external: [
        "@effect/experimental/Sse",
        "@effect/platform/HttpBody",
        "@effect/platform",
        "@effect/experimental",
        "@effect/ai-anthropic",
        "@effect/ai-openai",
        "@effect/ai",
        "effect"
      ]
    },
    // Increase chunk size limit to avoid warnings
    chunkSizeWarningLimit: 2000
  }
});
