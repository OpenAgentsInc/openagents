#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(appRoot, "..", "..");
const defaultArtifactPath = path.join(repoRoot, "output", "l402-local-node-smoke-artifact.json");

const args = process.argv.slice(2);
const wantsJson = args.includes("--json");
const artifactArgIndex = args.indexOf("--artifact");
const artifactPath = artifactArgIndex >= 0 ? path.resolve(args[artifactArgIndex + 1] ?? defaultArtifactPath) : defaultArtifactPath;

const npmExecutable = process.platform === "win32" ? "npm.cmd" : "npm";
const startedAt = Date.now();
fs.mkdirSync(path.dirname(artifactPath), { recursive: true });
if (fs.existsSync(artifactPath)) {
  fs.rmSync(artifactPath, { force: true });
}

const result = spawnSync(
  npmExecutable,
  ["test", "--", "tests/l402LocalNodeFlow.integration.test.ts"],
  {
    cwd: appRoot,
    encoding: "utf8",
    stdio: "pipe",
    env: {
      ...process.env,
      OA_L402_LOCAL_NODE_ARTIFACT_PATH: artifactPath,
    },
  },
);
const durationMs = Date.now() - startedAt;

const artifact = fs.existsSync(artifactPath)
  ? JSON.parse(fs.readFileSync(artifactPath, "utf8"))
  : null;

const observabilityRecords = Array.isArray(artifact?.flows)
  ? artifact.flows.flatMap((flow) =>
      Array.isArray(flow?.observabilityRecords) ? flow.observabilityRecords : [],
    )
  : [];

const summary = {
  ok: result.status === 0,
  status: result.status,
  durationMs,
  testFile: "tests/l402LocalNodeFlow.integration.test.ts",
  artifactPath,
  flowCount: Array.isArray(artifact?.flows) ? artifact.flows.length : 0,
  observabilityRecordCount: observabilityRecords.length,
  flowCorrelations: Array.isArray(artifact?.flows)
    ? artifact.flows.map((flow) => ({
        flow: flow.flow,
        taskId: flow.taskId,
        createRequestId: flow.createRequestId,
        transitionRequestIds: flow.transitionRequestIds,
        proofReference: flow.proofReference ?? null,
        blockedErrorCode: flow.blockedErrorCode ?? null,
        observabilityRecords: Array.isArray(flow.observabilityRecords)
          ? flow.observabilityRecords.map((record) => ({
              requestId: record.requestId ?? null,
              taskId: record.taskId ?? null,
              paywallId: record.paywallId ?? null,
              paymentProofRef: record.paymentProofRef ?? null,
              executionPath: record.executionPath ?? null,
              desktopSessionId: record.desktopSessionId ?? null,
              desktopRuntimeStatus: record.desktopRuntimeStatus ?? null,
              walletState: record.walletState ?? null,
              nodeSyncStatus: record.nodeSyncStatus ?? null,
              plane: record.plane ?? null,
              executor: record.executor ?? null,
            }))
          : [],
      }))
    : [],
};

if (wantsJson) {
  process.stdout.write(`${JSON.stringify(summary)}\n`);
} else if (summary.ok) {
  process.stdout.write(`L402 local-node smoke passed in ${durationMs}ms (artifact: ${artifactPath})\n`);
} else {
  process.stdout.write(`L402 local-node smoke failed in ${durationMs}ms\n`);
  if (result.stdout) process.stdout.write(`${result.stdout}\n`);
  if (result.stderr) process.stderr.write(`${result.stderr}\n`);
}

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}
