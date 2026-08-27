import "vite-plus/test/config";

import { resolve } from "node:path";
import { defineConfig } from "vite-plus";

import { nodeTestSuites } from "./scripts/node-test-suites.mjs";

const setupFile = resolve(import.meta.dirname, "scripts/vp3-vitest-setup.ts");

// Suites written against Node's built-in runner. Vitest cannot run them — it
// reports "No test suite found" — so they are excluded here and handed to
// `node --test` by the root `test` script instead. Both sides read the same
// discovery, so a suite can never be dropped from one without the other
// picking it up. See scripts/node-test-suites.mjs.
const nodeRunnerSuites = nodeTestSuites();

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
    // The completion gate runs integration suites with real local services,
    // production builds, and package installs. Eight forks keep those tests
    // concurrent without turning host scheduling into the thing under test.
    maxWorkers: 8,
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
            "**/.pylon-local/**",
            // `var/` is gitignored scratch. QA smokes generate real test files
            // under var/qa-pre-push-smoke/<run>/generated/, so a developer who
            // has run those smokes collects dozens of stale generated suites
            // into every later sweep and sees failures that no commit caused.
            "var/**",
            // Generated QA scenarios are live black-box probes. The QA package
            // exposes them through `test:generated`; the offline completion
            // gate must not make production availability part of unit truth.
            "apps/qa-runner/generated/**",
            "**/node_modules/**",
            "**/.claude/worktrees/**",
            "**/.worktrees/**",
            "**/dist/**",
            "**/dist-electron/**",
            "**/.{git,cache,output,temp}/**",
            "projects/**",
            "apps/openagents.com/apps/start/**",
            "apps/openagents.com/workers/api/**",
            ...nodeRunnerSuites,
          ],
          hookTimeout: 240_000,
          setupFiles: [setupFile],
          testTimeout: 240_000,
        },
      },
      "./apps/openagents.com/apps/start/vitest.config.ts",
      "./apps/openagents.com/workers/api/vitest.config.ts",
    ],
  },
});
