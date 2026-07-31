#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { mkdirSync } from "node:fs";
import {
  LIVEKIT_OPS,
  OBSERVATION_VALIDATORS,
  buildPublicReceipt,
  sha256,
  validateDeploymentBundle,
} from "./livekit-ops-policy.mjs";

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-acceptance.mjs \\
    --phase connectivity|load|drills|sarah_matrix|secret_scan|cost|rollback \\
    --bundle infra/livekit/bundle.json \\
    [--input PRIVATE_OBSERVATION.json --receipt PUBLIC_RECEIPT.json --apply]

Default mode validates the immutable deployment bundle and prints the exact
acceptance contract. It performs no live probe and writes no receipt.

--apply validates a real observation produced by the packaged Omega, load,
failure-drill, or redaction harness. It requires:
  OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST

The public receipt contains only opaque refs, digests, timestamps, outcomes,
and limitations. Do not pass a public receipt path for private evidence.
`);
};

const parseArgs = (args) => {
  const parsed = {
    apply: false,
    bundle: undefined,
    input: undefined,
    phase: undefined,
    receipt: undefined,
  };
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (["--bundle", "--input", "--phase", "--receipt"].includes(argument)) {
      const value = args[index + 1];
      if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
      parsed[argument.slice(2)] = value;
      index += 1;
      continue;
    }
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    throw new Error(`unsupported argument ${argument}`);
  }
  if (!parsed.phase || !Object.hasOwn(OBSERVATION_VALIDATORS, parsed.phase)) {
    throw new Error(
      "--phase must be connectivity, load, drills, sarah_matrix, secret_scan, cost, or rollback",
    );
  }
  if (!parsed.bundle) throw new Error("--bundle is required");
  if (parsed.apply && (!parsed.input || !parsed.receipt)) {
    throw new Error("--apply requires --input and --receipt");
  }
  if (!parsed.apply && (parsed.input || parsed.receipt)) {
    throw new Error("--input and --receipt are accepted only with --apply");
  }
  return parsed;
};

const readJson = (path, label) => {
  try {
    return JSON.parse(readFileSync(resolve(path), "utf8"));
  } catch (error) {
    throw new Error(`${label} is not readable JSON`, { cause: error });
  }
};

const expectation = (phase, bundle) => {
  const shared = {
    mode: "dry-run",
    phase,
    project: LIVEKIT_OPS.project,
    region: LIVEKIT_OPS.region,
    bundleDigest: sha256(JSON.stringify(bundle)),
    liveProbeExecuted: false,
    receiptWritten: false,
  };
  switch (phase) {
    case "connectivity":
      return {
        ...shared,
        required: [
          "packaged Omega room join",
          "direct UDP selected-path observation",
          "WebRTC TCP fallback selected-path observation",
          "TURN/TLS selected-path observation",
          "microphone publish and Sarah audio subscribe",
          "trusted certificate, signaling, public-IP advertisement, and exact settlement",
        ],
      };
    case "load":
      return {
        ...shared,
        required: [
          `${bundle.limits.maxConcurrentSarahRooms} concurrent two-participant Sarah rooms`,
          "at least 20% measured spare capacity",
          "hard application cap refusal above admitted concurrency",
          "SFU and worker CPU no higher than 80%",
          "packet loss no higher than 5%",
          "every admitted session terminal and settled",
        ],
      };
    case "drills":
      return {
        ...shared,
        required: [
          "SFU pod drain, node loss, and zone loss",
          "Redis failover, signaling removal, certificate renewal, and TURN loss",
          "worker crash, provider disconnect, and quota exhaustion",
          "server rollback",
          "visible bounded failure, no provider overlap, terminal accounting, fresh admission",
          "no unsupported uninterrupted-speech claim",
        ],
      };
    case "sarah_matrix":
      return {
        ...shared,
        required: [
          "literal principal.sarah and eight distinct session authority identities",
          "nonzero provider response and transcription usage equal to exact settlement",
          "success, cancellation, explicit interruption, timeout, planned worker crash, provider disconnect, hold exhaustion, and reconnect",
          "no reconnect worker or provider overlap and a fresh generation",
          "audible output observed by at least two simultaneous subscribers",
          "packaged Omega, packaged clients, pods, logs, Redis, object storage, traces, and crash artifacts privacy scan",
        ],
      };
    case "secret_scan":
      return {
        ...shared,
        required: [
          "pods, logs, Redis, object storage, traces, and crash artifacts",
          "zero secret, credential, raw media, and transcript findings",
          "forbidden-pattern corpus executed without recording matched values",
        ],
      };
    case "cost":
      return {
        ...shared,
        required: [
          "fixed monthly production floor at or above $1,500",
          "measured daily cost and current monthly forecast",
          "active Google budget alerts",
          `hard room cap of ${bundle.limits.maxConcurrentSarahRooms}`,
          "Google credits are not modeled as zero cost",
          "OpenAI usage remains a separate provider ledger",
        ],
      };
    case "rollback":
      return {
        ...shared,
        required: [
          "disable new livekit_room_v1 admission first",
          "drain or explicitly fail every existing room",
          "settle every hold and provider generation",
          "restore the previous pinned revision",
          "no silent custom_wss_v1 switch and no unrelated service mutation",
        ],
      };
    default:
      throw new Error(`unsupported phase ${phase}`);
  }
};

const run = () => {
  const args = parseArgs(process.argv.slice(2));
  const bundle = validateDeploymentBundle(readJson(args.bundle, "deployment bundle"));
  if (!args.apply) {
    process.stdout.write(`${JSON.stringify(expectation(args.phase, bundle), null, 2)}\n`);
    return;
  }
  if (process.env.OA_LIVEKIT_OWNER_GATE !== "I_ACCEPT_EP263_LIVEKIT_GCP_COST") {
    throw new Error("--apply requires OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST");
  }
  const observation = readJson(args.input, "private observation");
  if (observation.sourceBaseRevision !== bundle.sourceBaseRevision) {
    throw new Error("observation source base does not match the immutable deployment bundle");
  }
  const receipt = buildPublicReceipt(observation, bundle);
  if (receipt.phase !== args.phase) {
    throw new Error("observation phase does not match --phase");
  }
  const receiptPath = resolve(args.receipt);
  if (!receiptPath.includes(`${resolve("docs/ops/receipts/livekit")}/`)) {
    throw new Error("public receipt must stay under docs/ops/receipts/livekit");
  }
  mkdirSync(dirname(receiptPath), { recursive: true });
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  process.stdout.write(
    `${JSON.stringify({
      phase: receipt.phase,
      outcome: receipt.outcome,
      receiptRef: receipt.receiptRef,
      resultDigest: receipt.resultDigest,
      receiptPath,
    })}\n`,
  );
};

try {
  run();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  usage();
  process.exitCode = 1;
}
