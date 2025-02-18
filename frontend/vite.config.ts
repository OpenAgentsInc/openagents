import { defineConfig } from "vite"
import tsconfigPaths from "vite-tsconfig-paths"
import { reactRouter } from "@react-router/dev/vite"
import tailwindcss from "@tailwindcss/vite"

export default defineConfig({
  plugins: [tailwindcss(), reactRouter(), tsconfigPaths()],
  build: {
    outDir: 'build/client',
    assetsDir: 'assets',
    rollupOptions: {
      output: {
        manualChunks: undefined
      }
    }
  },
  base: '/',
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:8000',
        changeOrigin: true
      },
      '/ws': {
        target: 'http://localhost:8000',
        ws: true
      }
    }
  }
});
