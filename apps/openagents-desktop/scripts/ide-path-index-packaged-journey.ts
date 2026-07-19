import { execFileSync, spawn } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { chromium, type Browser } from "playwright";
import { Schema } from "effect";

import { IdePathIndexPackagedJourneyReceiptSchema } from "../src/ide/index-benchmark-contract.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const packagedOutputRoot = path.join(appRoot, "out");
const packagedDirectory = readdirSync(packagedOutputRoot, { withFileTypes: true })
  .find(entry => entry.isDirectory() && entry.name.endsWith("-darwin-arm64"));
const packagedApp = packagedDirectory === undefined
  ? undefined
  : readdirSync(path.join(packagedOutputRoot, packagedDirectory.name), { withFileTypes: true })
      .find(entry => entry.isDirectory() && entry.name.endsWith(".app"));
const packagedMacOsDirectory = packagedDirectory === undefined || packagedApp === undefined
  ? undefined
  : path.join(packagedOutputRoot, packagedDirectory.name, packagedApp.name, "Contents", "MacOS");
const packagedExecutable = packagedMacOsDirectory === undefined
  ? undefined
  : readdirSync(packagedMacOsDirectory, { withFileTypes: true }).find(entry => entry.isFile());
const packagedBinary = packagedMacOsDirectory === undefined || packagedExecutable === undefined
  ? null
  : path.join(packagedMacOsDirectory, packagedExecutable.name);
const screenshotRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-02-packaged-explorer.png";
const screenshotPath = path.join(repositoryRoot, screenshotRef);
const receiptPath = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-19-ide-02-packaged-journey.json",
);

const workspaceRoot = process.argv.slice(2).find(argument => path.isAbsolute(argument));
if (workspaceRoot === undefined || !path.isAbsolute(workspaceRoot) || !existsSync(workspaceRoot)) {
  throw new Error("usage: pnpm run ide:path-index-packaged-journey -- <absolute disposable tracked-source archive>");
}
const relativeToTemp = path.relative(path.resolve(tmpdir()), path.resolve(workspaceRoot));
if (relativeToTemp === "" || relativeToTemp === ".." || relativeToTemp.startsWith(`..${path.sep}`) || path.isAbsolute(relativeToTemp)) {
  throw new Error("IDE-02 packaged journey accepts only a disposable corpus beneath the OS temporary directory");
}
if (packagedBinary === null || !existsSync(packagedBinary)) {
  throw new Error(`packaged Desktop binary missing beneath ${packagedOutputRoot}`);
}

const countEntries = (root: string): number => {
  const queue = [root];
  let count = 0;
  while (queue.length > 0) {
    const directory = queue.pop()!;
    for (const item of readdirSync(directory, { withFileTypes: true })) {
      if (item.isSymbolicLink()) continue;
      count += 1;
      if (item.isDirectory()) queue.push(path.join(directory, item.name));
    }
  }
  return count;
};

const main = async (): Promise<void> => {
  const sourceEntries = countEntries(workspaceRoot);
  if (sourceEntries < 5_000) throw new Error(`large-repository corpus too small: ${sourceEntries}`);
  const userDataPath = mkdtempSync(path.join(tmpdir(), "openagents-ide02-packaged-"));
  const appProcess = spawn(packagedBinary, ["--remote-debugging-port=0"], {
    cwd: workspaceRoot,
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_USER_DATA: userDataPath,
      OPENAGENTS_DESKTOP_LAUNCH_CWD: workspaceRoot,
      OA_DESKTOP_SKIP_DEV_VOICE_HELPER: "1",
    },
    stdio: "ignore",
  });
  let cdpBrowser: Browser | null = null;
  try {
    const devToolsPortPath = path.join(userDataPath, "DevToolsActivePort");
    const devToolsDeadline = Date.now() + 20_000;
    while (!existsSync(devToolsPortPath) && Date.now() < devToolsDeadline) {
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    if (!existsSync(devToolsPortPath)) throw new Error("packaged Chromium DevTools port did not appear");
    const port = readFileSync(devToolsPortPath, "utf8").split("\n")[0];
    cdpBrowser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    const page = cdpBrowser.contexts().flatMap(context => context.pages())[0];
    if (page === undefined) throw new Error("packaged renderer page did not appear");
    await page.waitForSelector('aside[aria-label="Sessions"]', {
      state: "attached",
      timeout: 60_000,
    });
    process.stdout.write("[openagents-desktop] packaged shell attached\n");
    await page.keyboard.press(process.platform === "darwin" ? "Meta+E" : "Control+E");
    await page.waitForTimeout(500);
    if (await page.locator('[data-react-workspace="files"]').count() === 0) {
      await page.getByRole("button", { name: "Files", exact: true }).click();
    }
    const tree = page.locator('[data-oa-pierre-tree="true"]');
    await tree.waitFor({ state: "visible", timeout: 60_000 });
    process.stdout.write("[openagents-desktop] packaged Pierre tree visible\n");
    const status = page.locator("#oa-workspace-index-status");
    await status.waitFor({ state: "visible", timeout: 60_000 });
    await page.waitForFunction(() => /indexed paths ready\./u.test(
      document.querySelector("#oa-workspace-index-status")?.textContent ?? "",
    ), undefined, { timeout: 60_000 });
    const statusText = (await status.textContent()) ?? "";
    const indexedNodes = Number.parseInt(statusText.match(/^(\d+) indexed paths ready\.$/u)?.[1] ?? "0", 10);
    if (indexedNodes < 5_000) throw new Error(`packaged index was incomplete: ${statusText}`);

    const firstRow = tree.locator('[data-item-path]:not([data-file-tree-sticky-row="true"])').first();
    await firstRow.focus();
    await page.keyboard.press("End");
    const endPath = await tree.evaluate((host) => host.shadowRoot?.activeElement?.getAttribute("data-item-path") ?? null);
    await page.keyboard.press("Home");
    const homePath = await tree.evaluate((host) => host.shadowRoot?.activeElement?.getAttribute("data-item-path") ?? null);
    const keyboardHomeEnd = homePath !== null && endPath !== null && homePath !== endPath;

    await page.keyboard.press("Shift+F10");
    const menu = page.locator('[role="menu"][aria-label^="Actions for "]');
    await menu.waitFor({ state: "visible", timeout: 10_000 });
    const keyboardContextMenu = await menu.locator('[role="menuitem"]').count() >= 7;
    await page.keyboard.press("Escape");

    const fileRow = tree.locator('[data-item-type="file"]').first();
    const pointerPath = await fileRow.getAttribute("data-item-path");
    await fileRow.click();
    if (pointerPath !== null) {
      await page.waitForFunction((expectedPath) => [...document.querySelectorAll('[aria-label^="Editor for "]')]
        .some((element) => element.getAttribute("aria-label") === `Editor for ${expectedPath}`), pointerPath, {
        timeout: 10_000,
      });
    }
    const pointerActivation = pointerPath !== null && await page.locator('[aria-label^="Editor for "]').evaluateAll(
      (elements, expectedPath) => elements.some((element) =>
        element.getAttribute("aria-label") === `Editor for ${expectedPath}`),
      pointerPath,
    );
    const screenReaderTree = await tree.getAttribute("aria-label") === "Workspace files" &&
      await tree.getAttribute("aria-describedby") === "oa-workspace-index-status";
    const rootWithheld = !(await tree.evaluate((host, root) =>
      (host.shadowRoot?.textContent ?? "").includes(root as string), workspaceRoot));

    await page.screenshot({ path: screenshotPath, fullPage: true });
    const receipt = Schema.decodeUnknownSync(IdePathIndexPackagedJourneyReceiptSchema)({
      schemaVersion: "openagents.desktop.ide-path-index-packaged-journey.v1",
      capturedAt: new Date().toISOString(),
      commitSha: execFileSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" }).trim(),
      platform: process.platform,
      architecture: process.arch,
      packaged: true,
      sourceCorpus: "tracked-openagents-archive",
      sourceEntries,
      indexedNodes,
      indexState: "ready",
      pointerActivation,
      keyboardHomeEnd,
      keyboardContextMenu,
      screenReaderTree,
      rootWithheld,
      screenshotRef,
    });
    writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 });
    if (!pointerActivation || !keyboardHomeEnd || !keyboardContextMenu || !screenReaderTree || !rootWithheld) {
      throw new Error(`packaged IDE-02 journey failed: ${JSON.stringify(receipt)}`);
    }
    process.stdout.write(`[openagents-desktop] IDE-02 packaged journey: ${receiptPath}\n`);
  } finally {
    await cdpBrowser?.close();
    appProcess.kill("SIGTERM");
    await Promise.race([
      new Promise<void>(resolve => appProcess.once("exit", () => resolve())),
      new Promise<void>(resolve => setTimeout(resolve, 5_000)),
    ]);
    if (appProcess.exitCode === null) appProcess.kill("SIGKILL");
    if (existsSync(userDataPath) && statSync(userDataPath).isDirectory()) {
      rmSync(userDataPath, { recursive: true, force: true });
    }
  }
};

await main();
