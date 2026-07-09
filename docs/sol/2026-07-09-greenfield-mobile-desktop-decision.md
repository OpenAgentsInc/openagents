# Greenfield OpenAgents mobile and desktop decision

- Date: 2026-07-09
- Status: binding implementation direction
- Owners: product owner; Sol roadmap lane
- Issues: #8566, #8597, #8574
- Planned product promises: `openagents.mobile_app.v1`,
  `openagents.desktop_app.v1`

## Decision

OpenAgents mobile and OpenAgents Desktop are new applications. They are not
renames or progressive rewrites of the current Khala Code clients.

| Product | New root | Application model | Platform host |
| --- | --- | --- | --- |
| OpenAgents mobile | `apps/openagents-mobile` | Effect Native | React Native + Expo |
| OpenAgents Desktop | `apps/openagents-desktop` | Effect Native | Electron |

Sarah is home in both. Fleet, approvals, Blueprint, receipts, account recovery,
voice, code, terminal, and all other Khala Code product ideas are capabilities
inside the Sarah-first apps or their shared engines. There is no surviving
Khala Code product shell.

## Mobile identity is fixed

- Display/product name: `OpenAgents`.
- iOS bundle identifier: `com.openagents.app`.
- Android package/application ID: `com.openagents.app`.
- Icon source: `clients/khala-mobile/assets/images/icon.png`.
- Required copied icon SHA-256:
  `0a1865ac6d1efc792d365d9a37af9e6ffa3270fa7c8731f36129f35371bfc7ce`.

The icon must be copied into the new app and guarded by a digest test; the new
app must not read assets at runtime from a deprecated client. The legacy RN app
currently uses `com.openagents.khala.mobile`, and the Swift app uses
`com.openagents.khala`; neither is the destination identity. The repository did
not previously encode `com.openagents.app`, so its existing store ownership is
an owner-supplied authority fact that must be verified in App Store Connect and
Play Console. Build/version numbers and signing/provisioning advance from those
real records, not from either Khala app's local numbers.

The new OAuth client, URL scheme, push deep links, secure-store namespace,
database location, and owned OTA namespace must be coordinated explicitly.
Legacy `khala://`, `openagents-khala-mobile`, keychain, SQLite, or update data is
read only through a typed compatibility migration or deliberately not read at
all. No new app silently claims unknown data belonging to `com.openagents.app`.

## Desktop boundary is fixed

OpenAgents Desktop uses Electron. The old Electrobun shell, RPC transport,
packaging, updater metadata, `com.openagents.khala.code.desktop` identifier,
`khala-code://` scheme, `.khala-code` state root, release tags, and
`desktop/khala-code-desktop` feed are not destination architecture.
Legacy feed reads stay compatible for old clients and evidence, while both the
package release commands and shared updates publisher reject new Khala Code or
Autopilot desktop writes.

The new app needs an independent identity and Electron-compatible signed update
feed. Before the first packaged build, `NEEDS_OWNER.md` must freeze its macOS
bundle ID/product/executable, Windows AppUserModelId/installer identity, Linux
desktop/app ID, deep-link scheme, Electron `userData` root/session partition,
update product/feed/channel, GitHub tag namespace, and OAuth redirect/client
ownership. Do not infer the mobile identifier or reuse a Khala Code identity.

The required starting template is the MIT-licensed
[`LuanRoger/electron-shadcn`](https://github.com/LuanRoger/electron-shadcn).
The local review source is `~/work/projects/repos/electron-shadcn`; the reviewed
baseline was upstream commit
`a02e7bbfe0c196db22b76f40ec23b5c265d24215` on 2026-07-09. The scaffold records
the commit actually imported and keeps attribution in an `UPSTREAM.md` or
`NOTICE` manifest. Electron Forge, Vite,
Electron fuses, Vitest, Playwright, and the main/preload/renderer organization
are useful bootstrap material. The template's shadcn/Zod/oRPC/TanStack starter
application is not a second architecture and unused pieces are removed as
Effect Native, Effect Schema, and Effect services take ownership.
Before any launch/package, remove the template updater and publisher target for
`LuanRoger/electron-shadcn`, restrict React DevTools to development, remove its
`package-lock.json`, and integrate the scaffold with the Bun workspace lockfile.

The reviewed template is deliberately not copied unchanged: its BrowserWindow
sets `contextIsolation: true` but also `nodeIntegration: true`, does not
explicitly enable renderer sandboxing, and forwards a generic MessagePort for
oRPC. The scaffold's first security gate reverses those defaults before any
OpenAgents capability lands.

Electron's privileged boundary is deliberately narrow:

- `contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`,
  `webviewTag: false`, and `webSecurity: true` are asserted in source and the
  packaged app;
- renderer CSP is restrictive; permissions, navigation, and window-open are
  denied by default; external URLs are protocol/origin allowlisted;
- preload exposes a minimal `contextBridge` API, validates sender/frame origin,
  and never exposes raw `ipcRenderer`, generic MessagePorts, or Node/Electron
  built-ins;
- Effect Schema decodes every IPC request and response on both sides;
- filesystem, process, provider credentials, and raw private worker events stay
  outside the renderer;
- main-process resources are modeled as scoped Effect services/layers.
- packaged Electron fuses are verified, not assumed from Forge configuration.

The product renderer must mechanically boot through Effect Native and the
Electron host tracked by OpenAgentsInc/effect-native#69 (or a documented typed
interim gap). A source/dependency oracle rejects surviving shadcn/Zod/oRPC/
TanStack starter application semantics outside explicitly approved adapter code.

## Legacy freeze and extraction

These applications are deprecated now:

- `clients/khala-mobile`;
- `clients/khala-ios/Khala`;
- `clients/khala-code-desktop`.

They remain temporarily for parity evidence, contract/native-module/service
extraction, data migration, and critical security fixes. They receive no new
product features, UI work, branding changes, releases, or promises. The
greenfield apps never import their app packages or screen/shell trees.

Reusable auth, Sync, push, OTA, secure storage, harness, fleet, session,
approval, closeout, editor, terminal, and diagnostic contracts move into shared
packages behind Effect/Effect Schema boundaries. Existing enforced behavior
contracts are parity inputs; pending contracts remain pending until their real
oracles pass. A capability-disposition ledger classifies every Khala Code idea
as Sarah-home behavior, a specialist OpenAgents capability, or a shared engine
used by the Sarah-first apps. Only the superseded legacy implementation may be
retired after the successor disposition is explicit and proven.

There is no silent loss and no leftover Khala Code product surface.

## Cutover gates

The legacy apps leave active workspace/install/release/update paths only after:

1. independent greenfield scaffolds and identity/security oracles pass;
2. relevant contracts and shared services no longer reverse-import old clients;
3. Sarah conversation and a live FleetRun continue across web, mobile, and
   desktop through the same typed state and intents;
4. mobile proves clean iOS/Android builds, exact identifier/icon, self-hosted
   signed OTA, recovery, and store provisioning;
5. desktop proves a signed/notarized Electron RC, clean-machine first run,
   update, rollback, deep links, and secure IPC;
6. data migration or clean-start behavior is explicit and tested;
7. public install copy and product promises are transitioned or withdrawn with
   their integrity chain intact.

This app work can run in parallel, but it does not block the immediate Sarah
Fleet Command proof across the owner's local Codex, Claude, and Grok accounts.
The legacy promise transition and compatibility/evidence-route law is recorded
in
[`../promises/2026-07-09-khala-code-app-retirement-and-openagents-successors.md`](../promises/2026-07-09-khala-code-app-retirement-and-openagents-successors.md).
