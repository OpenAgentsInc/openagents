import path from "path";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [
    tailwindcss(),
    react(),
  ],
  resolve: {
    preserveSymlinks: true,
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "react-native": "react-native-web",
      "react-native$": "react-native-web",
      "@openagents/ui": path.resolve(__dirname, "../../packages/ui/src"),
      "@openagents/ui/*": path.resolve(__dirname, "../../packages/ui/src/*"),
    },
  },
});