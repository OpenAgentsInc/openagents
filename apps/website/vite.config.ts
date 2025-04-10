import { reactRouter } from "@react-router/dev/vite";
import { cloudflare } from "@cloudflare/vite-plugin";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";

export default defineConfig({
  server: {
    watch: {
      usePolling: true,
      interval: 500,
    },
    proxy: {
      // Proxy API requests to the Hono server running on port 3001
      // Proxy agents requests directly to the production server
      '/agents': {
        target: 'https://agents.openagents.com',
        changeOrigin: true,
        secure: true,
        ws: true, // Enable WebSocket proxying
      },
    },
  },
  plugins: [
    // Handle external modules with a pre plugin
    {
      name: 'external-modules',
      enforce: 'pre',
      resolveId(id) {
        // Handle all problematic modules
        if (id === 'react-native' ||
          id.startsWith('react-native/') ||
          id === 'react-native-markdown-display' ||
          id.startsWith('rxdb/') ||
          id === 'agents/react' ||
          id === '@openagents/core' ||
          id.includes('agents_react')) {
          return '\0empty-module:' + id;
        }
        return null;
      },
      load(id) {
        if (id.startsWith('\0empty-module:')) {
          // Provide different stubs based on the module
          const originalId = id.replace('\0empty-module:', '');

          if (originalId === '@openagents/core') {
            return `
              export default {};
              export function useOpenAgent() {
                return {
                  state: { messages: [] },
                  messages: [],
                  setMessages: () => {},
                  handleSubmit: () => {},
                  infer: async () => {},
                  setGithubToken: async () => {},
                  getGithubToken: async () => '',
                };
              }
            `;
          }

          if (originalId === 'agents/react' || originalId.includes('agents_react')) {
            return `
              export const useAgent = () => ({
                setState: () => {},
                call: () => Promise.resolve({})
              });
              export default {};
            `;
          }

          // Default empty module
          return 'export default {}; export const View = () => null; export const Text = () => null;';
        }
        return null;
      }
    },

    cloudflare({ viteEnvironment: { name: "ssr" } }),
    tailwindcss(),
    reactRouter(),
    tsconfigPaths()
  ]
});
