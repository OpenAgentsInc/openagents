import type { StorybookConfig } from '@storybook/react-vite';
import { mergeConfig } from 'vite';
import path from 'node:path';
import tailwind from '@tailwindcss/vite';

const config: StorybookConfig = {
  stories: [
    '../src/stories/**/*.mdx',
    '../src/stories/**/*.stories.@(js|jsx|mjs|ts|tsx)'
  ],
  staticDirs: ['../public'],
  addons: [
    '@storybook/addon-docs',
    '@storybook/addon-a11y'
  ],
  docs: { autodocs: false },
  framework: {
    name: '@storybook/react-vite',
    options: {}
  },
  typescript: { reactDocgen: false },
  async viteFinal(baseConfig) {
    const fromRoot = (...p: string[]) => path.resolve(process.cwd(), ...p);
    return mergeConfig(baseConfig, {
      plugins: [tailwind()],
      resolve: {
        alias: {
          '@': fromRoot('src'),
          // Mocks to avoid Tauri/Ollama in Storybook environment
          '@tauri-apps/api/core': fromRoot('src/__mocks__/tauri-api.ts'),
          '@/runtime/adapters/ollama-adapter': fromRoot('src/__mocks__/ollama-adapter.ts'),
          '@/runtime/useAcpRuntime': fromRoot('src/__mocks__/useAcpRuntime.ts'),
          '@/vendor/assistant-ui/external-store': fromRoot('src/__mocks__/external-store.ts'),
          // Third‑party pin to avoid subpath resolution issues in CI
          'nanoid/non-secure': fromRoot('node_modules/nanoid/non-secure/index.js'),
        },
      },
      server: {
        fs: {
          allow: [fromRoot('.'), fromRoot('..'), fromRoot('../packages')],
        },
      },
      build: { chunkSizeWarningLimit: 4096 },
      optimizeDeps: {
        include: ['secure-json-parse', 'nanoid/non-secure'],
        esbuildOptions: { target: 'es2023' },
      },
    });
  },
};
export default config;
