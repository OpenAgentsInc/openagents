import { defineConfig } from "vite";

import { decisionServer } from "./decision-server.ts";

export default defineConfig({
  plugins: [decisionServer()],
  server: { host: "127.0.0.1", port: 0 },
  worker: { format: "es" },
  build: {
    rollupOptions: {
      input: {
        haunt: new URL("./index.html", import.meta.url).pathname,
        medieval: new URL("./medieval/index.html", import.meta.url).pathname,
      },
    },
  },
});
