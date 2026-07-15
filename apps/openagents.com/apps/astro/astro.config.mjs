import { defineConfig } from 'astro/config'

export default defineConfig({
  base: '/astro',
  server: {
    port: Number(process.env.PORT ?? 4321),
  },
  trailingSlash: 'never',
})
