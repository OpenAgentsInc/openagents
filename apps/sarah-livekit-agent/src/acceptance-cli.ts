#!/usr/bin/env node

import { randomUUID } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { dispose } from "@livekit/rtc-node";
import { runSarahLiveKitAcceptance } from "./acceptance-harness.js";
import { runLiveSarahLiveKitScenario } from "./acceptance-livekit.js";
import { resolveDeployedSarahRevision } from "./acceptance-deployment.js";
import {
  FORCED_TRANSPORT_PROFILES,
  type ForcedTransportProfile,
} from "./ice-classification.js";

const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";
const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const receiptRoot = resolve(repositoryRoot, "docs/ops/receipts/livekit");

type Arguments = Readonly<{
  apply: boolean;
  privatePcm?: string;
  communityPcm?: string;
  communityRef?: string;
  channelRef?: string;
  forcedTransport?: string;
  receipt?: string;
}>;

const usage = () => {
  process.stderr.write(`Usage:
  pnpm --dir apps/sarah-livekit-agent acceptance -- \\
    --private-pcm <private 24kHz mono s16le prompt> \\
    --community-pcm <private 24kHz mono s16le prompt> \\
    --community-ref <community ref> --channel-ref <channel ref> \\
    [--forced-transport ${FORCED_TRANSPORT_PROFILES.join("|")}] \\
    --receipt docs/ops/receipts/livekit/<name>.json --apply

--forced-transport declares the network constraint the operator imposed on this
capture and defaults to unrestricted. The harness cannot discover an imposed
constraint, so it checks the declaration against the classified ICE paths and
refuses a capture that contradicts it.

Dry-run is the default and performs no network request. A live run targets only
https://openagents.com and wss://livekit.openagents.com. It requires:
  OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}
  OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_BEARER
  OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_OWNER_REF
  OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_BEARER
  OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_OWNER_REF

The two owner refs must differ because production accounting admits one active
Sarah voice generation per owner. PCM and transcript content are consumed in
memory and never written to the receipt.
`);
};

const parseArguments = (values: readonly string[]): Arguments => {
  const parsed: {
    apply: boolean;
    privatePcm?: string;
    communityPcm?: string;
    communityRef?: string;
    channelRef?: string;
    forcedTransport?: string;
    receipt?: string;
  } = { apply: false };
  const fields = new Map<string, keyof Omit<typeof parsed, "apply">>([
    ["--private-pcm", "privatePcm"],
    ["--community-pcm", "communityPcm"],
    ["--community-ref", "communityRef"],
    ["--channel-ref", "channelRef"],
    ["--forced-transport", "forcedTransport"],
    ["--receipt", "receipt"],
  ] as const);
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    // `pnpm --dir apps/sarah-livekit-agent acceptance -- ...`, the invocation
    // this repository's runbook documents, forwards the `--` separator itself.
    // Rejecting it made the documented command fail before any argument was
    // read. `failure-matrix-cli.ts` has always skipped it; this one did not.
    if (value === "--") continue;
    if (value === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (value === "--help" || value === "-h") {
      usage();
      process.exit(0);
    }
    const field = fields.get(value);
    const next = values[index + 1];
    if (field === undefined || next === undefined || next.startsWith("--")) {
      throw new Error(`unsupported or incomplete argument ${value}`);
    }
    parsed[field] = next;
    index += 1;
  }
  return parsed;
};

const requiredEnvironment = (name: string): string => {
  const value = process.env[name]?.trim();
  if (value === undefined || value === "") throw new Error(`${name} is required`);
  return value;
};

const requiredArgument = (value: string | undefined, name: string): string => {
  if (value === undefined || value.trim() === "")
    throw new Error(`${name} is required with --apply`);
  return value;
};

const forcedTransportProfile = (value: string | undefined): ForcedTransportProfile => {
  const declared = value?.trim() ?? "unrestricted";
  const profile = FORCED_TRANSPORT_PROFILES.find((known) => known === declared);
  if (profile === undefined) {
    throw new Error(`--forced-transport must be one of ${FORCED_TRANSPORT_PROFILES.join(", ")}`);
  }
  return profile;
};

const run = async () => {
  const args = parseArguments(process.argv.slice(2));
  const declaredForcedTransport = forcedTransportProfile(args.forcedTransport);
  if (!args.apply) {
    process.stdout.write(
      `${JSON.stringify({
        schema: "openagents.sarah.livekit-acceptance.dry-run.v1",
        environment: "production",
        liveProbeExecuted: false,
        receiptWritten: false,
        requiredScenarios: ["private", "community"],
        forcedTransportProfile: declaredForcedTransport,
        forcedTransportProfileAuthority: "operator_declared_checked_against_classified_paths",
        requiredObservations: [
          "authenticated admission and livekit_room_v1 session",
          "room-scoped participant grant",
          "ticketed gateway control-channel readiness",
          "microphone publication",
          "Sarah audio and ephemeral transcription",
          "explicit interrupt acknowledgement and interrupted lifecycle",
          "bounded post-interrupt Sarah audio tail",
          "at least one exactly accounted cancelled provider response",
          "literal principal.sarah and distinct authority identity digests",
          "generation-bearing identity digests distinct across the two rooms",
          "salted comparable identity digests for independent re-verification",
          "nonzero exact provider usage equal to terminal settlement",
          "audible Sarah output for owner and secondary subscriber",
          "classified selected publisher and subscriber ICE paths",
          "classified paths consistent with the declared forced transport profile",
          "overlapping private and community sessions",
          "terminal settlement receipts",
        ],
      })}\n`,
    );
    return;
  }
  if (process.env.OA_LIVEKIT_OWNER_GATE !== OWNER_GATE) {
    throw new Error(`--apply requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}`);
  }

  const sourceRevision = await resolveDeployedSarahRevision();
  const privatePcmPath = resolve(requiredArgument(args.privatePcm, "--private-pcm"));
  const communityPcmPath = resolve(requiredArgument(args.communityPcm, "--community-pcm"));
  if (
    privatePcmPath.startsWith(`${repositoryRoot}/`) ||
    communityPcmPath.startsWith(`${repositoryRoot}/`)
  ) {
    throw new Error("acceptance PCM prompts must remain outside the repository");
  }
  const communityRef = requiredArgument(args.communityRef, "--community-ref");
  const channelRef = requiredArgument(args.channelRef, "--channel-ref");
  const receiptPath = resolve(repositoryRoot, requiredArgument(args.receipt, "--receipt"));
  if (!receiptPath.startsWith(`${receiptRoot}/`)) {
    throw new Error("receipt path must be under docs/ops/receipts/livekit");
  }

  const privateOwnerRef = requiredEnvironment("OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_OWNER_REF");
  const communityOwnerRef = requiredEnvironment("OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_OWNER_REF");
  const runRef = randomUUID();
  const [privatePcm, communityPcm] = await Promise.all([
    readFile(privatePcmPath),
    readFile(communityPcmPath),
  ]);
  const receipt = await runSarahLiveKitAcceptance(
    {
      sourceRevision,
      forcedTransportProfile: declaredForcedTransport,
      privateScenario: {
        kind: "private",
        bearer: requiredEnvironment("OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_BEARER"),
        subscriberRef: `acceptance-private-subscriber-${runRef}`,
        ownerRef: privateOwnerRef,
        deviceRef: `acceptance-private-${runRef}`,
        threadRef: `acceptance-private-${runRef}`,
        sessionRef: `acceptance-private-${runRef}`,
        generation: 1,
        pcm: privatePcm,
        roomContext: { kind: "private" },
      },
      communityScenario: {
        kind: "community",
        bearer: requiredEnvironment("OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_BEARER"),
        subscriberRef: `acceptance-community-subscriber-${runRef}`,
        ownerRef: communityOwnerRef,
        deviceRef: `acceptance-community-${runRef}`,
        threadRef: `acceptance-community-${runRef}`,
        sessionRef: `acceptance-community-${runRef}`,
        generation: 1,
        pcm: communityPcm,
        roomContext: { kind: "community", communityRef, channelRef },
      },
    },
    { now: Date.now, runScenario: runLiveSarahLiveKitScenario },
  );

  await mkdir(dirname(receiptPath), { recursive: true });
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  process.stdout.write(
    `${JSON.stringify({
      outcome: receipt.outcome,
      forcedTransportProfile: receipt.forcedTransportProfile,
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
} finally {
  await dispose();
}
