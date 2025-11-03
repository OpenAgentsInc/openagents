import { defineConfig } from "vite";
import path from 'path'
import { fileURLToPath } from 'url'
import react from "@vitejs/plugin-react";
import tailwindcss from '@tailwindcss/vite'

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      // Ensure Zustand middleware is ESM so Vite transforms import.meta.env
      'zustand/middleware': path.resolve(__dirname, 'node_modules', 'zustand', 'esm', 'middleware.mjs'),
      react: path.resolve(__dirname, 'node_modules', 'react'),
      'react-dom': path.resolve(__dirname, 'node_modules', 'react-dom'),

      // Monorepo alias so CSS import resolves: '@openagentsinc/theme/web/theme.css'
      '@openagentsinc/theme': path.resolve(__dirname, '..', 'packages', 'openagents-theme'),
      '@openagentsinc/core': path.resolve(__dirname, '..', 'packages', 'openagents-core', 'src'),
      // Tinyvex client + tricoder types
      'tinyvex': path.resolve(__dirname, '..', 'packages', 'tinyvex', 'src'),
      'tricoder': path.resolve(__dirname, '..', 'packages', 'tricoder', 'src'),
      // Expo component reuse shims and alias
      'react-native': path.resolve(__dirname, 'src', 'shims', 'rn-web-runtime.tsx'),
      '@expo/vector-icons': path.resolve(__dirname, 'src', 'shims', 'expo-vector-icons.tsx'),
      '@react-native-async-storage/async-storage': path.resolve(__dirname, 'src', 'shims', 'async-storage.ts'),
      '@/constants/theme': path.resolve(__dirname, 'src', 'shims', 'expo-theme.ts'),
      '@/constants/typography': path.resolve(__dirname, 'src', 'shims', 'expo-typography.ts'),
      '@/providers/tinyvex': path.resolve(__dirname, 'src', 'shims', 'expo-tinyvex-provider.ts'),
      'expo-router': path.resolve(__dirname, 'src', 'shims', 'expo-router.ts'),
      // Allow importing Expo component files directly
      'expo': path.resolve(__dirname, '..', 'expo'),
      '@': path.resolve(__dirname, 'src'),
    },
    dedupe: ['react', 'react-dom', 'zustand'],
  },
  optimizeDeps: {
    include: ['zustand', 'zustand/middleware'],
    force: true,
    esbuildOptions: {
      define: {
        'import.meta.env.MODE': JSON.stringify(process.env.NODE_ENV || 'development'),
      },
    },
  },
  build: {
    commonjsOptions: { transformMixedEsModules: true },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
