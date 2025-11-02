import { defineConfig } from "vite";
import path from 'path'
import { fileURLToPath } from 'url'
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

export default defineConfig(async () => ({
  plugins: [react()],
  resolve: {
    alias: {
      // Monorepo alias so CSS import resolves: '@openagentsinc/theme/web/theme.css'
      '@openagentsinc/theme': path.resolve(__dirname, '..', 'packages', 'openagents-theme'),
      // Tinyvex client + tricoder types
      'tinyvex': path.resolve(__dirname, '..', 'packages', 'tinyvex', 'src'),
      'tricoder': path.resolve(__dirname, '..', 'packages', 'tricoder', 'src'),
      // Expo component reuse shims and alias
      'react-native': 'react-native-web',
      '@expo/vector-icons': path.resolve(__dirname, 'src', 'shims', 'expo-vector-icons.tsx'),
      '@/constants/theme': path.resolve(__dirname, 'src', 'shims', 'expo-theme.ts'),
      '@/constants/typography': path.resolve(__dirname, 'src', 'shims', 'expo-typography.ts'),
      '@/providers/tinyvex': path.resolve(__dirname, 'src', 'shims', 'expo-tinyvex-provider.ts'),
      'expo-router': path.resolve(__dirname, 'src', 'shims', 'expo-router.ts'),
      // Allow importing Expo component files directly
      'expo': path.resolve(__dirname, '..', 'expo'),
    },
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
