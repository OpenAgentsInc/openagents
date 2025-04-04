import { defineConfig } from "vite";
import path from "path";

// https://vitejs.dev/config
export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
      'react-native': 'react-native-web',
    }
  },
  // Ensure native Node.js modules like 'fs-extra' are not bundled.
  // Electron Forge's Vite plugin should handle nodeIntegration correctly,
  // but explicitly externalizing can prevent bundling issues.
  build: {
    rollupOptions: {
      external: [
        'electron', // Standard externals for Electron apps
        'node:path',
        'node:fs',
        'node:fs/promises',
        'node:child_process',
        'path',
        'fs',
        'os',
        // Add other Node built-ins or large deps if needed
      ]
    }
  },
  optimizeDeps: {
    esbuildOptions: {
      loader: {
        '.js': 'jsx',
      },
    },
  },
});
