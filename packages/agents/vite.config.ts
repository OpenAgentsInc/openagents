import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { cloudflare } from "@cloudflare/vite-plugin";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  build: {
    // We'll bundle all dependencies for the Cloudflare Worker
    rollupOptions: {
      // No externals - this way everything gets bundled
    }
  },
  plugins: [cloudflare(), react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
