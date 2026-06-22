import { foldkit } from '@foldkit/vite-plugin'
import stylex from '@stylexjs/unplugin'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'

type StylexViteOptions = NonNullable<Parameters<typeof stylex.vite>[0]> & {
  externalPackages?: ReadonlyArray<string>
}

const stylexOptions: StylexViteOptions = {
  useCSSLayers: true,
  runtimeInjection: false,
  externalPackages: ['@openagentsinc/ui', '@openagentsinc/autopilot-ui'],
  cssInjectionTarget: fileName => fileName.endsWith('.css'),
}

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
  plugins: [stylex.vite(stylexOptions), tailwindcss(), foldkit({ devToolsMcpPort: 9988 })],
  optimizeDeps: {
    entries: ['src/entry.ts'],
  },
})
