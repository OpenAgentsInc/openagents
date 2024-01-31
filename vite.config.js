import { defineConfig } from 'vite';
import laravel from 'laravel-vite-plugin';
import path from 'path';

export default defineConfig(({ command }) => {
  let alias = {}

  // dev environment
  if (command === 'serve') {
    Object.assign(alias, { '/fonts': path.resolve(__dirname, 'public/fonts') });
  }

  return {
    plugins: [
      laravel({
        input: ['resources/js/app.js', 'resources/css/app.css'],
        refresh: true,
      }),
    ],
  }
});
