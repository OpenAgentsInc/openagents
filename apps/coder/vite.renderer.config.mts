import path from "path";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig({
  // Configure asset handling
  assetsInclude: ['**/*.ttf'],
  plugins: [
    tailwindcss(),
    react(),
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
    },
  },
  optimizeDeps: {
    exclude: ['@openagents/ui'], // Exclude UI package from optimization
    include: [
      'react-native-web',
      'react-native-vector-icons',
      'react-native-vector-icons/Ionicons',
    ],
    esbuildOptions: {
      loader: {
        '.js': 'jsx',
      },
      resolveExtensions: ['.web.js', '.js', '.ts', '.jsx', '.tsx', '.json'],
      mainFields: ['browser', 'module', 'main'],
    },
  },
  server: {
    watch: {
      usePolling: true,
      interval: 500,
    },
  },
});