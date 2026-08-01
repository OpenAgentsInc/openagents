#!/usr/bin/env node

import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { dispose } from "@livekit/rtc-node";
import {
  SARAH_LIVEKIT_REMAINING_DRILL_SCENARIOS,
  buildSarahLiveKitRemainingDrillReceipt,
  runSarahLiveKitRemainingDrill,
  type SarahLiveKitAuthoritySnapshot,
  type SarahLiveKitRemainingDrillScenario,
} from "./remaining-drill-driver.js";
import type { SarahLiveKitAcceptanceScenario } from "./acceptance-harness.js";

const OWNER_GATE = "I_ACCEPT_EP263_SARAH_REMAINING_DRILLS";
const PROVIDER_OWNER_GATE = "I_ACCEPT_EXACT_SARAH_PROVIDER_DISCONNECT";
const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const receiptRoot = resolve(repositoryRoot, "docs/ops/receipts/livekit");

type Arguments = Readonly<{
  apply: boolean;
  scenario?: string;
  room?: string;
  pcm?: string;
  sourceRevision?: string;
  workerImageDigest?: string;
  observationWindowMs?: string;
  privateOutput?: string;
  receipt?: string;
  communityRef?: string;
  channelRef?: string;
}>;

const FIELDS = new Map<string, keyof Omit<Arguments, "apply">>([
  ["--scenario", "scenario"],
  ["--room", "room"],
  ["--pcm", "pcm"],
  ["--source-revision", "sourceRevision"],
  ["--worker-image-digest", "workerImageDigest"],
  ["--observation-window-ms", "observationWindowMs"],
  ["--private-output", "privateOutput"],
  ["--receipt", "receipt"],
  ["--community-ref", "communityRef"],
  ["--channel-ref", "channelRef"],
]);

const usage = () => {
  process.stderr.write(`Usage:
  pnpm --dir apps/sarah-livekit-agent remaining-drill -- \\
    --scenario ${SARAH_LIVEKIT_REMAINING_DRILL_SCENARIOS.join("|")} \\
    --room private|community [--community-ref <ref> --channel-ref <ref>] \\
    --pcm <24kHz mono s16le prompt outside repository> \\
    --source-revision <40 lowercase hex> \\
    --worker-image-digest sha256:<64 lowercase hex> \\
    --observation-window-ms <positive integer> \\
    --private-output <path outside repository> \\
    --receipt docs/ops/receipts/livekit/<name>.json --apply

Dry-run is the default. A live run requires:
  OA_SARAH_LIVEKIT_REMAINING_DRILL_OWNER_GATE=${OWNER_GATE}
  OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_BEARER / _PRIVATE_OWNER_REF, or
  OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_BEARER / _COMMUNITY_OWNER_REF
  standard libpq variables for production's read-only authority connection

provider_disconnect additionally requires OPENAGENTS_ADMIN_BEARER in process
memory and an already armed bounded acceptance revision. The command sends the
bearer only to the exact HTTPS API origin and never writes or prints it.

hold_exhaustion is retired as not_applicable_removed. Owner-waived Sarah
admission creates reservedMsat=0 and never mutates balance or held credit, so
there is no hold to exhaust and this driver cannot select that scenario.

reconnect closes and settles the old generation before admitting a distinct,
strictly later generation. It then rereads the old authority and refuses any
post-terminal activity, overlap, or revival.
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

const required = (value: string | undefined, name: string): string => {
  const trimmed = value?.trim();
  if (trimmed === undefined || trimmed === "") throw new Error(`${name} is required`);
  return trimmed;
};

const environment = (name: string): string => required(process.env[name], name);

const positiveInteger = (value: string | undefined, name: string): number => {
  const parsed = Number(required(value, name));
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`${name} is invalid`);
  return parsed;
};

const scenarioArgument = (value: string | undefined): SarahLiveKitRemainingDrillScenario => {
  const found = SARAH_LIVEKIT_REMAINING_DRILL_SCENARIOS.find((scenario) => scenario === value);
  if (found === undefined) {
    throw new Error(`--scenario must be ${SARAH_LIVEKIT_REMAINING_DRILL_SCENARIOS.join(", ")}`);
  }
  return found;
};

const sqlLiteral = (value: string): string => {
  if (
    value.length < 1 ||
    value.length > 256 ||
    [...value].some((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint < 32 || codePoint === 127;
    })
  ) {
    throw new Error("authority session ref is invalid");
  }
  return `'${value.replaceAll("'", "''")}'`;
};

const authoritySql = (sessionRef: string): string => `
BEGIN TRANSACTION READ ONLY;
WITH target AS (
  SELECT
    session.session_ref,
    session.generation,
    session.state,
    session.close_reason,
    session.created_at,
    COALESCE(session.settled_at, binding.worker_closed_at) AS terminal_at,
    session.reservation_ref,
    session.settlement_receipt_ref,
    binding.worker_job_ref,
    binding.provider_session_ref_digest,
    session.reserved_msat,
    session.charged_msat,
    session.input_tokens,
    session.output_tokens,
    session.cached_input_tokens,
    session.audio_input_tokens,
    session.audio_output_tokens
  FROM sarah_realtime_voice_sessions AS session
  LEFT JOIN sarah_livekit_room_bindings AS binding
    ON binding.session_ref = session.session_ref
  WHERE session.session_ref = ${sqlLiteral(sessionRef)}
), event_summary AS (
  SELECT
    COUNT(*) FILTER (WHERE event_kind = 'response_usage') AS response_count,
    COUNT(*) FILTER (WHERE event_kind = 'transcription_usage') AS transcription_count,
    (
      SELECT COUNT(*)
      FROM sarah_realtime_voice_usage
      WHERE session_ref = ${sqlLiteral(sessionRef)}
        AND usage_kind = 'response'
        AND provider_status = 'cancelled'
    ) AS cancelled_response_count,
    COUNT(*) FILTER (WHERE event_kind = 'close') AS terminal_event_count,
    COUNT(DISTINCT worker_job_ref) AS worker_job_count,
    COUNT(DISTINCT event_ref) FILTER (WHERE event_kind = 'provider_admitted') AS provider_session_count,
    MAX(observed_at::timestamptz) FILTER (WHERE event_kind = 'close') AS close_observed_at
  FROM sarah_livekit_worker_events
  WHERE session_ref = ${sqlLiteral(sessionRef)}
), after_terminal AS (
  SELECT COUNT(*) AS activity_count
  FROM sarah_livekit_worker_events, event_summary
  WHERE session_ref = ${sqlLiteral(sessionRef)}
    AND event_summary.close_observed_at IS NOT NULL
    AND observed_at::timestamptz > event_summary.close_observed_at
    AND event_kind <> 'close'
), disconnect AS (
  SELECT request_ref, applied_at IS NOT NULL AS applied
  FROM sarah_livekit_provider_disconnect_faults
  WHERE session_ref = ${sqlLiteral(sessionRef)}
)
SELECT json_build_object(
  'sessionRef', target.session_ref,
  'generation', target.generation,
  'state', target.state,
  'closeReason', target.close_reason,
  'startedAtMs', FLOOR(EXTRACT(EPOCH FROM target.created_at::timestamptz) * 1000)::bigint,
  'terminalAtMs', CASE WHEN target.terminal_at IS NULL THEN NULL
    ELSE FLOOR(EXTRACT(EPOCH FROM target.terminal_at::timestamptz) * 1000)::bigint END,
  'reservationRef', target.reservation_ref,
  'settlementReceiptRef', target.settlement_receipt_ref,
  'workerJobRef', target.worker_job_ref,
  'providerSessionRefDigest', target.provider_session_ref_digest,
  'reservedMsat', target.reserved_msat,
  'chargedMsat', target.charged_msat,
  'inputTokens', target.input_tokens,
  'outputTokens', target.output_tokens,
  'cachedInputTokens', target.cached_input_tokens,
  'audioInputTokens', target.audio_input_tokens,
  'audioOutputTokens', target.audio_output_tokens,
  'responseCount', COALESCE(event_summary.response_count, 0),
  'transcriptionCount', COALESCE(event_summary.transcription_count, 0),
  'cancelledResponseCount', COALESCE(event_summary.cancelled_response_count, 0),
  'terminalEventCount', COALESCE(event_summary.terminal_event_count, 0),
  'workerJobCount', COALESCE(event_summary.worker_job_count, 0),
  'providerSessionCount', GREATEST(
    CASE WHEN target.provider_session_ref_digest IS NULL THEN 0 ELSE 1 END,
    COALESCE(event_summary.provider_session_count, 0)
  ),
  'activityAfterTerminalCount', COALESCE(after_terminal.activity_count, 0),
  'providerDisconnectApplied', COALESCE(disconnect.applied, false),
  'providerDisconnectRequestRef', disconnect.request_ref
)
FROM target
CROSS JOIN event_summary
CROSS JOIN after_terminal
LEFT JOIN disconnect ON true;
COMMIT;
`;

const runPsql = (sql: string): Promise<string> =>
  new Promise((resolveOutput, reject) => {
    const child = spawn("psql", ["-X", "-q", "-A", "-t", "-v", "ON_ERROR_STOP=1"], {
      stdio: ["pipe", "pipe", "pipe"],
    });
    const output: Buffer[] = [];
    const errors: Buffer[] = [];
    child.stdout.on("data", (chunk: Buffer) => output.push(chunk));
    child.stderr.on("data", (chunk: Buffer) => errors.push(chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) resolveOutput(Buffer.concat(output).toString("utf8").trim());
      else
        reject(
          new Error(
            `psql authority read failed (${Buffer.concat(errors).toString("utf8").trim()})`,
          ),
        );
    });
    child.stdin.end(sql);
  });

const readAuthority = async (sessionRef: string): Promise<SarahLiveKitAuthoritySnapshot | null> => {
  const output = await runPsql(authoritySql(sessionRef));
  if (output === "") return null;
  let value: unknown;
  try {
    value = JSON.parse(output);
  } catch {
    throw new Error("psql authority read returned invalid JSON");
  }
  return value as SarahLiveKitAuthoritySnapshot;
};

const requestProviderDisconnect = async (
  input: Readonly<{
    requestRef: string;
    sessionRef: string;
    generation: number;
    providerSessionRefDigest: string;
  }>,
) => {
  const origin = new URL(process.env.OPENAGENTS_API_ORIGIN?.trim() || "https://openagents.com");
  if (origin.protocol !== "https:" || origin.username !== "" || origin.password !== "") {
    throw new Error("OPENAGENTS_API_ORIGIN must be an HTTPS origin without credentials");
  }
  const response = await fetch(new URL("/api/operator/sarah/livekit/provider-disconnect", origin), {
    method: "POST",
    headers: {
      authorization: `Bearer ${environment("OPENAGENTS_ADMIN_BEARER")}`,
      "content-type": "application/json",
      "x-openagents-livekit-owner-gate": PROVIDER_OWNER_GATE,
    },
    body: JSON.stringify({
      schema: "openagents.sarah.livekit-provider-disconnect-acceptance.v1",
      ...input,
      acknowledgement: "disconnect_exact_provider_socket",
    }),
  });
  if (!response.ok) {
    throw new Error(`provider-disconnect acceptance returned HTTP ${response.status}`);
  }
  const body = (await response.json()) as Record<string, unknown>;
  if (
    body.requestRef !== input.requestRef ||
    body.sessionRef !== input.sessionRef ||
    body.generation !== input.generation ||
    body.providerSessionRefDigest !== input.providerSessionRefDigest ||
    (body.state !== "requested" && body.state !== "applied") ||
    typeof body.replayed !== "boolean" ||
    body.sharedInfrastructureMutated !== false
  ) {
    throw new Error("provider-disconnect acceptance returned an invalid exact-generation receipt");
  }
  return body as {
    requestRef: string;
    sessionRef: string;
    generation: number;
    providerSessionRefDigest: string;
    state: "requested" | "applied";
    replayed: boolean;
    sharedInfrastructureMutated: false;
  };
};

const run = async () => {
  const args = parseArguments(process.argv.slice(2));
  if (!args.apply) {
    process.stdout.write(
      `${JSON.stringify({
        schema: "openagents.sarah.livekit-remaining-drill-dry-run.v1",
        environment: "production",
        scenarios: SARAH_LIVEKIT_REMAINING_DRILL_SCENARIOS,
        liveSessionOpened: false,
        providerDisconnectRequested: false,
        creditMutated: false,
        receiptWritten: false,
        requiredOwnerGate: OWNER_GATE,
      })}\n`,
    );
    return;
  }
  if (process.env.OA_SARAH_LIVEKIT_REMAINING_DRILL_OWNER_GATE !== OWNER_GATE) {
    throw new Error(`--apply requires OA_SARAH_LIVEKIT_REMAINING_DRILL_OWNER_GATE=${OWNER_GATE}`);
  }

  const scenario = scenarioArgument(args.scenario);
  const room = required(args.room, "--room");
  if (room !== "private" && room !== "community") throw new Error("--room is invalid");
  const pcmPath = resolve(required(args.pcm, "--pcm"));
  if (pcmPath.startsWith(`${repositoryRoot}/`)) {
    throw new Error("remaining drill PCM must stay outside the repository");
  }
  const privateOutput = resolve(required(args.privateOutput, "--private-output"));
  if (privateOutput === repositoryRoot || privateOutput.startsWith(`${repositoryRoot}/`)) {
    throw new Error("private remaining-drill observation must stay outside the repository");
  }
  const receipt = resolve(repositoryRoot, required(args.receipt, "--receipt"));
  if (!receipt.startsWith(`${receiptRoot}/`)) {
    throw new Error("remaining-drill public receipt must stay under docs/ops/receipts/livekit");
  }
  const pcm = await readFile(pcmPath);
  const suffix = room === "private" ? "PRIVATE" : "COMMUNITY";
  const sessionRef = `remaining-drill-${scenario}-${randomUUID()}`;
  const scenarioInput: SarahLiveKitAcceptanceScenario = {
    kind: room,
    bearer: environment(`OA_SARAH_LIVEKIT_ACCEPTANCE_${suffix}_BEARER`),
    subscriberRef: `drill-${room}-subscriber-${randomUUID()}`,
    ownerRef: environment(`OA_SARAH_LIVEKIT_ACCEPTANCE_${suffix}_OWNER_REF`),
    deviceRef: `remaining-drill-device-${randomUUID()}`,
    threadRef: `remaining-drill-thread-${randomUUID()}`,
    sessionRef,
    generation: 1,
    pcm,
    roomContext:
      room === "private"
        ? ({ kind: "private" } as const)
        : ({
            kind: "community" as const,
            communityRef: required(args.communityRef, "--community-ref"),
            channelRef: required(args.channelRef, "--channel-ref"),
          } as const),
  };
  const freshSession =
    scenario === "reconnect"
      ? {
          ...scenarioInput,
          subscriberRef: `drill-${room}-subscriber-${randomUUID()}`,
          sessionRef: `remaining-drill-reconnect-fresh-${randomUUID()}`,
          generation: 2,
        }
      : undefined;

  const observation = await runSarahLiveKitRemainingDrill(
    {
      scenario,
      session: scenarioInput,
      ...(freshSession === undefined ? {} : { freshSession }),
      sourceRevision: required(args.sourceRevision, "--source-revision"),
      workerImageDigest: required(args.workerImageDigest, "--worker-image-digest"),
      observationWindowMs: positiveInteger(args.observationWindowMs, "--observation-window-ms"),
    },
    {
      readAuthority,
      ...(scenario === "provider_disconnect" ? { requestProviderDisconnect } : {}),
    },
  );
  const publicReceipt = buildSarahLiveKitRemainingDrillReceipt(observation);
  await mkdir(dirname(privateOutput), { recursive: true });
  await writeFile(privateOutput, `${JSON.stringify(observation, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  await mkdir(dirname(receipt), { recursive: true });
  await writeFile(receipt, `${JSON.stringify(publicReceipt, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  process.stdout.write(
    `${JSON.stringify({
      scenario,
      outcome: publicReceipt.outcome,
      resultDigest: publicReceipt.resultDigest,
      privateOutput,
      receipt,
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
