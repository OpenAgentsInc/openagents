/**
 * Mechanical Electron + Effect Native boundary oracle (#8574 scope 4/5).
 *
 * Source-level assertions that fail loudly if the hardened boundary or the
 * EN-only renderer discipline regresses: sandbox/contextIsolation posture in
 * the main process, a bridge-only preload with no ipcRenderer/MessagePort,
 * no Electron or Node authority inside the renderer, and no starter/parallel
 * application architecture. React/shadcn/Base UI/Tailwind are permitted only
 * as the host implementation below Effect Native's state/intent boundary.
 */
import { describe, expect, test } from "vite-plus/test";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

const appRoot = path.resolve(import.meta.dirname, "..");
const read = (relative: string): string => readFileSync(path.join(appRoot, relative), "utf8");

/**
 * The negative oracles scan CODE, not prose: doc comments legitimately name
 * the banned APIs while explaining why they are banned.
 */
const stripComments = (source: string): string =>
  source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");

describe("Electron boundary (issue #8574 mandatory first-scaffold hardening)", () => {
  const main = stripComments(read("src/main.ts"));

  test("renderer window is sandboxed with node integration off", () => {
    expect(main).toContain("contextIsolation: true");
    expect(main).toContain("nodeIntegration: false");
    expect(main).toContain("sandbox: true");
    expect(main).toContain("webviewTag: false");
    expect(main).toContain("webSecurity: true");
    expect(main).toContain("minWidth: 480");
  });

  test("the shared workbench supports the minimum window, touch disclosure, and forced colors", () => {
    const css = read("../../packages/ui/src/desktop-workbench.css");
    expect(css).toContain("min-width: 0;");
    expect(css).toContain("@media (max-width: 560px)");
    expect(css).toContain("@media (hover: none), (pointer: coarse)");
    expect(css).toContain("@media (forced-colors: active)");
    expect(css).not.toContain("min-width: 760px");
  });

  test("local thread rename rejects untrusted renderer frames before mutation", () => {
    expect(main).toMatch(
      /ipcMain\.handle\(DesktopRenameLocalThreadChannel, \(event, value: unknown\) => \{\s+if \(!isTrustedRuntimeGatewaySender\(event\)\) return/,
    );
  });

  test("uses the built mobile icon for the window and macOS Dock", () => {
    expect(main).toContain('"assets", "openagents-icon.png"');
    expect(main).toContain("icon: desktopIconPath");
    expect(main).toContain("app.dock?.setIcon(desktopIconPath)");
  });

  test("keeps production identity stable while isolating development safeStorage", () => {
    const manifest = JSON.parse(read("package.json")) as { productName?: string };
    const html = read("index.html");
    expect(main).toContain("const desktopPreviewMode = !app.isPackaged");
    expect(main).toContain('? "OpenAgents"');
    expect(main).toContain("OpenAgents Preview");
    expect(main).toContain(': "OpenAgents Dev"');
    expect(main).toContain("app.setName(desktopApplicationName)");
    expect(main).toContain("process.title = desktopApplicationName");
    expect(main).toContain("title: desktopApplicationName");
    expect(manifest.productName).toBe("OpenAgents");
    expect(html).toContain("<title>OpenAgents</title>");
    expect(html).not.toContain("OpenAgents Desktop");
  });

  test("deny-by-default permission, navigation, window-open, and webview handlers", () => {
    expect(main).toContain("setPermissionRequestHandler");
    expect(main).toContain("will-navigate");
    expect(main).toContain("will-attach-webview");
    expect(main).toContain('setWindowOpenHandler(() => ({ action: "deny" }))');
  });

  test("no template updater, publisher target, or devtools installer survives", () => {
    expect(main).not.toContain("updateElectronApp");
    expect(main).not.toContain("electron-devtools-installer");
    expect(main).not.toContain("REACT_DEVELOPER_TOOLS");
  });

  test("no legacy Khala Code identity is reused", () => {
    for (const file of ["src/main.ts", "package.json", "index.html"]) {
      const source = stripComments(read(file));
      expect(source).not.toContain("com.openagents.khala.code.desktop");
      expect(source).not.toContain("khala-code://");
    }
  });

  test("preload exposes fixed typed capabilities and one decoded runtime event stream", () => {
    const preload = stripComments(read("src/preload.cts"));
    expect(preload).toContain("contextBridge.exposeInMainWorld");
    expect(preload).toContain("ipcRenderer.invoke(FleetStageChannel, request)");
    expect(preload).toContain("DesktopWorkspaceChooseChannel");
    expect(preload).toContain("chooseWorkspace: async (): Promise<boolean>");
    expect(preload).not.toContain("workspaceSummary:");
    expect(preload).toContain("workingDirectory: async");
    expect(preload).toContain("decodeWorkspaceWorkingDirectory");
    expect(preload).not.toContain("listWorkspaceFiles:");
    expect(preload).not.toContain("readWorkspaceFile:");
    expect(preload).not.toContain("saveWorkspaceFile:");
    expect(preload).not.toContain("workspaceGitStatus:");
    expect(preload).not.toContain("workspaceGitDiff:");
    expect(preload).toContain("decodeWorkspaceTreeRequest");
    expect(preload).toContain("decodeWorkspaceTreePage(response)");
    expect(preload).toContain("decodeWorkspaceSearchBridgeRequest(value)");
    expect(preload).toContain("decodeWorkspaceSearchResponse(response)");
    expect(preload).toContain("decodeWorkspaceSearchCancelRequest(value)");
    expect(preload).toContain("decodeWorkspaceSearchCancelResult(response)");
    expect(preload).toContain("decodeWorkspaceCreateRequest(value)");
    expect(preload).toContain("decodeWorkspaceRenameRequest(value)");
    expect(preload).toContain("decodeWorkspaceDeleteRequest(value)");
    expect(preload).toContain("decodeWorkspaceRevealRequest(value)");
    expect(preload).toContain("decodeWorkspaceOperationResult(response)");
    expect(preload).toContain("decodeWorkspaceDocumentRequest(value)");
    expect(preload).toContain("decodeWorkspaceDocumentSaveRequest(value)");
    expect(preload).toContain("decodeWorkspaceDocumentSaveAsRequest(value)");
    expect(preload).toContain("decodeWorkspaceDocumentResult(response)");
    expect(preload).toContain("decodeWorkspaceChange(value)");
    expect(preload).toContain(
      "ipcRenderer.on(DesktopWorkspaceChangeChannel, workspaceChangeHandler)",
    );
    expect(preload).toContain(
      "ipcRenderer.removeListener(DesktopWorkspaceChangeChannel, workspaceChangeHandler)",
    );
    expect(preload).toContain("workspaceChangeListeners.size !== 0");
    expect(preload).toContain("decodeDesktopRuntimeGatewayRequest(value)");
    expect(preload).toContain("decodeDesktopRuntimeGatewayResponse(response)");
    expect(preload).toContain("decodeDesktopRuntimeGatewayEvent(value)");
    expect(preload).toContain("ipcRenderer.on(DesktopRuntimeGatewayEventChannel, handler)");
    expect(preload).toContain(
      "ipcRenderer.removeListener(DesktopRuntimeGatewayEventChannel, handler)",
    );
    expect(preload).not.toContain("ipcRenderer.send");
    expect(preload).not.toContain("MessagePort");
    expect(preload).not.toContain('require("node:');
  });

  test("the dev launcher skips workspace lifecycle hooks and repairs Electron explicitly", () => {
    const launcher = read("scripts/oa-dev-launch");
    const restartSupervisor = read("scripts/oa-dev-supervisor.mjs");
    expect(launcher).toContain("pnpm install --frozen-lockfile --ignore-scripts");
    expect(launcher).toContain('node "$electron_package/install.js"');
    expect(launcher).toContain('if [[ "${1:-}" == "--restart" ]]');
    expect(launcher).toContain("/bin/launchctl submit");
    expect(launcher).toContain(
      'git -C "$source_repo" show "${target_sha}:apps/openagents-desktop/scripts/oa-dev-supervisor.mjs"',
    );
    expect(launcher).toContain("a supervised restart is already active");
    expect(launcher).toContain("recorded and executable process-group ownership disagree");
    expect(launcher).toContain("multiple launcher-owned OpenAgents Dev processes exist");
    expect(main).toContain("settingsIds.includes('settings-codex')");
    expect(main).toContain("const settingsBack");
    expect(main).toContain("maxRetries: 5, retryDelay: 50");
    expect(restartSupervisor).toContain("coordinatorProcessGroupId === config.oldProcessGroupId");
    expect(restartSupervisor).toContain("process.kill(-config.oldProcessGroupId, signal)");
    expect(restartSupervisor).toContain("deps.syncLaunchWorktree()");
    expect(restartSupervisor.indexOf("if (!stopped) throw")).toBeLessThan(
      restartSupervisor.indexOf("deps.syncLaunchWorktree()"),
    );
    expect(restartSupervisor).toContain("detached: true");
    expect(restartSupervisor).toContain("schemaVersion: receiptSchema");
  });

  test("main exposes fixed validated channels rather than arbitrary command authority", () => {
    expect(main).toContain("ipcMain.handle(FleetStageChannel");
    expect(main).toContain("decodeFleetStageRequest(value)");
    expect(main).toContain("decodeWorkspaceFileRequest(value)");
    expect(main).toContain("decodeWorkspaceSaveRequest(value)");
    expect(main).toContain("decodeWorkspaceGitDiffRequest(value)");
    expect(main).toContain("decodeWorkspaceTreeRequest(value)");
    expect(main).toContain("decodeWorkspaceSearchBridgeRequest(value)");
    expect(main).toContain("decodeWorkspaceSearchCancelRequest(value)");
    expect(main).toContain(
      "workspaceSearchRegistry.start(workspaceSearchOwnerRef(event.sender.id), request)",
    );
    expect(main).toContain(
      "workspaceSearchRegistry.cancel(workspaceSearchOwnerRef(event.sender.id), request.requestRef)",
    );
    expect(main).toContain("workspaceSearchRegistry.closeOwner(searchOwnerRef)");
    expect(main).toContain("ipcMain.handle(DesktopWorkspaceCreateChannel");
    expect(main).toContain("ipcMain.handle(DesktopWorkspaceRenameChannel");
    expect(main).toContain("ipcMain.handle(DesktopWorkspaceDeleteChannel");
    expect(main).toContain("ipcMain.handle(DesktopWorkspaceRevealChannel");
    expect(main).toContain("ipcMain.handle(DesktopWorkspaceDocumentOpenChannel");
    expect(main).toContain("ipcMain.handle(DesktopWorkspaceDocumentSaveChannel");
    expect(main).toContain("ipcMain.handle(DesktopWorkspaceDocumentSaveAsChannel");
    expect(main).toContain("workspace.createEntry(request)");
    expect(main).toContain("workspace.renameEntry(request)");
    expect(main).toContain("workspace.deleteEntry(request)");
    expect(main).toContain("workspace.revealEntry(request)");
    expect(main).toContain("workspace.openDocument(request)");
    expect(main).toContain("workspace.saveDocument(request)");
    expect(main).toContain("workspace.saveDocumentAs(request)");
    expect(main).toContain("shell.showItemInFolder(absolutePath)");
    expect(main).toContain("decodeWorkspaceWatchRequest(value)");
    expect(main).toContain("isTrustedRuntimeGatewaySender(event)");
    expect(main).toContain("rebindWorkspaceChangeSubscriptions()");
    expect(main).toContain("disableWorkspaceChangeSubscription(window.id)");
    expect(main).toContain("decodeDesktopRuntimeGatewayRequest(value)");
    expect(main).toContain("isTrustedRuntimeGatewaySender(event)");
    expect(main).not.toContain("ipcMain.on(");
  });

  test("main composes and resets the bounded Runtime Gateway live registry", () => {
    expect(main).toContain("createDesktopRuntimeLiveSubscriptions");
    expect(main).toContain("await runtimeLiveSubscriptions.reset()");
    expect(main).toContain("() => runtimeLiveSubscriptions");
    expect(main).toContain("hostLifecycle.sync()?.interactions()");
    expect(main).toContain("service.decide(command)");
    expect(main).not.toContain("conversation.live.update");
  });

  test("Runtime Gateway contract cannot carry credentials, URLs, raw IPC, or process handles", () => {
    const contract = stripComments(read("src/runtime-gateway-contract.ts"));
    for (const banned of [
      "token",
      "credential",
      "ownerUserId",
      "authorUserId",
      "url",
      "MessagePort",
      "ipcRenderer",
      "processHandle",
      "argv",
    ]) {
      expect(contract.toLowerCase()).not.toContain(banned.toLowerCase());
    }
  });

  test("normal verification uses checked-in smoke history instead of ambient provider homes", () => {
    expect(main).toContain('path.join(here, "..", "tests", "fixtures")');
    // The sessionsRoot resolution itself (smoke fixture vs. real ~/.codex vs.
    // isolated-app-proof scoping, #8999) lives in the shared, unit-tested
    // `resolveCodexSessionsRoot` so main.ts, tests, and the isolated-app-proof
    // boundary share one source of truth instead of main.ts recomputing it
    // inline. main.ts still calls it with smokeMode threaded through.
    expect(main).toContain("resolveCodexSessionsRoot(");
    expect(main).toContain("smokeMode,");
    const isolatedAppProof = stripComments(read("src/isolated-app-proof.ts"));
    expect(isolatedAppProof).toContain('path.join(input.smokeFixtureRoot, "codex-smoke", "sessions")');
    expect(isolatedAppProof).toContain("input.smokeMode");
  });

  test("automated smoke stays hidden unless headed presentation is explicit", () => {
    const manifest = JSON.parse(read("package.json")) as { scripts?: Record<string, string> };
    expect(main).toContain("const hiddenAutomationMode = (");
    expect(main).toContain('process.env.OPENAGENTS_DESKTOP_HEADED !== "1"');
    expect(main).toContain("if (!hiddenAutomationMode) window.show()");
    expect(main).toContain("if (hiddenAutomationMode) return");
    expect(main).toContain("if (hiddenAutomationMode) app.dock?.hide()");
    expect(main).toContain("headless smoke unexpectedly exposed the desktop window");
    expect(manifest.scripts?.["smoke"]).not.toContain("DESKTOP_HEADED");
    expect(manifest.scripts?.["smoke:headed"]).toContain("OPENAGENTS_DESKTOP_HEADED=1");
  });

  test("Khala Sync database identity and path remain in Electron main", () => {
    const preload = stripComments(read("src/preload.cts"));
    const renderer = stripComments(read("src/renderer/boot.ts"));
    for (const source of [preload, renderer]) {
      expect(source).not.toContain("khala-sync.sqlite");
      expect(source).not.toContain("openagents-desktop.");
      expect(source).not.toContain("openKhalaSyncStore");
    }
    expect(main).toContain('"sync", "khala-sync.sqlite"');
    expect(main).toContain("hostLifecycle.replaceSync(syncHost)");
    expect(main).toContain("hostLifecycle.dispose()");
  });

  test("native session custody remains behind Electron main safeStorage", () => {
    const preload = stripComments(read("src/preload.cts"));
    const renderer = stripComments(read("src/renderer/boot.ts"));
    for (const source of [preload, renderer]) {
      expect(source).not.toContain("safeStorage");
      expect(source).not.toContain("native-session.enc");
      expect(source).not.toContain("DesktopSessionCredential");
    }
    expect(main).toContain("safeStorage");
    expect(main).toContain('"session", "native-session.enc"');
    expect(main).toContain("openDesktopSessionVaultForAccountAction");
    expect(main).not.toContain("desktopSessionVault.recover().state");
    expect(main).not.toContain("recoverVerifiedDesktopSession");
    expect(main).toContain("signInDesktopSession");
    expect(main).toContain("signOutDesktopSession");
    expect(main).toContain("openExternal: url => shell.openExternal(url)");
  });

  test("workspace filesystem authority starts only after an explicit directory choice", () => {
    expect(main).toContain("makeDesktopHostLifecycle");
    expect(main).toContain('properties: ["openDirectory", "createDirectory"]');
    expect(main).toContain("defaultPath: currentRoot");
    expect(main).toContain("const root = result.filePaths[0]");
    expect(main).toContain("selectedRoot === null ? null : workspaceSnapshot()");
    expect(main).toContain("hostLifecycle.replaceWorkspace(openSelectedWorkspace(root))");
    expect(main).toContain(
      "const openSelectedWorkspace = (root: string) => openWorkspaceService(root",
    );
    expect(main).not.toContain("let workspaceRoot");
    expect(main).not.toContain("OPENAGENTS_DESKTOP_WORKSPACE");
  });

  test("renderer CSP is restrictive (no remote script/connect surface)", () => {
    const html = read("index.html");
    expect(html).toContain("Content-Security-Policy");
    expect(html).toContain("default-src 'none'");
    expect(html).toContain("script-src 'self'");
    expect(html).toContain("font-src 'self' data:");
    expect(html).toContain("connect-src 'none'");
  });

  test("recent-chat sidebar CSS is plain text and never restores card chrome", () => {
    const css = read("src/renderer/app.css");
    expect(css).not.toContain('[data-en-key^="sidebar-thread-"][data-en-tag="Button"]');
    expect(css).toContain('[data-en-key^="sidebar-thread-"] > [data-en-role="meta"]');
    expect(css).toContain("text-overflow: ellipsis");
    expect(css).not.toMatch(
      /\[data-en-key\^="sidebar-thread-"\][^{]*\{[^}]*(?:background|border|border-radius|box-shadow)\s*:/s,
    );
  });
});

describe("Effect Native renderer boundary (no parallel UI architecture)", () => {
  const rendererDir = path.join(appRoot, "src/renderer");
  const reactHostFiles = new Set([
    "boot.ts",
    "lexical-composer-editor.tsx",
    "react-composer.tsx",
    "react-primitive-adapters.tsx",
    "react-review.tsx",
    "react-connect-surface.tsx",
    "react-full-auto-surface.tsx",
    "react-settings-surface.tsx",
    "react-sensitive-text.tsx",
    "react-timeline.tsx",
    "react-workspace-surfaces.tsx",
    "visual-baseline-workbench.tsx",
  ]);
  const rendererSources = readdirSync(rendererDir)
    .filter((name) => /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name))
    .map((name) => ({
      name,
      source: stripComments(readFileSync(path.join(rendererDir, name), "utf8")),
    }));

  test("Desktop mounts the shared React-owned Effect Native surface", () => {
    const boot = read("src/renderer/boot.ts");
    expect(boot).toContain('from "@effect-native/render-dom/react"');
    expect(boot).toContain("compatibilityRequested");
    expect(boot).toContain(
      'dataset.desktopRenderer = compatibilityRequested ? "compatibility" : "react"',
    );
    expect(boot).toContain(
      "mountReactWorkbench(root, SubscriptionRef.changes(state), report, { theme })",
    );
    expect(boot).toContain("makeReactDomRenderer({");
    expect(boot).toContain('backend: "compatibility"');
    expect(boot).not.toContain("makeDomRenderer({");
  });

  test("renderer imports only EN, scoped React host libraries, and sibling modules", () => {
    const sharedOrSibling =
      /^(@effect-native\/(core|core\/effect|render-dom(?:\/react)?|tokens)|(\.\.\/|\.\/)[a-z-]+\.(?:ts|tsx|css))$/;
    const reactHostImport =
      /^(react(?:-dom\/client)?|@base-ui\/react(?:\/[a-z-]+)?|@lexical\/react\/[A-Za-z]+|lexical|cmdk|lucide-react|#components\/ui\/[a-z-]+)$/;
    const sharedReactWorkbenchImport = "@openagentsinc/ui/desktop-workbench";
    const ownedPierreAdapterImport = "./ide/pierre-tree-adapter.tsx";
    for (const { name, source } of rendererSources) {
      const specifiers = [...source.matchAll(/from\s+"([^"]+)"/g)].map((match) => match[1]!);
      specifiers.push(...[...source.matchAll(/import\s+"([^"]+)"/g)].map((match) => match[1]!));
      for (const specifier of specifiers) {
        expect(
          sharedOrSibling.test(specifier) ||
            (reactHostFiles.has(name) &&
              (reactHostImport.test(specifier) ||
                specifier === sharedReactWorkbenchImport ||
                (name === "react-workspace-surfaces.tsx" && specifier === ownedPierreAdapterImport))),
          `${name} imports disallowed renderer dependency ${specifier}`,
        ).toBe(true);
      }
    }
  });

  test("portable Effect Native state, recipes, projections, and intents stay React-free", () => {
    for (const { name, source } of rendererSources) {
      if (reactHostFiles.has(name)) continue;
      expect(name).not.toMatch(/\.tsx$/);
      expect(source).not.toMatch(/from\s+"(?:react|react-dom|@base-ui\/react)/);
      expect(source).not.toContain("className=");
      expect(source).not.toContain("ReactNode");
    }
  });

  test("renderer never touches Electron or Node builtins", () => {
    for (const { name, source } of rendererSources) {
      expect(source).not.toContain('from "electron"');
      expect(source).not.toContain('from "node:');
      expect(source).not.toContain("process.");
    }
  });

  test("React stack is renderer-only; no parallel router, store, schema, or starter kit returns", () => {
    const manifest = JSON.parse(read("package.json")) as {
      dependencies?: Record<string, string>;
      devDependencies?: Record<string, string>;
    };
    const dependencyNames = [
      ...Object.keys(manifest.dependencies ?? {}),
      ...Object.keys(manifest.devDependencies ?? {}),
    ];
    expect(manifest.dependencies?.react).toBe("19.2.7");
    expect(manifest.dependencies?.["react-dom"]).toBe("19.2.7");
    expect(dependencyNames).toEqual(
      expect.arrayContaining([
        "react",
        "react-dom",
        "@tailwindcss/vite",
        "tailwindcss",
        "@vitejs/plugin-react",
        "vite",
      ]),
    );
    expect(manifest.dependencies?.shadcn).toBeUndefined();
    expect(manifest.devDependencies?.shadcn).toMatch(/^\^4\./);
    expect(manifest.dependencies?.["@base-ui/react"]).toMatch(/^\^1\./);
    const banned = [
      /^zod$/,
      /@orpc\//,
      /@tanstack\//,
      /radix/,
      /^zustand$/,
      /^@effect\/atom-react$/,
    ];
    for (const name of dependencyNames) {
      for (const pattern of banned) {
        expect(name).not.toMatch(pattern);
      }
    }
  });

  test("generated shadcn sources remain renderer-only components without host or domain authority", () => {
    const componentDir = path.join(appRoot, "src/components/ui");
    const allowedImport =
      /^(react|@base-ui\/react(?:\/[a-z-]+)?|@shadcn\/react\/message-scroller|class-variance-authority|cmdk|lucide-react|#lib\/utils|#components\/ui\/[a-z-]+)$/;
    for (const name of readdirSync(componentDir).filter((value) => value.endsWith(".tsx"))) {
      const source = stripComments(read(`src/components/ui/${name}`));
      const imports = [...source.matchAll(/from\s+"([^"]+)"/g)].map((match) => match[1]!);
      for (const specifier of imports)
        expect(specifier, `${name}: ${specifier}`).toMatch(allowedImport);
      expect(source).not.toContain("@effect-native");
      expect(source).not.toContain("electron");
      expect(source).not.toContain("node:");
      expect(source).not.toContain("openagentsDesktop");
    }
  });

  test("no legacy desktop app import (greenfield law: extract, not inherit)", () => {
    for (const { name, source } of rendererSources) {
      expect(source).not.toContain("khala-code-desktop");
      expect(source).not.toContain("electrobun");
    }
  });
});

describe("OpenAI Apps SDK icon catalog", () => {
  test("Effect Native DOM resolves its closed icon contract through the shared catalog", () => {
    const domRenderer = read("../openagents.com/packages/effect-native-render-dom/src/index.ts");
    const catalog = read("../openagents.com/packages/effect-native-core/src/index.ts");
    // openagents#8813 Lane A: render-dom owns its icon SVG registry
    // (./icons.ts, iconAssetSvg) instead of depending on legacy
    // `@openagentsinc/ui/icon` — EN must never depend on legacy ui.
    expect(domRenderer).toContain('from "./icons"');
    expect(domRenderer).not.toContain("@openagentsinc/ui");
    expect(domRenderer).toContain('name === "Compose" ? "ChatCompose" : name');
    expect(domRenderer).toContain("iconAssetSvg[assetName]");
    expect(catalog).toContain('"ChatCompose"');
    expect(catalog).toContain('"Agent"');
  });
});

describe("Effect Native Liquid Glass lowering", () => {
  test("desktop backdrop and glass surfaces are authored in the catalog, not CSS-only", () => {
    const shell = read("src/renderer/shell.ts");
    const domRenderer = read("../openagents.com/packages/effect-native-render-dom/src/index.ts");
    expect(shell).toContain("BackgroundGradient(");
    expect(shell).toContain('surface: "glass"');
    expect(domRenderer).toContain("mobile SwiftUI");
    expect(domRenderer).toContain("blur(28px) saturate(1.35)");
  });
});
