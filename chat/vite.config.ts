import { defineConfig } from "vite"
import react from "@vitejs/plugin-react"

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  base: '/chat/',
  server: {
    proxy: {
      '/ws': {
        target: 'http://localhost:8000',
        ws: true,
      }
    }
  }
})
