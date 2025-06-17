import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@openagentsinc/ui': path.resolve(__dirname, '../ui/src')
    }
  },
  server: {
    fs: {
      // Allow serving files from parent directory
      allow: ['..']
    }
  }
})
