// @ts-check

import path from 'node:path';
import { fileURLToPath } from 'node:url';

import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';
import { defineConfig } from 'astro/config';

import cloudflare from '@astrojs/cloudflare';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// https://astro.build/config
export default defineConfig({
  site: 'https://openagents.com',
  trailingSlash: 'never',
  build: {
    format: 'file', // Generate page.html so /login works without trailing slash on Cloudflare
  },
  integrations: [mdx(), sitemap()],
  redirects: {
    '/sign-in': '/login',
    '/login/': '/login',
    '/sign-up': '/register',
    '/sign-up/': '/register',
    '/signup': '/register',
    '/signup/': '/register',
    '/register/': '/register',
  },
  vite: {
    resolve: {
      // better-auth 1.4.x ships Kysely adapter but does not export "adapters/kysely" in package.json
      alias: {
        'better-auth/adapters/kysely': path.join(
          __dirname,
          'node_modules/better-auth/dist/adapters/kysely-adapter/index.mjs'
        ),
      },
    },
  },
  adapter: cloudflare({
    platformProxy: {
      enabled: true
    },

    imageService: "compile"
  }),
});
