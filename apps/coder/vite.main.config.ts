import { defineConfig } from "vite";

// https://vitejs.dev/config
export default defineConfig({
  resolve: {
    mainFields: ["module", "jsnext:main", "jsnext"],
  },
  build: {
    target: 'node18',
    outDir: '.vite/build',
    minify: false,
    rollupOptions: {
      external: [
        'electron',
        'electron-squirrel-startup',
        '@react-native/assets-registry'
      ],
    },
  },
});