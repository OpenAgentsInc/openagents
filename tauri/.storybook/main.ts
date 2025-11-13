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
    const merged = mergeConfig(baseConfig, {
      plugins: [
        // Force critical aliases before any tsconfig-paths resolution
        {
          name: 'openagents:force-runtime-alias',
          enforce: 'pre',
          resolveId(source) {
            if (source === '@openagentsinc/assistant-ui-runtime' || source.startsWith('@openagentsinc/assistant-ui-runtime/')) {
              return { id: '@assistant-ui/react' };
            }
            if (source === '@openagentsinc/react-markdown' || source.startsWith('@openagentsinc/react-markdown/')) {
              return { id: '@assistant-ui/react-markdown' };
            }
            return null;
          },
        },
        tailwind(),
      ],
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
          '@openagentsinc/assistant-ui-runtime/*': '@assistant-ui/react',
          '@openagentsinc/react-markdown': '@assistant-ui/react-markdown',
          '@openagentsinc/react-markdown/styles/dot.css': '@assistant-ui/react-markdown/styles/dot.css',
          // Force assistant-stream to resolve from installed package, not monorepo sources
          'assistant-stream': fromRoot('node_modules/assistant-stream'),
          'assistant-stream/utils': fromRoot('node_modules/assistant-stream/dist/utils.js'),
          // Avoid resolving into monorepo sources if any path mapping leaks through
          [fromRoot('../packages/assistant-ui-runtime/src')]: '@assistant-ui/react',
        },
      },
      build: { chunkSizeWarningLimit: 4096 },
      optimizeDeps: {
        include: ['secure-json-parse', 'nanoid/non-secure'],
        esbuildOptions: { target: 'es2023' }
      },
    });
    // Disable tsconfig-paths plugin so our aliases win and Storybook doesn't
    // resolve to local monorepo sources outside tauri/.
    if (Array.isArray((merged as any).plugins)) {
      (merged as any).plugins = (merged as any).plugins.filter((p: any) => {
        const name = p && p.name ? String(p.name) : '';
        return !name.includes('tsconfig-paths');
      });
    }
    return merged;
  },
};
export default config;
