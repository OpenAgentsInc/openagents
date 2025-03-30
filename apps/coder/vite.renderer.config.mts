import path from "path";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import { nodePolyfills } from 'vite-plugin-node-polyfills';

export default defineConfig({
  // Configure asset handling
  assetsInclude: ['**/*.ttf'],
  plugins: [
    tailwindcss(),
    react(),
    // Add Node.js polyfills for browser environment
    nodePolyfills({
      // To add specific polyfills, specify them here
      include: ['url', 'util', 'events'],
      globals: {
        Buffer: true,
      },
    }),
  ],
  resolve: {
    preserveSymlinks: false,  // Changed to false to ensure symlinks work
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "react-native": "react-native-web",
      "react-native$": "react-native-web",
      // Remove UI package aliases - use npm linked version instead
      // "@openagents/ui": path.resolve(__dirname, "../../packages/ui/src"),
      // "@openagents/ui/*": path.resolve(__dirname, "../../packages/ui/src/*"),
      // Add aliases for Expo packages
      "@expo/vector-icons": path.resolve(__dirname, "./src/shims/expo-vector-icons.ts"),
      // Add shim for eventsource
      "eventsource": path.resolve(__dirname, "./src/shims/eventsource.ts"),
    },
  },
  optimizeDeps: {
    exclude: ['@openagents/ui'], // Exclude UI package from optimization
    include: [
      'react-native-web',
      'react-native-vector-icons',
      'react-native-vector-icons/Ionicons',
      'eventsource', // Add eventsource to be pre-bundled
    ],
    esbuildOptions: {
      loader: {
        '.js': 'jsx',
      },
      resolveExtensions: ['.web.js', '.js', '.ts', '.jsx', '.tsx', '.json'],
      mainFields: ['browser', 'module', 'main'],
    },
  },
  // Add Node.js built-in modules for browser
  build: {
    rollupOptions: {
      plugins: [],
    },
  },
  server: {
    watch: {
      usePolling: true,
      interval: 500,
    },
  },
});