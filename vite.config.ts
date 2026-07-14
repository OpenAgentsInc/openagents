import "vite-plus/test/config";

import { resolve } from "node:path";
import { defineConfig } from "vite-plus";

const setupFile = resolve(import.meta.dirname, "scripts/vp3-vitest-setup.ts");

/**
 * Canonical OpenAgents Vite Plus configuration.
 */
export default defineConfig({
  root: import.meta.dirname,
  pack: {
    dts: { eager: true },
    deps: {
      alwaysBundle: [/^@openagentsinc\//],
      onlyBundle: false,
      dts: { alwaysBundle: [/^@openagentsinc\//] },
    },
  },
  staged: {
    "*": "vp fmt",
  },
  fmt: {
    ignorePatterns: [
      ".pylon-local/**",
      ".worktrees/**",
      "apps/openagents-mobile/android/**",
      "apps/openagents-mobile/ios/**",
      "clients/khala-mobile/android/**",
      "clients/khala-mobile/ios/**",
      "coverage/**",
      "dist/**",
      "docs/archive/**",
      "node_modules/**",
      "pnpm-lock.yaml",
      "projects/**",
      "target/**",
      "var/**",
      "**/*.tsbuildinfo",
      "**/artifacts/**",
      "**/fixtures/**",
      "**/invalid/**",
      "**/routeTree.gen.ts",
    ],
    sortPackageJson: {},
  },
  lint: {
    ignorePatterns: [
      ".pylon-local/**",
      ".worktrees/**",
      "apps/openagents-mobile/android/**",
      "apps/openagents-mobile/ios/**",
      "clients/khala-mobile/android/**",
      "clients/khala-mobile/ios/**",
      "dist/**",
      "node_modules/**",
      "projects/**",
      "target/**",
      "var/**",
      "**/*.tsbuildinfo",
      "**/artifacts/**",
      "**/fixtures/**",
      "**/invalid/**",
      "**/routeTree.gen.ts",
    ],
    jsPlugins: ["./packages/oxlint-plugin-openagents/src/index.ts"],
    categories: {
      correctness: "warn",
      suspicious: "warn",
      perf: "warn",
    },
    rules: {
      "openagents/no-inline-schema-compile": "warn",
      "openagents/no-keyword-routing": "warn",
      // Existing suites migrate incrementally; every touched/new suite is visible now.
      "openagents/no-manual-effect-runtime-in-tests": "warn",
      "openagents/no-renderer-runtime-credentials": "error",
      "openagents/schema-contract-runtime-free": "warn",
      "openagents/subpath-only-imports": "error",
    },
    options: {
      typeAware: false,
      typeCheck: false,
    },
  },
  ssr: { noExternal: ["effect-cf"] },
  resolve: {
    alias: {
      "@opentui/core": resolve(import.meta.dirname, "scripts/vp3-opentui-test-stub.ts"),
      "cloudflare:workers": resolve(
        import.meta.dirname,
        "apps/openagents.com/workers/api/src/test/cloudflare-workers.ts",
      ),
    },
  },
  test: {
    projects: [
      {
        ssr: { noExternal: ["effect-cf"] },
        resolve: {
          alias: {
            "@opentui/core": resolve(import.meta.dirname, "scripts/vp3-opentui-test-stub.ts"),
            "cloudflare:workers": resolve(
              import.meta.dirname,
              "apps/openagents.com/workers/api/src/test/cloudflare-workers.ts",
            ),
          },
        },
        test: {
          name: "node",
          environment: "node",
          include: ["**/*.{test,spec}.{js,mjs,cjs,ts,mts,cts,jsx,tsx}", "**/*.node-suite.ts"],
          exclude: [
            "**/node_modules/**",
            "**/dist/**",
            "**/dist-electron/**",
            "**/.{git,cache,output,temp}/**",
            "projects/**",
            "apps/aiur/**",
            "apps/openagents.com/apps/start/**",
            "apps/openagents.com/workers/api/**",
            "clients/khala-mobile/**",
          ],
          hookTimeout: 240_000,
          setupFiles: [setupFile],
          testTimeout: 240_000,
        },
      },
      "./apps/aiur/vitest.config.ts",
      "./apps/openagents.com/apps/start/vitest.config.ts",
      "./apps/openagents.com/workers/api/vitest.config.ts",
      "./clients/khala-mobile/vitest.config.ts",
    ],
  },
});
