// @ts-check
import react from "@astrojs/react";
import cloudflare from "@astrojs/cloudflare";
import mdx from "@astrojs/mdx";
import sitemap from "@astrojs/sitemap";
import tailwindcss from "@tailwindcss/vite";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";
import { defineConfig, envField } from "astro/config";

// https://astro.build/config
export default defineConfig({
  site: "https://web-ct8.pages.dev",
  trailingSlash: "never",
  build: { format: "file" },
  adapter: cloudflare({
    platformProxy: { enabled: true },
    imageService: "compile",
  }),
  integrations: [react(), mdx(), sitemap()],
  redirects: {
    "/posts": "/feed",
    "/sign-in": "/login",
    "/sign-up": "/signup",
    "/register": "/signup",
  },
  env: {
    schema: {
      CONVEX_URL: envField.string({
        access: "public",
        context: "client",
      }),
      CONVEX_SITE_URL: envField.string({
        access: "public",
        context: "client",
      }),
      VITE_BREEZ_API_KEY: envField.string({
        access: "public",
        context: "client",
      }),
    },
  },
  vite: {
    plugins: [tailwindcss(), wasm(), topLevelAwait()],
    optimizeDeps: {
      exclude: ["@breeztech/breez-sdk-spark"],
    },
    build: {
      target: "esnext",
    },
    server: {
      headers: {
        "Cross-Origin-Embedder-Policy": "require-corp",
        "Cross-Origin-Opener-Policy": "same-origin",
      },
    },
  },
});
