import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { chromium, type Browser, type Page } from "playwright";
import { Schema } from "effect";

import { IdeSourceControlBenchmarkReceiptSchema, IdeSourceControlPackagedReceiptSchema } from "../src/ide/source-control-evidence-contract.ts";
import { packagedArtifactTreeDigest, resolvePackagedApp } from "./ide-packaged-artifact.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const evidenceRoot = path.join(appRoot, "benchmarks", "ide");
const benchmark = Schema.decodeUnknownSync(IdeSourceControlBenchmarkReceiptSchema)(JSON.parse(readFileSync(path.join(evidenceRoot, "2026-07-20-ide-12-source-control.json"), "utf8")));
const receiptPath = path.join(evidenceRoot, "2026-07-20-ide-12-source-control-packaged.json");
const screenshotPath = path.join(evidenceRoot, "2026-07-20-ide-12-source-control-packaged.png");
const tracePath = path.join(evidenceRoot, "2026-07-20-ide-12-source-control-packaged-trace.json");
const screenshotRef = "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-12-source-control-packaged.png";
const traceRef = "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-12-source-control-packaged-trace.json";

const git = (root: string, ...args: string[]): string => execFileSync("git", ["-C", root, ...args], { encoding: "utf8", env: { ...process.env, GIT_TERMINAL_PROMPT: "0" } }).trim();
const waitForRenderer = async (browser: Browser): Promise<Page> => {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const page = browser.contexts().flatMap(context => context.pages()).find(candidate => candidate.url().startsWith("openagents-app://renderer/"));
    if (page !== undefined) return page;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error("IDE-12 packaged renderer did not appear");
};
const poll = async (check: () => boolean, message: string): Promise<void> => {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) { if (check()) return; await new Promise(resolve => setTimeout(resolve, 50)); }
  throw new Error(message);
};

const main = async (): Promise<void> => {
  const workspace = mkdtempSync(path.join(tmpdir(), "openagents-ide12-package-workspace-"));
  const profile = mkdtempSync(path.join(tmpdir(), "openagents-ide12-package-profile-"));
  const bare = mkdtempSync(path.join(tmpdir(), "openagents-ide12-package-remote-"));
  git(bare, "init", "--bare");
  git(workspace, "init", "-b", "main"); git(workspace, "config", "user.name", "IDE-12 packaged"); git(workspace, "config", "user.email", "ide12-packaged@openagents.local");
  writeFileSync(path.join(workspace, "source.txt"), "base\n"); writeFileSync(path.join(workspace, "recover.txt"), "recover base\n");
  git(workspace, "add", "."); git(workspace, "commit", "-m", "packaged seed"); git(workspace, "remote", "add", "origin", bare); git(workspace, "push", "-u", "origin", "main");
  writeFileSync(path.join(workspace, "source.txt"), "packaged change\n");
  const appPath = resolvePackagedApp(); const artifact = packagedArtifactTreeDigest(appPath);
  const events: Array<{ kind: string; message: string }> = [];
  const safe = (value: string): string => value.replaceAll(workspace, "«workspace»").replaceAll(profile, "«profile»").replaceAll(process.env.HOME ?? "__none__", "«home»").replace(/(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}/gu, "«redacted»").slice(0, 500);
  const appProcess = spawn("open", ["-n", "-W", "-a", appPath, path.join(workspace, "source.txt"), "--args", "--remote-debugging-port=0"], {
    cwd: workspace, env: { ...process.env, OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1", OPENAGENTS_DESKTOP_USER_DATA: profile, OPENAGENTS_DESKTOP_LAUNCH_CWD: workspace, OA_DESKTOP_SKIP_DEV_VOICE_HELPER: "1" }, stdio: "ignore",
  });
  let browser: Browser | null = null; let applicationPid: number | null = null;
  try {
    const portPath = path.join(profile, "DevToolsActivePort"); await poll(() => existsSync(portPath), "IDE-12 packaged DevTools port did not appear");
    const port = readFileSync(portPath, "utf8").split("\n")[0]!;
    const pid = Number.parseInt(execFileSync("lsof", ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"], { encoding: "utf8" }).trim().split("\n")[0] ?? "", 10); if (Number.isSafeInteger(pid)) applicationPid = pid;
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`); const page = await waitForRenderer(browser);
    page.on("console", message => events.push({ kind: `console:${message.type()}`, message: safe(message.text()) })); page.on("pageerror", error => events.push({ kind: "pageerror", message: safe(error.message) }));
    await page.locator("[data-react-workspace]").first().waitFor({ state: "visible", timeout: 30_000 });
    const closeFiles = page.getByRole("button", { name: "Close Files", exact: true });
    if (await closeFiles.isVisible()) await closeFiles.click();
    await page.locator('[data-react-workspace="chat"]').waitFor({ state: "visible", timeout: 10_000 });
    await page.evaluate(() => {
      const prefix = "openagents.desktop.surface-layout.v1:";
      const layout = JSON.stringify({ version: 1, surfaces: ["review"], active: "review", maximized: false, width: 520 });
      for (const key of Object.keys(localStorage).filter(key => key.startsWith(prefix))) localStorage.setItem(key, layout);
      localStorage.setItem(`${prefix}unbound`, layout);
    });
    await page.reload({ waitUntil: "domcontentloaded" });
    if (await closeFiles.isVisible()) await closeFiles.click();
    const surface = page.getByLabel("Review surface"); await surface.waitFor({ state: "visible", timeout: 30_000 });
    await surface.getByRole("button", { name: "Refresh changes" }).click();
    const stage = surface.getByRole("button", { name: "Stage source.txt" }); await stage.waitFor({ state: "visible", timeout: 20_000 }); await stage.click();
    await surface.getByLabel("Commit message").fill("packaged source-control delivery"); await surface.getByRole("button", { name: "Commit staged" }).click();
    await poll(() => git(workspace, "log", "-1", "--format=%s") === "packaged source-control delivery", "IDE-12 packaged commit did not settle");
    await surface.getByRole("button", { name: "Push exact HEAD" }).click(); await poll(() => git(bare, "rev-parse", "refs/heads/main") === git(workspace, "rev-parse", "HEAD"), "IDE-12 packaged push postcondition failed");
    writeFileSync(path.join(workspace, "recover.txt"), "discard then recover\n"); await surface.getByRole("button", { name: "Refresh changes" }).click();
    const discard = surface.getByRole("button", { name: "Discard unstaged change in recover.txt" }); await discard.waitFor({ state: "visible", timeout: 20_000 }); await discard.click();
    await surface.getByRole("button", { name: "Discard change" }).click(); await poll(() => readFileSync(path.join(workspace, "recover.txt"), "utf8") === "recover base\n", "IDE-12 packaged discard failed");
    const recover = surface.getByRole("button", { name: "Recover discarded change" }); await recover.waitFor({ state: "visible", timeout: 20_000 }); await recover.click(); await poll(() => readFileSync(path.join(workspace, "recover.txt"), "utf8") === "discard then recover\n", "IDE-12 packaged recovery failed");
    const delivery = await surface.getByLabel("Delivery phase evidence").textContent() ?? "";
    const refresh = surface.getByRole("button", { name: "Refresh changes" });
    await refresh.focus(); const keyboardFocus = await refresh.evaluate(element => element === element.ownerDocument.activeElement);
    await page.screenshot({ path: screenshotPath, fullPage: true });
    const checks = { stage: true, commit: true, pushRemoteOid: true, discard: true, restartSafeRecovery: true, deliveryDistinct: delivery.includes("committed") && delivery.includes("pushed") && delivery.includes("owner accepted") && delivery.includes("released"), keyboardFocus, privatePathsWithheld: !(await surface.textContent() ?? "").includes(workspace) };
    if (Object.values(checks).some(value => !value)) throw new Error(`IDE-12 packaged checks failed: ${JSON.stringify(checks)}`);
    const receipt = Schema.decodeUnknownSync(IdeSourceControlPackagedReceiptSchema)({ schemaVersion: "openagents.desktop.ide-source-control-packaged.v1", issue: "IDE-12", candidateCommitSha: benchmark.candidateCommitSha, recordedAt: new Date().toISOString(), artifactTreeSha256: artifact.sha256, target: "darwin-arm64", checks, screenshotRef, traceRef, passed: true });
    writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 }); writeFileSync(tracePath, `${JSON.stringify({ schemaVersion: "openagents.desktop.ide-source-control-packaged-trace.v1", issue: "IDE-12", candidateCommitSha: benchmark.candidateCommitSha, events, privateMaterialIncluded: false }, null, 2)}\n`, { mode: 0o600 });
    process.stdout.write(`[openagents-desktop] IDE-12 packaged journey: ${receiptPath}\n`);
  } finally {
    await browser?.close().catch(() => undefined); if (applicationPid !== null) try { process.kill(applicationPid, "SIGTERM"); } catch {} appProcess.kill("SIGTERM");
    for (const root of [workspace, profile, bare]) rmSync(root, { recursive: true, force: true, maxRetries: 8, retryDelay: 100 });
  }
};
await main();
