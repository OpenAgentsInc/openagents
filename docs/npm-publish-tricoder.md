Tricoder (NPX) Publishing Guide

Overview
- Goal: provide a tiny NPX‑runnable CLI named `tricoder` that prints bridge instructions only.
- No Node WebSockets. Does not start the Rust bridge. Pure placeholder UX for now.
- Lives in the monorepo under `packages/tricoder` and is published to npm as `tricoder`.

Repo Layout
- Workspace root enables npm workspaces: package.json:1
- Package path: packages/tricoder
- Shared TS config: tsconfig.base.json:1

Current Package Metadata
- packages/tricoder/package.json:1
  - name: `tricoder`
  - version: `0.0.1-alpha`
  - type: `module` (ESM)
  - author: `OpenAgents, Inc.`
  - license: `Apache-2.0` (matches root LICENSE)
  - description: placeholder CLI that prints bridge instructions
  - repository: `https://github.com/OpenAgentsInc/openagents.git` (directory: `packages/tricoder`)
  - main: `dist/index.js`
  - types: `dist/index.d.ts`
  - bin: `{ "tricoder": "dist/index.js" }`
  - files: `["dist"]` (publish built output only)
  - engines: `{ node: ">=18" }`
  - scripts:
    - build: `tsc -p tsconfig.json`
    - prepublishOnly: `npx -y -p typescript tsc -p tsconfig.json && chmod +x dist/index.js`

Source Entry
- packages/tricoder/src/index.ts:1
- Behavior: prints a banner; no process references (avoids @types/node requirement).
- Shebang present (`#!/usr/bin/env node`) so the installed bin is executable.

Why the name “tricoder”
- Attempting to publish `openagents` failed due to similarity with an existing `open-agents` package (npm error 403).
- To avoid disputes and unblock NPX, we claimed the unique unscoped name `tricoder`.

Build and Publish Flow
1) Bump version (semantic, prerelease ok)
   - From `packages/tricoder`:
     - `npm version 0.0.1-alpha` (or `npm version patch` / `npm version minor`)
2) Publish
   - `npm publish --access public`
   - prepublishOnly runs automatically:
     - Builds TypeScript via an ephemeral `npx typescript` (no local dev dep needed)
     - `chmod +x dist/index.js` so the bin is executable on install
3) Verify
   - `npx tricoder@latest`
   - Expect: the placeholder banner prints; no side effects.

Local Testing (without publishing)
- Fast pack preview: `npm pack` and inspect the tarball lists contain `dist/index.js`.
- Global link for quick iteration:
  - `npm link` (from `packages/tricoder`)
  - `tricoder` (runs the linked bin)
  - `npm unlink -g tricoder` to remove when done.

Zsh auto_cd Pitfall (macOS)
- If `setopt auto_cd` is enabled, typing a directory name (e.g., `openagents`) may cd instead of running a command.
- We disabled this to avoid shadowing the `tricoder` bin:
  - `~/.zshrc: setopt auto_cd` is commented out.
  - `cdpath=($HOME/code)` remains, so `cd project` still searches `~/code`.

ESM and Typescript Notes
- ESM is used (`type: module`). If you add Node APIs later, either:
  - add `@types/node` and set `types: ["node"]` in `tsconfig.json`, or
  - keep avoiding ambient `process`/`__dirname` in TS types.
- Shebang is preserved in the compiled output; `chmod +x` ensures execution.

Files Included in npm
- Only `dist` is published (see `files`). Keep source out of the tarball.
- Root LICENSE applies; package declares `Apache-2.0`.

Troubleshooting
- “No bin file found at dist/index.js” during publish:
  - Ensure `prepublishOnly` exists and builds successfully, or run `npm run build && chmod +x dist/index.js` manually before `npm publish`.
- “command not found” after `npx tricoder`:
  - Usually indicates the tarball didn’t include `dist` or the entry isn’t executable. Re‑publish after fixing prepublish/build.
- 403 similarity error when claiming names:
  - Choose a unique name (we use `tricoder`). Scoped alternatives are possible (e.g., `@openagentsinc/tricoder`).

Future Changes (when ready)
- Replace the placeholder with a real launcher (still no Node WebSockets):
  - Print pairing steps, optionally probe local ports.
  - At a later stage, it may shell out to the Rust bridge if we decide to re‑enable that flow.
- Add metadata (homepage, bugs, keywords, funding) once the npm page is public.

