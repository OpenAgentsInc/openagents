import { foldkit } from '@foldkit/vite-plugin'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'

// #6046: StyleX removed. The @openagentsinc/ui and @openagentsinc/autopilot-ui
// packages were already listed as StyleX `externalPackages` here, so their
// StyleX `create()` blocks were never compiled into this app's CSS — the
// components render from their own Tailwind utility classes. Removing the
// StyleX plugin is therefore a no-op for the rendered output, and drops the
// `@stylexjs/*` dependency from the web build.

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
})
