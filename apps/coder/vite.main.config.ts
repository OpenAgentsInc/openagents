import { defineConfig } from "vite";

// https://vitejs.dev/config
export default defineConfig({
  resolve: {
    // Some libs that can run in both Web and Node.js, such as `axios`
    mainFields: ["module", "jsnext:main", "jsnext"],
  },
});