import { reactRouter } from "@react-router/dev/vite";
import { cloudflare } from "@cloudflare/vite-plugin";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";

export default defineConfig({
  plugins: [
    cloudflare({ viteEnvironment: { name: "ssr" } }),
    tailwindcss(),
    reactRouter(),
    tsconfigPaths(),
    // Handle RxDB issues with a virtual module
    {
      name: 'rxdb-ssr-compat',
      enforce: 'pre',
      resolveId(id) {
        if (id.includes('rxdb') || id.includes('@openagents/core')) {
          return '\0empty-module';
        }
        return null;
      },
      load(id) {
        if (id === '\0empty-module') {
          return 'export default {}; export const useOpenAgent = () => ({});';
        }
        return null;
      }
    }
  ],
  optimizeDeps: {
    exclude: ['rxdb', '@openagents/core']
  },
  build: {
    rollupOptions: {
      external: ['rxdb', '@openagents/core']
    }
  }
});
