import { foldkit } from '@foldkit/vite-plugin'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'

// #6046: theming is centralized in @openagentsinc/design-tokens (the typed
// --oa-* token source, re-exported via @openagentsinc/ui/tokens) with no build
// plugin. The @openagentsinc/ui component stylesheet (which imports the
// design-tokens theme.css :root projection itself) is imported from
// src/styles.css as plain CSS, so Tailwind/Vite bundles it normally — no style
// compiler plugin is needed here. openagents#8813 Lane C dropped this app's
// direct @openagentsinc/design-tokens and @openagentsinc/autopilot-ui CSS
// imports/deps: the former was a redundant duplicate of what @openagentsinc/ui
// already pulls in, and the latter's oa-autopilot-domain-*/oa-autopilot-
// session-* classes were unused dead weight (@openagentsinc/ui is still a
// load-bearing dependency for most of this app and stays).

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
