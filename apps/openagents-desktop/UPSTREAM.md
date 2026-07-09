# Upstream template attribution

OpenAgents Desktop was scaffolded per issue #8574 from the MIT-licensed
[`LuanRoger/electron-shadcn`](https://github.com/LuanRoger/electron-shadcn)
template.

- Reviewed baseline: upstream commit
  `a02e7bbfe0c196db22b76f40ec23b5c265d24215`
  (local read-only mirror: `~/work/projects/repos/electron-shadcn`).
- License: MIT, Copyright (c) LuanRoger (`luanroger.dev@gmail.com`). The
  template's LICENSE terms are honored for all derived structure.

## What this scaffold adopted from the template

- The main / preload / renderer process split and window bootstrap shape
  (`src/main.ts` window creation, macOS lifecycle handlers).
- The hardening posture the issue mandates on top of it: the reviewed
  baseline ships `contextIsolation: true` but also `nodeIntegration: true`
  and no explicit renderer sandbox — this scaffold's first commit sets
  `nodeIntegration: false`, `sandbox: true`, `webviewTag: false`, restrictive
  CSP, deny-by-default permission/navigation/window-open handlers, and a
  minimal `contextBridge` preload, proven by `tests/electron-boundary.test.ts`.
- The recorded Electron Forge fuse posture for the future packaging exit:
  `RunAsNode: false`, `EnableCookieEncryption: true`,
  `EnableNodeOptionsEnvironmentVariable: false`,
  `EnableNodeCliInspectArguments: false`,
  `EnableEmbeddedAsarIntegrityValidation: true`, `OnlyLoadAppFromAsar: true`
  (from the template's `forge.config.ts`).

## What was removed per issue #8574

- `updateElectronApp` call and the `@electron-forge/publisher-github` target
  pointing at `LuanRoger/electron-shadcn`.
- React DevTools installation (`electron-devtools-installer`).
- The generic MessagePort/oRPC starter bridge and raw `ipcRenderer` preload.
- shadcn/Radix/Tailwind components, React, TanStack Router/Query, Zod, i18n,
  and all starter demo UI — Effect Native replaces the application
  architecture entirely.
- `package-lock.json` — the app is a Bun workspace member of the openagents
  monorepo lockfile.

## Deferred to later #8574 exits (recorded, not silently dropped)

- Electron Forge + Vite build/packaging pipeline, makers, and the fuse plugin
  (this exit bundles with Bun and runs unpackaged via `electron .`).
- Vitest/Playwright scaffolding — replaced by the monorepo-standard
  `bun test` sweep for unit/build oracles; Playwright-driven packaged-app
  E2E arrives with the packaging/signing exit (#8574 scope 7).
- Signed/notarized builds, clean-machine first-run smoke, rollback, and the
  updates feed (scope 7), all pending the owner identity freeze (scope 1).
