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
          // Use published assistant-ui for Storybook instead of local runtime package
          '@openagentsinc/assistant-ui-runtime': '@assistant-ui/react',
          '@openagentsinc/react-markdown': '@assistant-ui/react-markdown',
          '@openagentsinc/react-markdown/styles/dot.css': '@assistant-ui/react-markdown/styles/dot.css',
          // Force assistant-stream to resolve from installed package, not monorepo sources
          'assistant-stream': fromRoot('node_modules/assistant-stream'),
          'assistant-stream/utils': fromRoot('node_modules/assistant-stream/dist/utils.js'),
        },
      },
      build: { chunkSizeWarningLimit: 4096 },
      optimizeDeps: {
        include: ['secure-json-parse'],
        esbuildOptions: { target: 'es2023' }
      },
    });
  },
};
export default config;
