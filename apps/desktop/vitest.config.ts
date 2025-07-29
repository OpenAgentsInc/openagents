/// <reference types="vitest" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react({
    jsxRuntime: 'automatic'
  })],
  esbuild: {
    jsx: 'automatic',
    jsxFactory: undefined,
    jsxFragment: undefined
  },
  test: {
    globals: true,
    environment: 'jsdom', 
    setupFiles: ['./test-setup.ts'],
    css: true,
    reporters: ['verbose'],
    server: {
      deps: {
        inline: ['@testing-library/react', 'react', 'react-dom']
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
    deps: {
      inline: ['@tauri-apps/api'],
    },
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
    },
  },
})
