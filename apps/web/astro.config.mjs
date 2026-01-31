// @ts-check
import react from "@astrojs/react";
import cloudflare from "@astrojs/cloudflare";
import mdx from "@astrojs/mdx";
import sitemap from "@astrojs/sitemap";
import tailwindcss from "@tailwindcss/vite";
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
      PUBLIC_CONVEX_SITE_URL: envField.string({
        access: "public",
        context: "client",
      }),
    },
  },
  vite: {
    plugins: [tailwindcss()],
  },
});
