import { defineConfig } from 'vite';
import laravel from 'laravel-vite-plugin';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  resolve: {
    alias: {
      "agentgraph": path.resolve(__dirname, "agentgraph"),
      "@": path.resolve(__dirname, "resources/js"),
    }
  },
  plugins: [
    laravel({
      input: ['resources/js/app.tsx', 'resources/css/app.css'],
      refresh: true,
    }),
    react(),
  ],
});
