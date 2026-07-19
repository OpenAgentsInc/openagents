import { describe, expect, test } from "vite-plus/test";
import { readFileSync } from "node:fs";
import path from "node:path";

import config, {
  OPENAGENTS_DESKTOP_BUNDLE_ID,
  OPENAGENTS_DESKTOP_PROTOCOL,
  canonicalArtifactPath,
} from "../forge.config.ts";
import { desktopReleaseArtifactName } from "../scripts/release-artifact-name.ts";
import { macOSCodeDocumentExtensions, macOSCodeDocumentTypes } from "../src/macos-document-open.ts";

const root = path.resolve(import.meta.dirname, "..");
const mainSource = readFileSync(path.join(root, "src", "main.ts"), "utf8");

describe("CUT-26 macOS artifact contract", () => {
  test("names generated release artifacts product-version-platform-architecture", () => {
    expect(
      desktopReleaseArtifactName({
        product: "OpenAgents",
        version: "0.1.2",
        platform: "darwin",
        arch: "arm64",
        extension: ".dmg",
      }),
    ).toBe("OpenAgents-0.1.2-arm64.dmg");
    expect(
      desktopReleaseArtifactName({
        product: "OpenAgents",
        version: "0.1.2-rc.3",
        platform: "darwin",
        arch: "arm64",
        extension: "zip",
      }),
    ).toBe("OpenAgents-0.1.2-rc.3-darwin-arm64.zip");
    expect(() =>
      desktopReleaseArtifactName({
        product: "OpenAgents",
        version: "../0.1.2",
        platform: "darwin",
        arch: "arm64",
        extension: ".dmg",
      }),
    ).toThrow("Invalid release artifact version");
    expect(
      canonicalArtifactPath(
        "/tmp/forge-name-derived-from-live-checkout.dmg",
        "darwin",
        "arm64",
        "9.8.7-rc.4",
      ),
    ).toBe("/tmp/OpenAgents-9.8.7-rc.4-arm64.dmg");
  });

  test("normal production launches use the canonical OpenAgents profile with a legacy atomic migration", () => {
    expect(mainSource).toContain('path.join(app.getPath("appData"), "OpenAgents")');
    expect(mainSource).toContain(
      "renameSync(legacyDevelopmentUserDataPath, productionUserDataPath)",
    );
    expect(mainSource).not.toContain(': path.join(app.getPath("appData"), "OpenAgentsDesktopDev")');
  });
  test("freezes independent identity, DMG+ZIP outputs, ASAR, and external Codex ownership", () => {
    expect(OPENAGENTS_DESKTOP_BUNDLE_ID).toBe("com.openagents.desktop");
    expect(OPENAGENTS_DESKTOP_BUNDLE_ID).not.toContain("khala");
    expect(OPENAGENTS_DESKTOP_PROTOCOL).toBe("openagents");
    const asar = config.packagerConfig?.asar as { unpack?: string; unpackDir?: string };
    expect(asar.unpack).toContain("claude-agent-sdk");
    expect(asar.unpack).not.toContain("@openai/codex");
    expect(asar.unpackDir).toBe("dist/{renderer,workers}");
    const ignore = config.packagerConfig?.ignore;
    expect(typeof ignore).toBe("function");
    expect(typeof ignore === "function" && ignore("/node_modules/effect/index.js")).toBe(true);
    const makers = (config.makers ?? []) as ReadonlyArray<{
      name: string;
      platforms: ReadonlyArray<string>;
    }>;
    const darwinMakers = makers.filter((maker) => maker.platforms.includes("darwin"));
    expect(darwinMakers).toHaveLength(2);
    expect(darwinMakers.map((maker) => maker.name).toSorted()).toEqual(["dmg", "zip"]);
    const linuxMakers = makers.filter((maker) => maker.platforms.includes("linux"));
    expect(linuxMakers.map((maker) => maker.name).toSorted()).toEqual([
      "appimage",
      "deb",
      "rpm",
    ]);
    const appImageMaker = readFileSync(
      path.join(root, "scripts", "maker-appimage.ts"),
      "utf8",
    );
    expect(appImageMaker).toContain("extraMetadata: { desktopName: this.config.appId }");
    expect(appImageMaker).toContain("syncDesktopName: true");
    expect(appImageMaker).toContain("icon: this.config.icon");
    expect(appImageMaker).toContain("MimeType:");
    const linuxDesktop = readFileSync(
      path.join(root, "resources", "linux-desktop.ejs"),
      "utf8",
    );
    expect(linuxDesktop).toContain("Exec=<%= binaryName %> %U");
    expect(linuxDesktop).toContain("StartupWMClass=<%= startupWmClass %>");
    expect(linuxDesktop).toContain("MimeType=<%= mimeType.join(';') %>;");
    const installerPatch = readFileSync(
      path.join(root, "..", "..", "patches", "electron-installer-common@0.10.4.patch"),
      "utf8",
    );
    expect(installerPatch).toContain("this.options.desktopFileName || this.appIdentifier");
    expect(installerPatch).toContain("this.options.binaryName || this.appIdentifier");
    const manifest = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8")) as {
      scripts: Record<string, string>;
    };
    expect(manifest.scripts["make:mac"]).toBe(
      "node --import tsx scripts/prepare-macos-maker.ts && node --import tsx scripts/stage-and-package.ts --target darwin-arm64 --mode make",
    );
    const nativePreparation = readFileSync(
      path.join(root, "scripts", "prepare-macos-maker.ts"),
      "utf8",
    );
    expect(nativePreparation).toContain('const nativePackages = ["macos-alias", "fs-xattr"]');
    expect(nativePreparation).toContain('Runtime.spawnSync([nodeGyp, "rebuild"]');
    expect(nativePreparation).toContain("process.versions.modules");
    expect(nativePreparation).toContain('npm_config_loglevel: "error"');
    const workspace = readFileSync(path.join(root, "..", "..", "pnpm-workspace.yaml"), "utf8");
    expect(workspace).toContain("macos-alias: true");
    expect(workspace).toContain("fs-xattr: true");
  });

  test("packages the product-owned macOS icon instead of Electron's fallback", () => {
    expect(config.packagerConfig?.icon).toBe("resources/openagents-icon.icns");
    const icon = readFileSync(path.join(root, "resources", "openagents-icon.icns"));
    expect(icon.subarray(0, 4).toString("ascii")).toBe("icns");
    expect(icon.byteLength).toBeGreaterThan(1_000_000);
  });

  test("registers the packaged app as an alternate editor for code documents", () => {
    expect(config.packagerConfig?.extendInfo).toMatchObject({
      CFBundleDocumentTypes: macOSCodeDocumentTypes,
      LSSupportsOpeningDocumentsInPlace: true,
    });
    expect(macOSCodeDocumentExtensions).toEqual(
      expect.arrayContaining(["md", "js", "jsx", "ts", "tsx"]),
    );
    expect(macOSCodeDocumentTypes.every(type =>
      type.CFBundleTypeRole === "Editor" && type.LSHandlerRank === "Alternate",
    )).toBe(true);
  });

  test("captures macOS open-file delivery before ready and dispatches only a relative path", () => {
    const openFileListener = mainSource.indexOf('app.on("open-file"');
    const readyHandler = mainSource.indexOf("void app.whenReady().then");
    expect(openFileListener).toBeGreaterThan(0);
    expect(openFileListener).toBeLessThan(readyHandler);
    expect(mainSource).toContain("event.preventDefault()");
    expect(mainSource).toContain("resolveMacOSDocumentOpenTarget(selectedPath");
    expect(mainSource).toContain('{ kind: "path", pathRef: target.pathRef }');
    expect(mainSource).toContain('"open_file"');
    expect(mainSource).toContain("desktopDocumentOpenRendererArgument(launchDocumentTarget.pathRef)");
    expect(mainSource).toContain("additionalArguments:");
  });

  test("integrates the macOS traffic lights into the blue application chrome", () => {
    expect(mainSource).toContain('titleBarStyle: "hiddenInset"');
    expect(mainSource).toContain("trafficLightPosition: { x: 12, y: 12 }");
    expect(mainSource).not.toContain('titleBarStyle: "default"');
  });

  test("locks the hardened Electron fuse posture", () => {
    const source = readFileSync(path.join(root, "forge.config.ts"), "utf8");
    for (const expected of [
      "RunAsNode]: false",
      "EnableCookieEncryption]: true",
      "EnableNodeOptionsEnvironmentVariable]: false",
      "EnableNodeCliInspectArguments]: false",
      "EnableEmbeddedAsarIntegrityValidation]: true",
      "OnlyLoadAppFromAsar]: true",
      "LoadBrowserProcessSpecificV8Snapshot]: false",
      "GrantFileProtocolExtraPrivileges]: false",
      "strictlyRequireAllFuses: true",
    ])
      expect(source).toContain(expected);
    const main = readFileSync(path.join(root, "src", "main.ts"), "utf8");
    expect(main).toContain(
      'path.join(process.resourcesPath, "app.asar.unpacked", "dist", "renderer")',
    );
    expect(main).toContain("protocol.registerSchemesAsPrivileged");
    expect(main).toContain("protocol.handle(DesktopRendererScheme");
    expect(main).toContain("window.loadURL(desktopRendererEntry)");
    expect(main).toContain("app.commandLine.appendSwitch(chromiumSwitch)");
    expect(main).not.toContain("partition: `openagents-isolated-proof-");
    expect(main).toContain("primaryDesktopWindow = window");
    expect(main).toContain("isTrustedDesktopRendererUrl({");
    expect(main).toContain('["index.html", "text/html; charset=utf-8"]');
    expect(main).toContain("cpSync(smokeFixtureSourceRoot, smokeFixtureRoot, { recursive: true })");
    const build = readFileSync(path.join(root, "scripts", "build.ts"), "utf8");
    expect(build).toContain('"smoke-fixtures"');
  });

  test("packages the fixed architecture voice helper as an executable signed resource", () => {
    expect(config.packagerConfig?.extraResource).toContain("dist/native");
    expect(config.packagerConfig?.extraResource).toContain("dist/builtin-skills");
    const source = readFileSync(path.join(root, "forge.config.ts"), "utf8");
    expect(source).toContain('"oa-desktop-audio"');
    expect(source).toContain('"claude"');
    // Explicit-triple native builds and provider staging live in the DIST-03
    // staging builder; Forge only consumes the staged workspace.
    expect(source).toContain("requireStagedBuildInputs");
    expect(source).not.toContain("const architecture = process.arch");
    const staging = readFileSync(path.join(root, "scripts", "stage-target.ts"), "utf8");
    expect(staging).toContain("`@anthropic-ai/claude-agent-sdk-${platform}-${arch}`");
    expect(staging).toContain('"--target",');
    expect(staging).toContain("rustTargetTriple");
    expect(staging).toContain("chmod(destination, 0o755)");
    expect(staging).toContain("manifest.json");
    expect(staging).toContain('"openagents-icon.icns"');
    expect(staging).toContain('"openagents-icon.png"');
    expect(staging).toContain('"linux-desktop.ejs"');
    expect(config.packagerConfig?.extendInfo).toMatchObject({
      NSMicrophoneUsageDescription: expect.any(String),
    });
  });

  test("packages the product-owned ProductSpec and AssuranceSpec compatibility assets", () => {
    const resources = config.packagerConfig?.extraResource;
    expect(resources).toContain("dist/builtin-skills");
    const manifest = JSON.parse(
      readFileSync(path.join(root, "resources", "builtin-skills", "manifest.json"), "utf8"),
    ) as { skills: Array<{ name: string; authority: string; ambientFallback: boolean }> };
    expect(manifest.skills).toContainEqual(
      expect.objectContaining({
        name: "productspec-work",
        authority: "proposal_only",
        ambientFallback: false,
      }),
    );
    expect(manifest.skills).toContainEqual(
      expect.objectContaining({
        name: "assurancespec-work",
        authority: "proposal_only",
        ambientFallback: false,
      }),
    );
  });

  test("entitlements stay minimal and never disable library validation or permit debugging", () => {
    for (const name of ["entitlements.mac.plist", "entitlements.mac.inherit.plist"]) {
      const plist = readFileSync(path.join(root, "build", name), "utf8");
      expect(plist).toContain("allow-jit");
      expect(plist).toContain("allow-unsigned-executable-memory");
      expect(plist).not.toContain("disable-library-validation");
      expect(plist).not.toContain("get-task-allow");
      expect(plist).not.toContain("allow-dyld-environment-variables");
    }
  });

  test("notarization is API-key-only and never embeds credential material", () => {
    const source = readFileSync(path.join(root, "forge.config.ts"), "utf8");
    expect(source).toContain("ASC_API_PRIVATE_KEY_PATH");
    expect(source).toContain("ASC_API_KEY_ID");
    expect(source).toContain("ASC_API_ISSUER_ID");
    expect(source).toContain('"code-sign"');
    expect(source).toContain('"signing-identity"');
    expect(source).toContain('"/Electron Framework.framework/Versions/Current/"');
    expect(source).toContain("isMacCodeSignablePath");
    expect(source).toContain('"chrome_crashpad_handler"');
    expect(source).toContain('"ShipIt"');
    expect(source).not.toContain("appleIdPassword");
    expect(source).not.toContain("@openagents.com");
  });

  test("the signed artifact contains no Codex package or nested Codex signer target", () => {
    const source = readFileSync(path.join(root, "forge.config.ts"), "utf8");
    const manifest = readFileSync(path.join(root, "package.json"), "utf8");
    expect(source).not.toContain('"codex",');
    expect(source).not.toContain("verifyPackagedCodexRuntime");
    expect(manifest).not.toContain('"@openai/codex"');
  });
});
