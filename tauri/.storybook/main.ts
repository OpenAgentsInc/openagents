import type { StorybookConfig } from '@storybook/react-vite';
import { mergeConfig } from 'vite';
import { createRequire } from 'node:module';
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
    const req = createRequire(import.meta.url);
    return mergeConfig(baseConfig, {
      plugins: [
        {
          name: 'openagents:monorepo-bare-resolver',
          enforce: 'pre',
          resolveId(source, importer) {
            if (!importer) return null;
            // Only remap when importing from monorepo packages sources
            if (!importer.includes('/packages/')) return null;
            // Ignore relative/absolute and vite virtual ids
            if (source.startsWith('.') || source.startsWith('/') || source.startsWith('\0')) return null;
            // Keep monorepo internal packages resolved via tsconfig paths
            if (source.startsWith('@openagentsinc/')) return null;
            if (source === 'assistant-stream' || source.startsWith('assistant-stream/')) return null;
            // Map known bare deps to this app's node_modules
            const KNOWN = new Set([
              'zod',
              'zustand',
              'secure-json-parse',
              'nanoid',
              'react-textarea-autosize',
              '@radix-ui/primitive',
              '@radix-ui/react-compose-refs',
              '@radix-ui/react-context',
              '@radix-ui/react-popover',
              '@radix-ui/react-primitive',
              '@radix-ui/react-slot',
              '@radix-ui/react-use-callback-ref',
              '@radix-ui/react-use-escape-keydown',
            ]);
            // Handle subpaths like 'nanoid/non-secure' and potential nested paths
            const base = source.startsWith('@') ? source.split('/').slice(0,2).join('/') : source.split('/')[0];
            if (KNOWN.has(base)) {
              try {
                return req.resolve(source, { paths: [process.cwd()] });
              } catch {
                return null;
              }
            }
            return null;
          },
        },
        tailwind()
      ],
      resolve: {
        alias: {
          '@': fromRoot('src'),
          // Mocks to avoid Tauri/Ollama in Storybook environment
          '@tauri-apps/api/core': fromRoot('src/__mocks__/tauri-api.ts'),
          '@/runtime/adapters/ollama-adapter': fromRoot('src/__mocks__/ollama-adapter.ts'),
          '@/runtime/useAcpRuntime': fromRoot('src/__mocks__/useAcpRuntime.ts'),
          '@/vendor/assistant-ui/external-store': fromRoot('src/__mocks__/external-store.ts'),
          // Specific subpath commonly used
          'nanoid/non-secure': req.resolve('nanoid/non-secure', { paths: [process.cwd()] }),
        },
      },
      server: {
        fs: {
          allow: [fromRoot('.'), fromRoot('..'), fromRoot('../packages')],
        },
      },
      build: { chunkSizeWarningLimit: 4096 },
      optimizeDeps: {
        include: [
          'secure-json-parse',
          'nanoid',
          'nanoid/non-secure',
          'zod',
          'zustand',
          '@radix-ui/primitive',
          '@radix-ui/react-compose-refs',
          '@radix-ui/react-context',
          '@radix-ui/react-popover',
          '@radix-ui/react-primitive',
          '@radix-ui/react-slot',
          '@radix-ui/react-use-callback-ref',
          '@radix-ui/react-use-escape-keydown',
          'react-textarea-autosize',
        ],
        esbuildOptions: { target: 'es2023' },
      },
    });
  },
};
export default config;
