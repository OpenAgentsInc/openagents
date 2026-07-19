import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { createPackage } from "@electron/asar";
import { Schema } from "effect";

import {
  IdePackageSpikeMatrixReceiptSchema,
  decodeIdePackageSpikeProbeReceipt,
  type IdePackageSpikeProbeReceipt,
} from "../src/ide/package-spike-contract.ts";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const electronBinary = path.join(repositoryRoot, "node_modules", ".bin", "electron");
const outputPath = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-01-package-spike.json");
const screenshotPath = path.join(appRoot, "benchmarks", "ide", "2026-07-19-ide-01-tokyo-night.png");
const receiptPrefix = "[openagents-desktop ide-package-spike] ";

const runProbe = (
  application: string,
  layout: IdePackageSpikeProbeReceipt["layout"],
): IdePackageSpikeProbeReceipt => {
  const result = spawnSync(electronBinary, [application], {
    cwd: repositoryRoot,
    encoding: "utf8",
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_IDE_PACKAGE_SPIKE: "1",
      OPENAGENTS_DESKTOP_IDE_PACKAGE_LAYOUT: layout,
      ...(layout === "development"
        ? { OPENAGENTS_DESKTOP_IDE_PACKAGE_SCREENSHOT_PATH: screenshotPath }
        : {}),
    },
    maxBuffer: 8 * 1024 * 1024,
    timeout: 120_000,
  });
  if (result.status !== 0) {
    throw new Error(
      `${layout} probe exited ${String(result.status)}: ${result.stderr || result.stdout}`,
    );
  }
  const line = result.stdout
    .split(/\r?\n/u)
    .find((candidate) => candidate.startsWith(receiptPrefix));
  if (line === undefined) throw new Error(`${layout} probe did not emit a receipt`);
  return decodeIdePackageSpikeProbeReceipt(JSON.parse(line.slice(receiptPrefix.length)));
};

const assertProbe = (receipt: IdePackageSpikeProbeReceipt): void => {
  if (receipt.cycles.length !== 3) throw new Error(`${receipt.layout}: expected three cycles`);
  for (const [index, cycle] of receipt.cycles.entries()) {
    const ready = cycle.ready;
    const disposed = cycle.disposed;
    if (ready.cycle !== index || ready.phase !== "ready")
      throw new Error(`${receipt.layout}: invalid ready cycle ${index}`);
    if (
      ready.monaco.languageWorkersReady.join(",") !== "editor,json,css,html,typescript" ||
      ready.monaco.modelCount !== 4 ||
      ready.monaco.editorsCreated !== 1
    )
      throw new Error(`${receipt.layout}: Monaco workers/models did not initialize`);
    if (
      !ready.pierre.rendered ||
      !ready.pierre.unified ||
      !ready.pierre.split ||
      !ready.pierre.annotation ||
      !ready.pierre.selectedRange ||
      !ready.pierre.workerInitialized ||
      !ready.pierre.virtualized ||
      ready.pierre.scaleItems !== 200 ||
      ready.pierre.renderedScaleItems >= ready.pierre.scaleItems
    )
      throw new Error(`${receipt.layout}: Pierre fixture did not fully render`);
    if (ready.resources.externalUrls.length !== 0 || ready.resources.loadedUrls.length < 5)
      throw new Error(`${receipt.layout}: offline resource proof failed`);
    if (disposed.monaco.modelCount !== 0 || disposed.resources.activeWorkers !== 0)
      throw new Error(`${receipt.layout}: cycle ${index} leaked models/workers`);
  }
  if (
    receipt.expectedFailure.phase !== "expected_failure" ||
    receipt.expectedFailure.monaco.failureLabel !== "injected-typescript-worker-failure" ||
    receipt.expectedFailure.monaco.modelCount !== 0 ||
    receipt.expectedFailure.resources.activeWorkers !== 0 ||
    receipt.expectedFailure.resources.externalUrls.length !== 0
  )
    throw new Error(`${receipt.layout}: expected worker failure did not fail closed`);
};

const main = async (): Promise<void> => {
  const temporaryRoot = mkdtempSync(path.join(tmpdir(), "oa-ide-package-spike-"));
  try {
    const stagedApp = path.join(temporaryRoot, "app");
    const archivePath = path.join(temporaryRoot, "app.asar");
    mkdirSync(stagedApp);
    cpSync(path.join(appRoot, "dist"), path.join(stagedApp, "dist"), { recursive: true });
    writeFileSync(
      path.join(stagedApp, "package.json"),
      `${JSON.stringify({ name: "openagents-ide-package-spike", main: "dist/main.js" }, null, 2)}\n`,
    );
    await createPackage(stagedApp, archivePath);

    const development = runProbe(appRoot, "development");
    const asar = runProbe(archivePath, "asar");
    assertProbe(development);
    assertProbe(asar);

    const commitSha = spawnSync("git", ["rev-parse", "HEAD"], {
      cwd: repositoryRoot,
      encoding: "utf8",
    }).stdout.trim();
    const matrix = Schema.decodeUnknownSync(IdePackageSpikeMatrixReceiptSchema)({
      schemaVersion: "openagents.desktop.ide-package-spike-matrix.v1",
      capturedAt: new Date().toISOString(),
      commitSha,
      development,
      asar,
      assertions: [
        "Development and ASAR layouts each complete three create/dispose cycles.",
        "Monaco editor, JSON, CSS, HTML, and TypeScript workers initialize from private-scheme assets.",
        "Pierre renders unified/split diffs, a controlled selection, an annotation, and its worker pool.",
        "Pierre CodeView virtualizes a 200-file review collection rather than mounting every file.",
        "Every observed request stays on openagents-app:; the fixture requires no network.",
        "Every disposal reaches zero Monaco models and zero tracked workers.",
        "An injected TypeScript-worker construction failure disposes every model and tracked worker.",
      ],
    });
    mkdirSync(path.dirname(outputPath), { recursive: true });
    writeFileSync(outputPath, `${JSON.stringify(matrix, null, 2)}\n`);
    console.log(`[openagents-desktop] IDE package spike receipt: ${outputPath}`);
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
};

await main();
