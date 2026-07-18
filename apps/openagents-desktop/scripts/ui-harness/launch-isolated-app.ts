/**
 * UI-automation harness (#8976 gap analysis: "No UI-click automation
 * capability exists, in this repo or in my tool access" -- the prior
 * FA-QA-01 session's exact honest blocker).
 *
 * This module launches the REAL OpenAgents Desktop renderer -- the same
 * production React/Effect-Native code path the owner's Desktop uses -- as a
 * Playwright-driven Electron process, so a script can click, type, read DOM
 * state, and screenshot it like a real user would. It deliberately reuses
 * the ALREADY-REVIEWED isolated-preview safety contract
 * (`scripts/oa-dev-preview`, `src/isolated-app-proof.ts`) instead of
 * inventing a new one:
 *
 *   - `OPENAGENTS_DESKTOP_USER_DATA` is always a fresh `mkdtemp` directory
 *     strictly beneath `os.tmpdir()` -- verified here AND independently
 *     enforced by `isIsolatedAppProof` in main.ts, which throws if preview
 *     mode is requested against any other profile.
 *   - `OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF=1` enables Chromium's mock
 *     keychain and disables the native session vault (main.ts /
 *     src/isolated-app-proof.ts). A separately gated caller may still ask
 *     the Claude Agent SDK to use its ordinary authenticated SDK session;
 *     this harness itself never probes Keychain, copies credentials, or
 *     claims provider readiness. Codex-local continues to read its normal
 *     file-backed auth independently of Electron userData/session state.
 *   - `OPENAGENTS_DESKTOP_PREVIEW=1` requires the isolated proof gate above
 *     (main.ts: "OpenAgents Desktop preview requires an isolated OS-temporary
 *     userData profile") -- it is impossible to combine preview mode with a
 *     real production userData path.
 *   - This module NEVER reads or writes `~/Library/Application Support/
 *     OpenAgents Dev` or `.../OpenAgents` (the live singleton-locked owner
 *     profile). Every launch path here is asserted at runtime to sit
 *     beneath the OS temp directory before Electron ever starts.
 *
 * Unlike `scripts/oa-dev-preview` (which spawns Electron as an opaque OS
 * process via `Runtime.spawn` and only reads stdout), this harness launches
 * through Playwright's `_electron.launch()` so the caller gets a real
 * Chromium-backed `Page` for the main renderer window -- `page.click(...)`,
 * `page.fill(...)`, `page.keyboard.press(...)`, `page.locator(...)`,
 * `page.screenshot(...)` all drive the actual rendered UI over CDP, not a
 * fixture projection.
 *
 * Reusable by design (any future Desktop UI-automation task can import
 * `launchIsolatedDesktopApp`), but deliberately NOT a full test framework:
 * no retry/assertion DSL, no fixture catalog. Callers own their own
 * waits/assertions with plain Playwright APIs.
 */
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication, type Page } from "playwright";
import type { ViteDevServer } from "vite";
import { createServer } from "vite";
import { buildDesktop } from "../build.ts";
import { desktopDevServerHost, desktopDevServerPort } from "../../vite.config.ts";

export const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

/** Throws unless `candidate` resolves to a real path strictly beneath the OS
 * temporary directory. The same shape of check `isIsolatedAppProof` runs
 * inside main.ts -- duplicated here so a harness bug fails BEFORE spawning
 * Electron, not after. Deliberately mirrors main.ts's OWN comparison
 * (`path.resolve`, no `realpathSync`): on macOS `os.tmpdir()` and Electron's
 * `app.getPath("temp")` both return the unresolved `/var/folders/...` form
 * (not the symlink-resolved `/private/var/folders/...` form), so resolving
 * symlinks here would make this stricter than -- and inconsistent with --
 * the actual runtime gate in `src/isolated-app-proof.ts`. */
export const assertStrictlyUnderTempDir = (candidate: string, label: string): void => {
  const temp = path.resolve(tmpdir());
  const resolved = path.resolve(candidate);
  const relative = path.relative(temp, resolved);
  const ok =
    relative.length > 0 &&
    relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative);
  if (!ok) {
    throw new Error(
      `[ui-harness] refusing to use ${label}=${resolved} -- it is not strictly beneath the OS temp directory ${temp}. ` +
        "This harness only ever drives an isolated OS-temp Desktop profile, never a real/default one.",
    );
  }
};

/** Throws if `candidate` looks like it could be the real owner Desktop
 * profile (belt-and-suspenders on top of assertStrictlyUnderTempDir --
 * catches a caller passing a temp path that was somehow seeded from a real
 * profile name). */
const assertNotOwnerProfileName = (candidate: string): void => {
  const lowered = candidate.toLowerCase();
  if (
    lowered.includes("openagents dev") ||
    (lowered.includes("/openagents/") && !lowered.includes("openagents-desktop"))
  ) {
    throw new Error(
      `[ui-harness] refusing userData path that resembles a real owner profile: ${candidate}`,
    );
  }
};

export type IsolatedDesktopApp = Readonly<{
  app: ElectronApplication;
  page: Page;
  userDataPath: string;
  launchCwd: string;
  close: () => Promise<void>;
}>;

export type LaunchIsolatedDesktopOptions = Readonly<{
  /** Directory Full Auto and workspace-scoped features resolve as "the
   * workspace" (`OPENAGENTS_DESKTOP_LAUNCH_CWD`). Pass a disposable scratch
   * git repo, NEVER a real product checkout -- Full Auto can genuinely write
   * files there via a live Codex-local turn. */
  launchCwd: string;
  /** Reuse an existing isolated userData directory instead of minting a
   * fresh one (e.g. a restart-continuity check that must relaunch against
   * the SAME durable profile). Still asserted strictly-under-temp-dir. */
  userDataPath?: string;
  /** Extra environment for the Electron process (merged after the harness's
   * own safety-critical vars, so a caller cannot accidentally override
   * OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF/OPENAGENTS_DESKTOP_USER_DATA). */
  extraEnv?: Record<string, string>;
  /** Skip the Rust voice-helper cargo build (irrelevant to renderer UI work
   * and slow on a cold target dir). Defaults to true. */
  skipVoiceHelperBuild?: boolean;
  /** Delete userDataPath when closing. Defaults to true; set false to keep
   * durable state around for a subsequent relaunch (restart-continuity). */
  cleanupUserDataOnClose?: boolean;
}>;

/**
 * Builds the app, starts the renderer dev server, and launches Electron
 * through Playwright against a fresh (or reused) isolated OS-temp profile.
 * Mirrors `scripts/dev.ts` + `scripts/oa-dev-preview`'s env-var contract
 * exactly.
 */
export const launchIsolatedDesktopApp = async (
  options: LaunchIsolatedDesktopOptions,
): Promise<IsolatedDesktopApp> => {
  if (!existsSync(options.launchCwd)) {
    throw new Error(`[ui-harness] launchCwd does not exist: ${options.launchCwd}`);
  }

  const userDataPath =
    options.userDataPath ?? mkdtempSync(path.join(tmpdir(), "openagents-desktop-ui-harness-"));
  assertStrictlyUnderTempDir(userDataPath, "OPENAGENTS_DESKTOP_USER_DATA");
  assertNotOwnerProfileName(userDataPath);

  if (options.skipVoiceHelperBuild !== false) process.env.OA_DESKTOP_SKIP_DEV_VOICE_HELPER = "1";
  await buildDesktop();

  const viteServer: ViteDevServer = await createServer({
    configFile: path.join(appRoot, "vite.config.ts"),
    root: appRoot,
    mode: "openagents-preview",
  });
  await viteServer.listen();
  const address = viteServer.httpServer?.address();
  const listeningPort =
    typeof address === "object" && address !== null ? address.port : desktopDevServerPort;
  const devServerUrl = `http://${desktopDevServerHost}:${listeningPort}`;

  const electronBinary = path.join(
    appRoot,
    "node_modules",
    "electron",
    "dist",
    "Electron.app",
    "Contents",
    "MacOS",
    "Electron",
  );
  if (!existsSync(electronBinary)) {
    await viteServer.close();
    throw new Error(
      `[ui-harness] Electron binary missing at ${electronBinary} -- run "node node_modules/electron/install.js"`,
    );
  }

  let app: ElectronApplication;
  try {
    app = await electron.launch({
      executablePath: electronBinary,
      args: ["."],
      cwd: appRoot,
      env: {
        ...(process.env as Record<string, string>),
        OPENAGENTS_DESKTOP_DEV_SERVER_URL: devServerUrl,
        OPENAGENTS_DESKTOP_PREVIEW: "1",
        OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
        OPENAGENTS_DESKTOP_USER_DATA: userDataPath,
        OPENAGENTS_DESKTOP_LAUNCH_CWD: options.launchCwd,
        ...(options.extraEnv ?? {}),
      },
    });
  } catch (error) {
    await viteServer.close();
    throw error;
  }

  const page = await app.firstWindow();
  await page.waitForLoadState("domcontentloaded");

  const close = async (): Promise<void> => {
    try {
      await app.close();
    } catch {
      /* already closed */
    }
    try {
      await viteServer.close();
    } catch {
      /* already closed */
    }
    if (options.cleanupUserDataOnClose !== false) {
      try {
        rmSync(userDataPath, { recursive: true, force: true });
      } catch {
        /* best-effort cleanup */
      }
    }
  };

  return { app, page, userDataPath, launchCwd: options.launchCwd, close };
};
