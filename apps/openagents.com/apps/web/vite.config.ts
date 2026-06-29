import { foldkit } from '@foldkit/vite-plugin'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'

// #6046: theming is centralized in @openagentsinc/design-tokens (the typed
// --oa-* token source, re-exported via @openagentsinc/ui/tokens) with no build
// plugin. The @openagentsinc/ui and @openagentsinc/autopilot-ui component
// stylesheets (and the design-tokens theme.css :root projection) are imported
// from src/styles.css as plain CSS, so Tailwind/Vite bundle them normally — no
// style compiler plugin is needed here.

const devApiOrigin =
  process.env.OPENAGENTS_WEB_DEV_API_ORIGIN ?? 'https://openagents.com'

export default defineConfig({
  build: {
    chunkSizeWarningLimit: 600,
    rollupOptions: {
      output: {
        assetFileNames: assetInfo =>
          assetInfo.names.some(name => name.endsWith('.css'))
            ? 'assets/openagents.css'
            : 'assets/[name]-[hash][extname]',
      },
    },
  },
  plugins: [tailwindcss(), foldkit({ devToolsMcpPort: 9988 })],
  optimizeDeps: {
    entries: ['src/entry.ts'],
  },
  server: {
    proxy: {
      '/api': {
        changeOrigin: true,
        secure: true,
        target: devApiOrigin,
      },
    },
  },
})
