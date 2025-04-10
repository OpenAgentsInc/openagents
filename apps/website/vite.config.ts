import { reactRouter } from "@react-router/dev/vite";
import { cloudflare } from "@cloudflare/vite-plugin";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";

export default defineConfig({
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
            id.startsWith('rxdb/')) {
          return '\0empty-module';
        }
        return null;
      },
      load(id) {
        if (id === '\0empty-module') {
          return 'export default {}; export function useOpenAgent() { return {}; }';
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