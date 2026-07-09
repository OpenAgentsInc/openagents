# EN-ELECTRON: Effect Native platform host for Electron

## Outcome

Provide the Effect Native host/runtime boundary needed by the greenfield
OpenAgents Desktop app in OpenAgentsInc/openagents#8574. Electron is platform
machinery; Effect Native remains the application, component, state, and typed
intent model.

This supersedes the Electrobun destination assumed by closed Phase 4 issues #20
and #21. Their catalog and foreign-host work remains useful history, but the
Electrobun adapter is not the OpenAgents Desktop host.

## Scope

1. Define and publish the Electron host package/API (provisional package name
   `@effect-native/platform-electron`; freeze the public name before release).
2. Provide scoped Effect services for application lifecycle, windows,
   single-instance/deep links, menus, safe external links, and typed
   main/preload/renderer messaging without making Electron an orchestration
   authority.
3. Require `contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`,
   `webviewTag: false`, `webSecurity: true`, restrictive CSP, deny-by-default
   permissions/navigation/window-open behavior, allowlisted external protocols,
   IPC sender/frame-origin validation, and verified packaged fuses.
4. Expose a minimal `contextBridge` surface. Never expose raw `ipcRenderer`, a
   generic MessagePort bridge, Node/Electron built-ins, filesystem/process
   authority, provider credentials, or raw private worker events to the
   renderer.
5. Decode every request and response with Effect Schema on both sides and model
   main-process resources with scoped Effect layers.
6. Preserve typed foreign-host support for Monaco, terminal, and bounded native
   desktop facilities.
7. Ship conformance fixtures proving one component/intent program can run in a
   hardened Electron renderer and that insecure configuration or malformed IPC
   is refused.

## Boundary

The `LuanRoger/electron-shadcn` bootstrap, Forge packaging, product identity,
updates, signing, and OpenAgents-specific services belong to openagents#8574.
This upstream issue owns the reusable Effect Native Electron host contract and
conformance proof only.

## Exit

A consumer can boot an Effect Native program in a hardened Electron
main/preload/renderer topology using the published host API. Automated tests
prove the security defaults, schema-decoded IPC, renderer import boundary,
foreign-host behavior, resource cleanup, and refusal cases. Documentation marks
the earlier Electrobun Phase 4 path as historical.
