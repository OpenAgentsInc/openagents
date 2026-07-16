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
      ".claude/worktrees/**",
      ".worktrees/**",
      "apps/openagents-mobile/android/**",
      "apps/openagents-mobile/ios/**",
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
      "**/-changelog-data.gen.ts",
      "**/routeTree.gen.ts",
    ],
    sortPackageJson: {},
  },
  lint: {
    ignorePatterns: [
      ".pylon-local/**",
      ".claude/worktrees/**",
      ".worktrees/**",
      "apps/openagents-mobile/android/**",
      "apps/openagents-mobile/ios/**",
      "dist/**",
      "node_modules/**",
      "projects/**",
      "target/**",
      "var/**",
      "**/*.tsbuildinfo",
      "**/artifacts/**",
      "**/fixtures/**",
      "**/invalid/**",
      "**/-changelog-data.gen.ts",
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
  resolve: {
    alias: {
      "@opentui/core": resolve(import.meta.dirname, "scripts/vp3-opentui-test-stub.ts"),
    },
  },
  test: {
    projects: [
      {
        resolve: {
          alias: {
            "@opentui/core": resolve(import.meta.dirname, "scripts/vp3-opentui-test-stub.ts"),
          },
        },
        test: {
          name: "node",
          environment: "node",
          include: ["**/*.{test,spec}.{js,mjs,cjs,ts,mts,cts,jsx,tsx}", "**/*.node-suite.ts"],
          exclude: [
            "**/node_modules/**",
            "**/.claude/worktrees/**",
            "**/.worktrees/**",
            "**/dist/**",
            "**/dist-electron/**",
            "**/.{git,cache,output,temp}/**",
            "projects/**",
            "apps/aiur/**",
            "apps/openagents.com/apps/start/**",
            "apps/openagents.com/workers/api/**",
          ],
          hookTimeout: 240_000,
          setupFiles: [setupFile],
          testTimeout: 240_000,
        },
      },
      "./apps/aiur/vitest.config.ts",
      "./apps/openagents.com/apps/start/vitest.config.ts",
      "./apps/openagents.com/workers/api/vitest.config.ts",
    ],
  },
});
