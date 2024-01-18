import { defineConfig } from 'vite';
import laravel from 'laravel-vite-plugin';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig(({ command, mode }) => {
  const alias = {
    "agentgraph": path.resolve(__dirname, "agentgraph"),
    "@": path.resolve(__dirname, "resources/js"),
  }

  // dev environment
  if (command === 'serve') {
    Object.assign(alias, { '/fonts': path.resolve(__dirname, 'public/fonts') });
  }

  return {
    resolve: {
      alias
    },
    plugins: [
      laravel({
        input: ['resources/js/app.tsx', 'resources/css/app.css', 'resources/css/regards.css'],
        refresh: true,
      }),
      react(),
    ],
  }
});
