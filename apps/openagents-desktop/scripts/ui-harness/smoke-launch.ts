/**
 * Minimal harness self-check: launch the isolated preview, wait for the
 * composer shell to paint, screenshot it, quit. Proves
 * `launchIsolatedDesktopApp` actually produces a real clickable window
 * before any evidence-gathering script builds on top of it.
 *
 * Usage: node --import tsx scripts/ui-harness/smoke-launch.ts <scratch-workspace-dir> <out-png>
 */
import path from "node:path";
import { launchIsolatedDesktopApp } from "./launch-isolated-app.ts";

const main = async (): Promise<void> => {
  const launchCwd = process.argv[2];
  const outPng = process.argv[3];
  if (launchCwd === undefined || outPng === undefined) {
    throw new Error("usage: smoke-launch.ts <scratch-workspace-dir> <out-png>");
  }
  console.log(`[ui-harness smoke] launching against launchCwd=${launchCwd}`);
  const desktop = await launchIsolatedDesktopApp({ launchCwd });
  console.log(`[ui-harness smoke] launched, userData=${desktop.userDataPath}`);
  try {
    await desktop.page.waitForSelector('[data-en-key], [class*="oa-react"]', { timeout: 60_000 });
    const title = await desktop.page.title();
    const url = desktop.page.url();
    console.log(`[ui-harness smoke] window ready: title=${JSON.stringify(title)} url=${url}`);
    await desktop.page.screenshot({ path: path.resolve(outPng) });
    console.log(`[ui-harness smoke] screenshot written to ${path.resolve(outPng)}`);
  } finally {
    await desktop.close();
  }
};

await main().catch((error) => {
  console.error(
    "[ui-harness smoke] FAILED",
    error instanceof Error ? (error.stack ?? error.message) : error,
  );
  process.exit(1);
});
