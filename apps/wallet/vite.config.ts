import { defineConfig, PluginOption } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'
// import wasm from "vite-plugin-wasm"; // Commented out
// import topLevelAwait from "vite-plugin-top-level-await"; // Commented out

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    // wasm(), // Commented out
    // topLevelAwait() // Commented out
  ] as PluginOption[],
  // optimizeDeps: { // Commented out
  //   exclude: ['@breeztech/breez-sdk-liquid']
  // },
  define: {
    'process.env': {},
  },
  build: {
    target: 'esnext',
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
})
