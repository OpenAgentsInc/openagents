import { defineWorkersConfig } from "@cloudflare/vitest-pool-workers/config";
import path from "node:path";

export default defineWorkersConfig({
  resolve: {
    alias: [
      {
        find: /^ajv$/,
        replacement: path.resolve(__dirname, "./tests/ajv-shim.ts")
      },
      {
        // @workos/authkit-session currently ships ESM with `import ... with { type: 'json' }`,
        // which the Workers Vitest runtime cannot parse. Stub it in tests.
        find: "@workos/authkit-session",
        replacement: path.resolve(__dirname, "./tests/workos-authkit-session-shim.ts")
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
        // We stub Workers AI + external network in tests; avoid starting Wrangler remote proxy sessions.
        remoteBindings: false,
        // Avoid spawning many isolated runtimes (and localhost fallback-module servers) which can
        // intermittently fail with EADDRNOTAVAIL/connection refused on some machines.
        singleWorker: true,
        wrangler: { configPath: "./wrangler.jsonc" }
      }
    }
  }
});
