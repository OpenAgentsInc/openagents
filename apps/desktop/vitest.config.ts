/// <reference types="vitest" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react()], // Uses automatic JSX runtime by default
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./test-setup.ts'],
    css: true,
    reporters: ['verbose'],
    deps: {
      optimizer: {
        web: {
          // Critical for React 19 compatibility
          include: ['react', 'react-dom', 'react/jsx-runtime', '@tauri-apps/api']
        }
      }
    },
    include: [
      'src/**/*.{test,spec}.{ts,tsx}',
    ],
    exclude: [
      'node_modules/**',
      'e2e/**/*',
      '**/e2e/**/*',
      '**/*.e2e.*',
      'src-tauri/**',
      '**/playwright.config.*',
    ],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'src/test/',
        'e2e/',
        '**/*.d.ts',
        '**/*.test.{ts,tsx}',
        '**/*.spec.{ts,tsx}',
      ],
    },
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
      // Resolve jsx-runtime conflicts
      'react/jsx-runtime': 'react/jsx-runtime'
    },
  },
})
