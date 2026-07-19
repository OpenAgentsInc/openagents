import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { _electron as electron } from "playwright";
import { Schema } from "effect";

import { IdePathIndexPackagedJourneyReceiptSchema } from "../src/ide/index-benchmark-contract.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const packagedBinary = path.join(
  appRoot,
  "out",
  "OpenAgents-darwin-arm64",
  "OpenAgents.app",
  "Contents",
  "MacOS",
  "OpenAgents",
);
const screenshotRef = "apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-02-packaged-explorer.png";
const screenshotPath = path.join(repositoryRoot, screenshotRef);
const receiptPath = path.join(
  appRoot,
  "benchmarks",
  "ide",
  "2026-07-19-ide-02-packaged-journey.json",
);

const workspaceRoot = process.argv[2];
if (workspaceRoot === undefined || !path.isAbsolute(workspaceRoot) || !existsSync(workspaceRoot)) {
  throw new Error("usage: pnpm run ide:path-index-packaged-journey -- <absolute disposable tracked-source archive>");
}
const relativeToTemp = path.relative(path.resolve(tmpdir()), path.resolve(workspaceRoot));
if (relativeToTemp === "" || relativeToTemp === ".." || relativeToTemp.startsWith(`..${path.sep}`) || path.isAbsolute(relativeToTemp)) {
  throw new Error("IDE-02 packaged journey accepts only a disposable corpus beneath the OS temporary directory");
}
if (!existsSync(packagedBinary)) {
  throw new Error(`packaged Desktop binary missing at ${packagedBinary}`);
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
  const app = await electron.launch({
    executablePath: packagedBinary,
    cwd: workspaceRoot,
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_USER_DATA: userDataPath,
      OPENAGENTS_DESKTOP_LAUNCH_CWD: workspaceRoot,
      OA_DESKTOP_SKIP_DEV_VOICE_HELPER: "1",
    },
  });
  try {
    const page = await app.firstWindow();
    await page.waitForSelector('aside[aria-label="Sessions"]', {
      timeout: 60_000,
    });
    await page.keyboard.press(process.platform === "darwin" ? "Meta+E" : "Control+E");
    const tree = page.locator('[data-oa-pierre-tree="true"]');
    await tree.waitFor({ state: "visible", timeout: 60_000 });
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
    await app.close();
    if (existsSync(userDataPath) && statSync(userDataPath).isDirectory()) {
      rmSync(userDataPath, { recursive: true, force: true });
    }
  }
};

await main();
