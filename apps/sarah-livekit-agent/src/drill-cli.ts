#!/usr/bin/env node

import { randomUUID } from "node:crypto";
import { execFile } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import { dispose } from "@livekit/rtc-node";
import {
  SARAH_LIVEKIT_DRILL_DEFAULT_HOLD_MS,
  SARAH_LIVEKIT_DRILL_SCENARIOS,
  assertPublicSafeSarahLiveKitDrillObservation,
  digestDrillInstance,
  runSarahLiveKitDrill,
  type SarahLiveKitDrillFaultResult,
  type SarahLiveKitDrillScenario,
} from "./drill-driver.js";
import { EXPECTED_FAULT_ACTION, SARAH_LIVEKIT_SFU_LOSS_BOUND_MS } from "./failure-matrix.js";

const OWNER_GATE = "I_ACCEPT_EP263_SARAH_LIVEKIT_DRILL";
const LIVEKIT_NAMESPACE = "livekit-system";
const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const receiptRoot = resolve(repositoryRoot, "docs/ops/receipts/livekit");
const run_ = promisify(execFile);

type Arguments = Readonly<{
  apply: boolean;
  scenario?: string;
  room?: string;
  pcm?: string;
  boundMs?: string;
  observationWindowMs?: string;
  holdMs?: string;
  communityRef?: string;
  channelRef?: string;
  faultPod?: string;
  workerPod?: string;
  billableSessions?: string;
  receipt?: string;
}>;

const FIELDS = new Map<string, keyof Omit<Arguments, "apply">>([
  ["--scenario", "scenario"],
  ["--room", "room"],
  ["--pcm", "pcm"],
  ["--bound-ms", "boundMs"],
  ["--observation-window-ms", "observationWindowMs"],
  ["--hold-ms", "holdMs"],
  ["--community-ref", "communityRef"],
  ["--channel-ref", "channelRef"],
  ["--fault-pod", "faultPod"],
  ["--worker-pod", "workerPod"],
  ["--billable-sessions", "billableSessions"],
  ["--receipt", "receipt"],
]);

const usage = () => {
  process.stderr.write(`Usage:
  pnpm --dir apps/sarah-livekit-agent drill -- \\
    --scenario ${SARAH_LIVEKIT_DRILL_SCENARIOS.join("|")} \\
    --room private|community [--community-ref <ref> --channel-ref <ref>] \\
    --pcm <24kHz mono s16le prompt outside the repository> \\
    --bound-ms <bound measured from the fault> \\
    [--observation-window-ms <window, default 2x the bound>] \\
    [--hold-ms <live hold before the fault, default ${SARAH_LIVEKIT_DRILL_DEFAULT_HOLD_MS}>] \\
    [--fault-pod <pod the operator already identified> --worker-pod <Sarah worker pod>] \\
    --billable-sessions <billable Sarah sessions live in the cluster right now> \\
    --receipt docs/ops/receipts/livekit/<name>.json --apply

One session, brought live through the production acceptance path, held open, and
faulted at a recorded instant. The two-room acceptance harness cannot do this: it
requires two concurrent scenarios and asserts their overlap, which no drill can
hold a single session through and which sfu_loss forbids outright.

sfu_loss pins its bound to the ${SARAH_LIVEKIT_SFU_LOSS_BOUND_MS} ms the failure
matrix defines and refuses any other value, so a run cannot satisfy this command
and then fail the receipt validator.

TARGET SELECTION IS THE OPERATOR'S. This command never searches for a pod to
destroy. --fault-pod and --worker-pod name pods the operator already identified
with the runbook procedure, in namespace ${LIVEKIT_NAMESPACE}. Both are recorded
only as digests. A pod-deleting fault refuses to run when the two names are equal.

Dry-run is the default and performs no network request, no fault, and no write.
A live run requires:
  OA_SARAH_LIVEKIT_DRILL_OWNER_GATE=${OWNER_GATE}
  OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_BEARER    (--room private)
  OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_OWNER_REF (--room private)
  OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_BEARER    (--room community)
  OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_OWNER_REF (--room community)

A drill that exceeds its bound, never settles, or aims at the wrong instance is
recorded with outcome "contradicted" and exits nonzero. That is a finding to
keep, not a run to repeat until it looks better.
`);
};

const parseArguments = (values: readonly string[]): Arguments => {
  const parsed: { apply: boolean } & Record<string, string | boolean> = { apply: false };
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
    const field = FIELDS.get(value as string);
    const next = values[index + 1];
    if (field === undefined || next === undefined || next.startsWith("--")) {
      throw new Error(`unsupported or incomplete argument ${value}`);
    }
    parsed[field] = next;
    index += 1;
  }
  return parsed as Arguments;
};

const requiredEnvironment = (name: string): string => {
  const value = process.env[name]?.trim();
  if (value === undefined || value === "") throw new Error(`${name} is required`);
  return value;
};

const requiredArgument = (value: string | undefined, name: string): string => {
  if (value === undefined || value.trim() === "") {
    throw new Error(`${name} is required with --apply`);
  }
  return value;
};

const wholeMs = (value: string | undefined, name: string): number => {
  const parsed = Number(requiredArgument(value, name));
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive whole number of milliseconds`);
  }
  return parsed;
};

const drillScenario = (value: string | undefined): SarahLiveKitDrillScenario => {
  const found = SARAH_LIVEKIT_DRILL_SCENARIOS.find((known) => known === value?.trim());
  if (found === undefined) {
    throw new Error(`--scenario must be one of ${SARAH_LIVEKIT_DRILL_SCENARIOS.join(", ")}`);
  }
  return found;
};

const POD_NAME = /^[a-z0-9]([a-z0-9-]{0,251}[a-z0-9])?$/u;

/**
 * Delete exactly one named pod, and nothing else.
 *
 * `execFile` with an argument array, not a shell, so no operator-supplied string
 * can widen the action. The name is checked against the Kubernetes object-name
 * grammar first, and a fault aimed at the Sarah worker pod is refused before the
 * call rather than recorded as a contradiction afterwards: destroying the worker
 * makes the observation indistinguishable from planned_worker_crash, so there is
 * no useful evidence to keep.
 */
const deleteExactPod = async (podName: string): Promise<void> => {
  if (!POD_NAME.test(podName)) throw new Error("pod name is not a Kubernetes object name");
  await run_("kubectl", [
    "delete",
    "pod",
    "--namespace",
    LIVEKIT_NAMESPACE,
    podName,
    "--wait=false",
  ]);
};

const buildInjector = (
  scenario: SarahLiveKitDrillScenario,
  args: Arguments,
): ((context: {
  requestClientCancel: () => Promise<void>;
}) => Promise<SarahLiveKitDrillFaultResult>) => {
  const faultAction = EXPECTED_FAULT_ACTION[scenario];
  if (faultAction === "delete_exact_sfu_pod" || faultAction === "delete_exact_worker_pod") {
    const faultPod = requiredArgument(args.faultPod, "--fault-pod");
    const workerPod = requiredArgument(args.workerPod, "--worker-pod");
    if (faultAction === "delete_exact_sfu_pod" && faultPod === workerPod) {
      throw new Error("sfu_loss must not destroy the Sarah worker pod");
    }
    return async () => {
      await deleteExactPod(faultPod);
      return {
        targetInstanceDigest: digestDrillInstance(faultPod),
        workerInstanceDigest: digestDrillInstance(workerPod),
        workerInstanceSurvived: await podExists(workerPod),
      };
    };
  }
  if (faultAction === "client_cancel") {
    return async (context) => {
      await context.requestClientCancel();
      return {};
    };
  }
  if (faultAction === "bounded_deadline") {
    // The timeout drill's fault is the passage of time. There is nothing to
    // execute; the instant the driver stamps is the instant the clock started
    // being the only thing that could end this session.
    return () => Promise.resolve({});
  }
  throw new Error(
    `${scenario} needs a fault this command cannot execute (${faultAction}); ` +
      "run it through the runbook procedure and record the result separately",
  );
};

const podExists = async (podName: string): Promise<boolean> => {
  if (!POD_NAME.test(podName)) throw new Error("pod name is not a Kubernetes object name");
  try {
    await run_("kubectl", [
      "get",
      "pod",
      "--namespace",
      LIVEKIT_NAMESPACE,
      podName,
      "--output",
      "name",
    ]);
    return true;
  } catch {
    return false;
  }
};

const run = async () => {
  const args = parseArguments(process.argv.slice(2));
  if (!args.apply) {
    process.stdout.write(
      `${JSON.stringify({
        schema: "openagents.sarah.livekit-drill.dry-run.v1",
        environment: "production",
        liveSessionOpened: false,
        faultInjected: false,
        receiptWritten: false,
        requiredOwnerGate: OWNER_GATE,
        scenarios: SARAH_LIVEKIT_DRILL_SCENARIOS,
        faultActions: Object.fromEntries(
          SARAH_LIVEKIT_DRILL_SCENARIOS.map((name) => [name, EXPECTED_FAULT_ACTION[name]]),
        ),
        pinnedBounds: { sfu_loss: SARAH_LIVEKIT_SFU_LOSS_BOUND_MS },
        namespace: LIVEKIT_NAMESPACE,
        guarantees: [
          "exactly one billable Sarah session for the whole drill window",
          "fault instant recorded separately from the session start",
          "bound measured from the fault, never from the session duration",
          "settlement read over HTTP, so it survives the loss of the transport",
          "a failed drill is recorded as contradicted rather than discarded",
          "pod targets are operator-named, never searched for, and recorded as digests",
        ],
      })}\n`,
    );
    return;
  }
  if (process.env.OA_SARAH_LIVEKIT_DRILL_OWNER_GATE !== OWNER_GATE) {
    throw new Error(`--apply requires OA_SARAH_LIVEKIT_DRILL_OWNER_GATE=${OWNER_GATE}`);
  }

  const scenario = drillScenario(args.scenario);
  const roomKind = args.room?.trim();
  if (roomKind !== "private" && roomKind !== "community") {
    throw new Error("--room must be private or community");
  }
  const boundMs =
    scenario === "sfu_loss" ? SARAH_LIVEKIT_SFU_LOSS_BOUND_MS : wholeMs(args.boundMs, "--bound-ms");
  const observationWindowMs =
    args.observationWindowMs === undefined
      ? boundMs * 2
      : wholeMs(args.observationWindowMs, "--observation-window-ms");
  const holdMs =
    args.holdMs === undefined
      ? SARAH_LIVEKIT_DRILL_DEFAULT_HOLD_MS
      : wholeMs(args.holdMs, "--hold-ms");
  const billableSessions = Number(requiredArgument(args.billableSessions, "--billable-sessions"));
  if (!Number.isSafeInteger(billableSessions) || billableSessions < 1) {
    throw new Error("--billable-sessions must be at least one: this drill's own session is live");
  }

  const pcmPath = resolve(requiredArgument(args.pcm, "--pcm"));
  if (pcmPath.startsWith(`${repositoryRoot}/`)) {
    throw new Error("drill PCM prompts must remain outside the repository");
  }
  const receiptPath = resolve(repositoryRoot, requiredArgument(args.receipt, "--receipt"));
  if (!receiptPath.startsWith(`${receiptRoot}/`)) {
    throw new Error("receipt path must be under docs/ops/receipts/livekit");
  }

  const injectFault = buildInjector(scenario, args);
  const suffix = roomKind === "private" ? "PRIVATE" : "COMMUNITY";
  const runRef = randomUUID();
  const pcm = await readFile(pcmPath);

  const observation = await runSarahLiveKitDrill(
    {
      scenario,
      boundMs,
      observationWindowMs,
      holdMs,
      injectFault,
      // Read at the fault instant. Operator-declared, for the same reason the
      // acceptance receipt's forced-transport profile is: nothing inside one
      // session can see how many other billable sessions the cluster is
      // carrying. The declaration is recorded and travels with the limitation.
      countBillableSessions: () => billableSessions,
      session: {
        kind: roomKind,
        bearer: requiredEnvironment(`OA_SARAH_LIVEKIT_ACCEPTANCE_${suffix}_BEARER`),
        subscriberRef: `drill-${roomKind}-subscriber-${runRef}`,
        ownerRef: requiredEnvironment(`OA_SARAH_LIVEKIT_ACCEPTANCE_${suffix}_OWNER_REF`),
        deviceRef: `drill-${roomKind}-${runRef}`,
        threadRef: `drill-${roomKind}-${runRef}`,
        sessionRef: `drill-${roomKind}-${runRef}`,
        generation: 1,
        pcm,
        roomContext:
          roomKind === "private"
            ? { kind: "private" }
            : {
                kind: "community",
                communityRef: requiredArgument(args.communityRef, "--community-ref"),
                channelRef: requiredArgument(args.channelRef, "--channel-ref"),
              },
      },
    },
    {},
  );

  assertPublicSafeSarahLiveKitDrillObservation(observation);
  await mkdir(dirname(receiptPath), { recursive: true });
  await writeFile(receiptPath, `${JSON.stringify(observation, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  process.stdout.write(
    `${JSON.stringify({
      scenario: observation.scenario,
      faultAction: observation.faultAction,
      outcome: observation.outcome,
      contradictions: observation.contradictions,
      faultToTerminalMs: observation.faultToTerminalMs,
      boundMs: observation.boundMs,
      settlementState: observation.settlement?.state ?? null,
      receiptPath,
    })}\n`,
  );
  if (observation.outcome === "contradicted") {
    // Written, reported, and failed. The receipt is the point; the nonzero exit
    // keeps a contradicted drill from being scripted over as a success.
    process.exitCode = 1;
  }
};

try {
  await run();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  usage();
  process.exitCode = 1;
} finally {
  await dispose();
}
