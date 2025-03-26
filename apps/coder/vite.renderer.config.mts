import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import tailwindcss from "@tailwindcss/vite";

// https://vitejs.dev/config
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
      'react-native': 'react-native-web',
      '@react-native': 'react-native-web',
      // Add shim for Expo vector icons
      '@expo/vector-icons': path.resolve(__dirname, './src/shims/expo-vector-icons.ts'),
    },
    extensions: ['.web.tsx', '.web.ts', '.web.jsx', '.web.js', '.tsx', '.ts', '.jsx', '.js']
  },
  build: {
    commonjsOptions: {
      transformMixedEsModules: true,
    },
  },
  optimizeDeps: {
    esbuildOptions: {
      loader: {
        '.js': 'jsx',
      },
      target: 'es2020',
    },
    include: [
      'react-native-web',
      'react-dom'
    ],
  },
  server: {
    port: 5173,
    strictPort: true,
  }
});