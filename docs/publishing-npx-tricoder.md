# Publishing `tricoder` for npx

This documents how we publish and test the `tricoder` NPX launcher (the desktop bridge helper) from this monorepo. It also captures the common pitfalls we hit along the way.

What users run
- NPX: `npx tricoder@latest`
- Local dev: `npm run build` then `node dist/index.js`

Package location
- Path: `packages/tricoder`
- Entry: `src/index.ts` (built to `dist/index.js`)
- Command name: `tricoder` (from `package.json -> bin`)

package.json essentials
- `name`: `tricoder`
- `version`: semver (we use pre-releases like `0.2.0-alpha.1` as needed)
- `type`: `module` (ESM)
- `bin`: `{ "tricoder": "dist/index.js" }`
- `main`/`types`: `dist/index.js` / `dist/index.d.ts`
- `files`: `["dist"]` so npm includes built artifacts
- `author`: `OpenAgents, Inc.`
- `license`: `Apache-2.0` (matches repo root)
- `repository`: `{ type: "git", url: "https://github.com/OpenAgentsInc/openagents.git", directory: "packages/tricoder" }`
- `engines`: `{ "node": ">=18" }`
- Scripts:
  - `build`: `tsc -p tsconfig.json`
  - `prepublishOnly`: builds and marks the entry executable
  - `dev`: `tsx src/index.ts` (for quick local runs; see AGENTS.md warning below)

Build and publish
1) Build first
   - `cd packages/tricoder`
   - `npm run build`
   - Sanity: `npm pack` should show `dist/index.js` and types in the tarball

2) Link locally (optional)
   - `npm link`
   - Test: `tricoder` should run from your shell

3) Publish
   - `npm version 0.2.0-alpha.1` (or similar)
   - `npm publish --access public`
   - If npm warns "No bin file found at dist/index.js", you likely forgot to build or `files: ["dist"]` is missing

4) Test via NPX
   - From another shell: `npx tricoder@latest`
   - If macOS zsh tries to `cd` into a folder named `tricoder` when you type `tricoder`, remove any shell alias/function that conflicts (see below)

Shell alias conflicts
- Some zsh configs auto-`cd` when you type a folder name. If you have a directory named `openagents` or `tricoder`, that can shadow the command.
- Fix: remove the alias/function from `~/.zshrc` (look for `nocorrect` / `cdable_vars` / functions that `cd` on unknown command). Restart the shell.

What tricoder does (dev placeholder)
- No Node WebSockets; the real bridge is Rust.
- Spawns `cargo run -p oa-tunnel -- --to bore.pub --local-port <port>` twice to expose:
  - Bridge WS: local `127.0.0.1:8787` → `ws://bore.pub:<port>/ws`
  - Convex HTTP: local `127.0.0.1:7788` → `http://bore.pub:<port>`
- Prints a base64url "Bridge Code" the app can paste to connect.
- Tails the local bridge’s broadcast feed to show `[bridge.*]` and `[codex]` events for debugging.

Do not run `npm run dev` in automation
- It’s a long‑running tunnel process. See AGENTS.md: prefer `npx tricoder` or `node dist/index.js`.

Troubleshooting
- `npx tricoder` prints nothing or errors about `codex`:
  - Codex is optional for MVP. You’ll still see the Bridge/Convex URLs. For assistant responses you need `codex` in `PATH`.
- No assistant messages appear in the mobile app:
  - Open the tricoder logs and look for `[bridge] bridge.run_submit` after sending a message.
  - If missing, ensure the app is connected to the shown `ws://bore.pub:<port>/ws` and that Settings shows a healthy Convex URL.
  - If run.submit is seen but no `[codex]` events, set `RUST_LOG=debug` and re-run the bridge to inspect stdout JSONL.

Release hygiene
- Bump versions before publishing; NPX caches per version.
- Keep `dist/index.js` executable in `prepublishOnly` (chmod +x) so direct installs work on unix.

