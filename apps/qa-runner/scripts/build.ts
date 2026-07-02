#!/usr/bin/env bun
// Build the self-contained, standalone `qa` CLI bundle (issue #6191 / Rhys req #5).
//
// WHY THIS EXISTS
// ---------------
// `@openagentsinc/qa-runner` depends on the workspace package
// `@openagentsinc/probe-runtime`, which is `private` and unpublished. A naive
// `bunx @openagentsinc/qa-runner` therefore cannot resolve that dependency
// outside the monorepo — the standalone-install acceptance for #6191 would be a
// lie. This build BUNDLES the BYO `qa` CLI (`src/byo.ts`) into a single
// self-contained `dist/qa.js` so a standalone install needs NO workspace deps
// and NO OpenAgents login.
//
// Two important properties:
//   1. The source imports narrow `@openagentsinc/probe-runtime` subpaths. The BYO
//      CLI only reaches the computer-use surface (browser tools + playwright
//      adapter); the giant barrel `index.ts` `export *`s heavy, unrelated modules
//      (terminal `@opentui/core` native binaries, backends, benchmark,
//      OpenRouter) that the BYO path never touches and that do not bundle for a
//      generic `node` target. The alias below is kept as a compatibility guard
//      for any lingering bare imports.
//   2. `playwright` is kept EXTERNAL: it is a real runtime dependency declared
//      in `package.json` (with its own postinstall browser download), so we let
//      the standalone install resolve it normally rather than inlining a
//      browser engine. `effect` is INLINED so no workspace catalog is needed.
//
// Bundling happens at the JS/module-graph level, so the pre-existing
// `packages/probe` *typecheck* errors do NOT block it — bun build does not
// typecheck.
//
// Output: `dist/qa.js` — an ESM, node-targeted, executable (shebang) bundle.

import { chmodSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

const here = dirname(new URL(import.meta.url).pathname);
const pkgRoot = resolve(here, "..");
const entry = resolve(pkgRoot, "src/byo.ts");
const outFile = resolve(pkgRoot, "dist/qa.js");

// Absolute path to probe-runtime's computer-use entry. Aliasing the bare
// specifier here avoids pulling the heavy barrel if a legacy import comes back.
const computerUseEntry = resolve(
  pkgRoot,
  "../../packages/probe/packages/runtime/src/computer-use/index.ts",
);

const aliasProbeRuntimeToComputerUse = {
  name: "alias-probe-runtime-to-computer-use",
  setup(build: {
    onResolve: (
      opts: { filter: RegExp },
      cb: (args: { path: string }) => { path: string } | undefined,
    ) => void;
  }) {
    build.onResolve({ filter: /^@openagentsinc\/probe-runtime$/ }, () => ({
      path: computerUseEntry,
    }));
  },
};

mkdirSync(dirname(outFile), { recursive: true });

const result = await Bun.build({
  entrypoints: [entry],
  outdir: dirname(outFile),
  naming: "qa.js",
  target: "node",
  format: "esm",
  // `playwright` stays external (declared runtime dep, downloads its own
  // browser). Everything else — effect + the probe-runtime computer-use
  // surface — is inlined so no workspace/catalog resolution is needed.
  external: ["playwright"],
  // @ts-expect-error — Bun's plugin type is structural; our minimal shape is fine.
  plugins: [aliasProbeRuntimeToComputerUse],
});

if (!result.success) {
  for (const log of result.logs) console.error(log);
  console.error("qa-runner build FAILED");
  process.exit(1);
}

// Normalize the shebang. `src/byo.ts` starts with `#!/usr/bin/env bun` and bun
// build emits its own `// @bun` marker; for a STANDALONE install we want a
// single, portable node shebang so `node dist/qa.js` and the `bin` symlink both
// work without bun present. Strip any leading shebang/marker lines, then prepend
// exactly one node shebang.
const built = await Bun.file(outFile).text();
const withoutLeadingMarkers = built.replace(/^(#![^\n]*\n|\/\/ @bun\n)+/, "");
await Bun.write(outFile, `#!/usr/bin/env node\n${withoutLeadingMarkers}`);

chmodSync(outFile, 0o755);

const bytes = (await Bun.file(outFile).arrayBuffer()).byteLength;
console.log(`built ${outFile} (${(bytes / 1024).toFixed(1)} KiB)`);
console.log("externals kept out of the bundle: playwright");
