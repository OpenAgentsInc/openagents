import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  root: '.',
  resolve: {
    alias: {
      '@openagentsinc/ui': resolve(__dirname, '../ui/dist/webtui.css')
    }
  },
  server: {
    port: 5173,
    open: true
  }
});