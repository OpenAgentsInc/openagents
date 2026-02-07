import { defineWorkersConfig } from "@cloudflare/vitest-pool-workers/config";
import path from "node:path";

export default defineWorkersConfig({
  resolve: {
    alias: [
      {
        find: /^ajv$/,
        replacement: path.resolve(__dirname, "./tests/ajv-shim.ts")
      }
    ]
  },
  environments: {
    ssr: {
      keepProcessEnv: true
    }
  },
  test: {
    // https://github.com/cloudflare/workers-sdk/issues/9822
    deps: {
      optimizer: {
        ssr: {
          include: ["ajv"]
        }
      }
    },
    poolOptions: {
      workers: {
        // We stub Workers AI in tests; avoid starting Wrangler remote proxy sessions
        // (flaky + can incur usage charges).
        remoteBindings: false,
        // Avoid spawning many isolated runtimes (and localhost fallback-module servers) which can
        // intermittently fail with EADDRNOTAVAIL/connection refused on some machines.
        singleWorker: true,
        wrangler: { configPath: "./wrangler.jsonc" }
      }
    }
  }
});
