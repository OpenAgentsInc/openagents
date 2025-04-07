import path from "path";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import { nodePolyfills } from 'vite-plugin-node-polyfills';

export default defineConfig({
  // Configure asset handling
  assetsInclude: ['**/*.ttf', '**/*.woff', '**/*.woff2'],
  plugins: [
    tailwindcss(),
    react(),
    // Add Node.js polyfills for browser environment
    nodePolyfills({
      // Include polyfills for all required Node.js modules in browser
      include: [
        'url', 
        'util', 
        'events', 
        'stream', 
        'path', 
        'http', 
        'https', 
        'zlib', 
        'querystring',
        'buffer',
        'crypto',
        'os'
      ],
      globals: {
        Buffer: true,
        process: true,
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
      // Add shim for child_process (needed for AI/MCP modules)
      "child_process": path.resolve(__dirname, "./src/shims/child_process.ts"),
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
    assetsInlineLimit: 0, // Don't inline any assets, keep all as URLs
    minify: false, // Disable minification for easier debugging
    sourcemap: true, // Enable sourcemaps in production for debugging
    // Configure chunk naming for better error reporting
    rollupOptions: {
      plugins: [],
      // Add externals to fix the browser/node compatibility issues with MCP
      external: [
        'child_process',
        'fs',
        'path',
        'util',
        'os',
        'crypto',
        'stream',
        'events',
        'buffer',
        'querystring',
        'url',
        'http',
        'https',
        'zlib',
        // Add specific AI/MCP related modules
        'ai/mcp-stdio',
        'ai',
      ],
      output: {
        manualChunks: {
          react: ['react', 'react-dom'],
          ui: [
            '@/components/ui',
            '@openagents/ui'
          ],
          utils: ['@/utils'],
          vendors: [
            // Third-party libraries that are unlikely to change
            'uuid',
            'lucide-react',
            'tailwindcss',
          ]
        },
        chunkFileNames: 'assets/js/[name]-[hash].js',
      },
    },
  },
  server: {
    watch: {
      usePolling: true,
      interval: 500,
    },
    proxy: {
      // Proxy API requests to the Hono server running on port 3001
      '/api': {
        target: 'http://localhost:3001',
        changeOrigin: true,
        secure: false,
      },
      // Proxy agents requests directly to the production server
      '/agents': {
        target: 'https://agents.openagents.com',
        changeOrigin: true,
        secure: true,
        ws: true, // Enable WebSocket proxying
        // No need to set CORS headers - let the server handle it
      },
    },
  },
});