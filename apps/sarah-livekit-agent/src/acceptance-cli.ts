#!/usr/bin/env node

import { randomUUID } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runSarahLiveKitAcceptance } from "./acceptance-harness.js";
import { runLiveSarahLiveKitScenario } from "./acceptance-livekit.js";
import { resolveDeployedSarahRevision } from "./acceptance-deployment.js";

const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";
const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const receiptRoot = resolve(repositoryRoot, "docs/ops/receipts/livekit");

type Arguments = Readonly<{
  apply: boolean;
  privatePcm?: string;
  communityPcm?: string;
  communityRef?: string;
  channelRef?: string;
  receipt?: string;
}>;

const usage = () => {
  process.stderr.write(`Usage:
  pnpm --dir apps/sarah-livekit-agent acceptance -- \\
    --private-pcm <private 24kHz mono s16le prompt> \\
    --community-pcm <private 24kHz mono s16le prompt> \\
    --community-ref <community ref> --channel-ref <channel ref> \\
    --receipt docs/ops/receipts/livekit/<name>.json --apply

Dry-run is the default and performs no network request. A live run targets only
https://openagents.com and wss://livekit.openagents.com. It requires:
  OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}
  OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_BEARER
  OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_OWNER_REF
  OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_SUBSCRIBER_GRANT
  OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_SUBSCRIBER_REF
  OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_BEARER
  OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_OWNER_REF
  OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_SUBSCRIBER_GRANT
  OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_SUBSCRIBER_REF

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
    receipt?: string;
  } = { apply: false };
  const fields = new Map<string, keyof Omit<typeof parsed, "apply">>([
    ["--private-pcm", "privatePcm"],
    ["--community-pcm", "communityPcm"],
    ["--community-ref", "communityRef"],
    ["--channel-ref", "channelRef"],
    ["--receipt", "receipt"],
  ] as const);
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
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

const run = async () => {
  const args = parseArguments(process.argv.slice(2));
  if (!args.apply) {
    process.stdout.write(
      `${JSON.stringify({
        schema: "openagents.sarah.livekit-acceptance.dry-run.v1",
        environment: "production",
        liveProbeExecuted: false,
        receiptWritten: false,
        requiredScenarios: ["private", "community"],
        requiredObservations: [
          "authenticated admission and livekit_room_v1 session",
          "room-scoped participant grant",
          "microphone publication",
          "Sarah audio and ephemeral transcription",
          "literal principal.sarah and distinct authority identity digests",
          "nonzero exact provider usage equal to terminal settlement",
          "audible Sarah output for owner and secondary subscriber",
          "selected publisher and subscriber ICE paths",
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
      privateScenario: {
        kind: "private",
        bearer: requiredEnvironment("OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_BEARER"),
        subscriberGrant: requiredEnvironment(
          "OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_SUBSCRIBER_GRANT",
        ),
        subscriberRef: requiredEnvironment("OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_SUBSCRIBER_REF"),
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
        subscriberGrant: requiredEnvironment(
          "OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_SUBSCRIBER_GRANT",
        ),
        subscriberRef: requiredEnvironment("OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_SUBSCRIBER_REF"),
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
