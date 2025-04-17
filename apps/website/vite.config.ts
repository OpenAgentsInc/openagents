import { reactRouter } from "@react-router/dev/vite";
import { cloudflare } from "@cloudflare/vite-plugin";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";
import path from 'path';

export default defineConfig({
  plugins: [
    cloudflare({ viteEnvironment: { name: "ssr" } }),
    tailwindcss(),
    reactRouter(),
    tsconfigPaths(),
  ],
  // NOTE: We're not using ssr.external because the Cloudflare Vite plugin doesn't support it
  // Instead, we rely only on the alias configuration to ensure consistent React resolution
  resolve: {
    alias: {
      // Point to the root node_modules instead of the app-specific one
      'react': path.resolve(__dirname, '../../node_modules/react'),
      'react-dom': path.resolve(__dirname, '../../node_modules/react-dom'),
      // FORCE resolution to the .edge version for both generic and specific imports
      'react-dom/server': path.resolve(__dirname, '../../node_modules/react-dom/server.edge.js'),
      'react-dom/server.edge': path.resolve(__dirname, '../../node_modules/react-dom/server.edge.js'),
      // Ensure agents/react resolves consistently
      'agents/react': path.resolve(__dirname, '../../node_modules/agents/dist/react.js'),
    },
    // This helps ensure proper deduplication of packages
    dedupe: ['react', 'react-dom', 'agents', 'agents/react'],
  },
});
