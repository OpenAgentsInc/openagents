#!/usr/bin/env node

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  SARAH_LIVEKIT_FAILURE_SCENARIOS,
  buildSarahLiveKitFailureMatrixReceipt,
  type SarahLiveKitFailureMatrixObservation,
} from "./failure-matrix.js";

const OWNER_GATE = "I_ACCEPT_EP263_SARAH_FAILURE_MATRIX";
const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const receiptRoot = resolve(repositoryRoot, "docs/ops/receipts/livekit");

type Arguments = Readonly<{
  apply: boolean;
  input?: string;
  receipt?: string;
}>;

const usage = () => {
  process.stderr.write(`Usage:
  pnpm --dir apps/sarah-livekit-agent failure-matrix -- \\
    --input <private observation JSON outside the repository> \\
    --receipt docs/ops/receipts/livekit/<name>.json --apply

Dry-run is the default. It performs no network request, fault injection, pod
mutation, provider disconnect, or receipt write.

--apply validates observations collected by the production runbook and writes
one public-safe receipt. It still performs no live or destructive action. It
requires:
  OA_SARAH_LIVEKIT_FAILURE_MATRIX_OWNER_GATE=${OWNER_GATE}

The private observation must remain outside the repository. The harness refuses
missing scenarios, overlapping worker/provider generations, duplicate terminal
events, stale-generation reconnect, usage/hold/settlement disagreement, and any
secret, raw-media, or transcript finding.
`);
};

const parseArguments = (values: readonly string[]): Arguments => {
  const parsed: { apply: boolean; input?: string; receipt?: string } = { apply: false };
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === "--") continue;
    if (value === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (value === "--help" || value === "-h") {
      usage();
      process.exit(0);
    }
    if (value !== "--input" && value !== "--receipt") {
      throw new Error(`unsupported argument ${value}`);
    }
    const next = values[index + 1];
    if (next === undefined || next.startsWith("--")) throw new Error(`${value} requires a value`);
    parsed[value === "--input" ? "input" : "receipt"] = next;
    index += 1;
  }
  if (!parsed.apply && (parsed.input !== undefined || parsed.receipt !== undefined)) {
    throw new Error("--input and --receipt are accepted only with --apply");
  }
  if (parsed.apply && (parsed.input === undefined || parsed.receipt === undefined)) {
    throw new Error("--apply requires --input and --receipt");
  }
  return parsed;
};

const run = async () => {
  const args = parseArguments(process.argv.slice(2));
  if (!args.apply) {
    process.stdout.write(
      `${JSON.stringify({
        schema: "openagents.sarah.livekit-failure-matrix.dry-run.v1",
        environment: "production",
        liveActionExecuted: false,
        receiptWritten: false,
        requiredOwnerGate: OWNER_GATE,
        scenarios: SARAH_LIVEKIT_FAILURE_SCENARIOS,
        guarantees: [
          "exact usage, hold, and settlement reconciliation per scenario",
          "one terminal event and one worker/provider generation at a time",
          "fresh generation after reconnect without settled-generation revival",
          "digest-only authority and evidence projection",
          "zero secret, raw-media, and transcript findings",
        ],
      })}\n`,
    );
    return;
  }
  if (process.env.OA_SARAH_LIVEKIT_FAILURE_MATRIX_OWNER_GATE !== OWNER_GATE) {
    throw new Error(`--apply requires OA_SARAH_LIVEKIT_FAILURE_MATRIX_OWNER_GATE=${OWNER_GATE}`);
  }
  const inputPath = resolve(args.input as string);
  if (inputPath === repositoryRoot || inputPath.startsWith(`${repositoryRoot}/`)) {
    throw new Error("private failure-matrix observation must remain outside the repository");
  }
  const receiptPath = resolve(repositoryRoot, args.receipt as string);
  if (!receiptPath.startsWith(`${receiptRoot}/`)) {
    throw new Error("receipt path must be under docs/ops/receipts/livekit");
  }
  const observation = JSON.parse(
    await readFile(inputPath, "utf8"),
  ) as SarahLiveKitFailureMatrixObservation;
  const observedAtMs = Date.parse(observation.observedAt);
  const observationAgeMs = Date.now() - observedAtMs;
  if (
    !Number.isFinite(observedAtMs) ||
    observationAgeMs < -5 * 60_000 ||
    observationAgeMs > 24 * 60 * 60_000
  ) {
    throw new Error("private failure-matrix observation must be from the last 24 hours");
  }
  const receipt = buildSarahLiveKitFailureMatrixReceipt(observation);
  await mkdir(dirname(receiptPath), { recursive: true });
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  process.stdout.write(
    `${JSON.stringify({
      outcome: receipt.outcome,
      receiptRef: receipt.receiptRef,
      resultDigest: receipt.resultDigest,
      receiptPath,
    })}\n`,
  );
};

try {
  await run();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  usage();
  process.exitCode = 1;
}
