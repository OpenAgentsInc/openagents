import { defineConfig } from 'vite';
import laravel from 'laravel-vite-plugin';
import path from 'path';

export default defineConfig(({ command, mode }) => {
  // dev environment
  if (command === 'serve') {
    Object.assign(alias, { '/fonts': path.resolve(__dirname, 'public/fonts') });
  }

  return {
    resolve: { alias },
    plugins: [
      laravel({
        input: ['resources/css/app.css'],
        refresh: true,
      }),
    ],
  }
});
