import { defineConfig } from "vite"
import tailwindcss from "@tailwindcss/vite"
import path from "path"
import { fileURLToPath } from "url"

const __dirname = path.dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  plugins: [tailwindcss()],
  root: __dirname,
  base: "/",
  publicDir: "static", // Use a different public dir to avoid conflict
  
  // Entry points for client code
  build: {
    outDir: "public",
    emptyOutDir: false, // Don't clear public dir as it has other assets
    rollupOptions: {
      input: {
        // Main client entry
        client: path.resolve(__dirname, "src/client/index.ts"),
        // Chat functionality
        chat: path.resolve(__dirname, "src/client/chat.ts"),
        // Model selector
        "model-selector": path.resolve(__dirname, "src/client/model-selector.ts")
      },
      output: {
        entryFileNames: "js/[name].js",
        chunkFileNames: "js/[name]-[hash].js",
        assetFileNames: (assetInfo) => {
          if (assetInfo.name?.endsWith('.css')) {
            return 'css/[name][extname]'
          }
          return 'assets/[name]-[hash][extname]'
        },
        format: "es",
        exports: "named"
      }
    },
    target: "es2020",
    sourcemap: true,
    minify: process.env.NODE_ENV === "production"
  },
  
  // Development server configuration
  server: {
    port: 5173,
    strictPort: true,
    
    // Proxy API and chat routes to Bun server
    proxy: {
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true
      },
      "/chat": {
        target: "http://localhost:3000", 
        changeOrigin: true
      },
      // Proxy WebSocket connections for hot reload
      "/ws": {
        target: "ws://localhost:3000",
        ws: true
      }
    }
  },
  
  // Resolve configuration for workspace packages
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@openagentsinc/psionic": path.resolve(__dirname, "../../packages/psionic/src"),
      "@openagentsinc/sdk": path.resolve(__dirname, "../../packages/sdk/src"),
      "@openagentsinc/nostr": path.resolve(__dirname, "../../packages/nostr/src"),
      "@openagentsinc/ui": path.resolve(__dirname, "../../packages/ui/src"),
      "@openagentsinc/ai": path.resolve(__dirname, "../../packages/ai/src")
    }
  },
  
  // Optimize dependencies
  optimizeDeps: {
    include: [
      "effect",
      "@effect/platform",
      "@noble/curves",
      "@noble/hashes"
    ],
    exclude: [
      "@openagentsinc/psionic",
      "@openagentsinc/sdk", 
      "@openagentsinc/nostr",
      "@openagentsinc/ui",
      "@openagentsinc/ai"
    ]
  },
  
  // CSS configuration
  css: {
    postcss: {
      plugins: []
    }
  }
})