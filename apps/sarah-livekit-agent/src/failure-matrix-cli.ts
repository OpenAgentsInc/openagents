#!/usr/bin/env node

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  SARAH_LIVEKIT_FAILURE_SCENARIOS,
  buildSarahLiveKitFailureMatrixReceipt,
  validateSarahLiveKitFailureMatrixAuthorityRows,
  validateSarahLiveKitFailureMatrixObservation,
  type SarahLiveKitFailureMatrixObservation,
  type SarahLiveKitUnmeteredAuthorityCaptureRow,
} from "./failure-matrix.js";
import { canonicalFailureMatrixPaths } from "./failure-matrix-paths.js";

const OWNER_GATE = "I_ACCEPT_EP263_SARAH_FAILURE_MATRIX";
const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));

const requiredEnvironment = (name: string): string => {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
};

const runPsql = (statement: string): Promise<string> =>
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
      else reject(new Error(`psql authority read failed (${Buffer.concat(errors).toString("utf8").trim()})`));
    });
    child.stdin.end(statement);
  });

const readProductionAuthorityRows = async (
  observation: SarahLiveKitFailureMatrixObservation,
): Promise<readonly SarahLiveKitUnmeteredAuthorityCaptureRow[]> => {
  const receiptRefs = [
    ...observation.scenarios.map(
      (scenario) => scenario.unmeteredAuthorityCapture.captureReceiptRef,
    ),
    observation.retiredScenarios[0].unmeteredAuthorityCapture.captureReceiptRef,
  ];
  const literals = receiptRefs.map((reference) => `'${reference}'`).join(", ");
  const output = await runPsql(`
    SELECT json_build_object(
      'databaseName', current_database(),
      'rows', COALESCE(json_agg(json_build_object(
        'sessionRef', capture.session_ref,
        'authority', capture.authority,
        'generation', capture.generation,
        'startLedgerStateDigest', capture.start_ledger_state_digest,
        'endLedgerStateDigest', capture.end_ledger_state_digest,
        'startBalanceStateDigest', capture.start_balance_state_digest,
        'endBalanceStateDigest', capture.end_balance_state_digest,
        'ledgerMutationCount', capture.ledger_mutation_count::integer,
        'captureReceiptRef', capture.capture_receipt_ref,
        'captureDigest', capture.capture_digest,
        'sessionState', session.state,
        'creditMode', session.credit_mode,
        'providerAccountingStatus', binding.provider_accounting_status,
        'closeReason', session.close_reason,
        'reservedMsat', session.reserved_msat,
        'chargedMsat', session.charged_msat,
        'settlementReceiptRef', session.settlement_receipt_ref,
        'terminalAuthorityRef', capture.terminal_authority_ref,
        'inputTokens', usage.input_tokens,
        'outputTokens', usage.output_tokens,
        'cachedInputTokens', usage.cached_input_tokens,
        'audioInputTokens', usage.audio_input_tokens,
        'audioOutputTokens', usage.audio_output_tokens,
        'usageChargeMsat', usage.charge_msat,
        'responseCount', usage.response_count,
        'transcriptionCount', usage.transcription_count,
        'cancelledResponseCount', usage.cancelled_response_count
      ) ORDER BY capture.capture_receipt_ref)
        FILTER (WHERE capture.capture_receipt_ref IS NOT NULL), '[]'::json)
    )
    FROM sarah_voice_unmetered_authority_captures AS capture
    INNER JOIN sarah_realtime_voice_sessions AS session
      ON session.session_ref = capture.session_ref
    LEFT JOIN sarah_livekit_room_bindings AS binding
      ON binding.session_ref = capture.session_ref
      AND binding.generation = capture.generation
    LEFT JOIN LATERAL (
      SELECT COALESCE(SUM(input_tokens), 0) AS input_tokens,
        COALESCE(SUM(output_tokens), 0) AS output_tokens,
        COALESCE(SUM(cached_input_tokens), 0) AS cached_input_tokens,
        COALESCE(SUM(audio_input_tokens), 0) AS audio_input_tokens,
        COALESCE(SUM(audio_output_tokens), 0) AS audio_output_tokens,
        COALESCE(SUM(charge_msat), 0) AS charge_msat,
        COUNT(*) FILTER (WHERE usage_kind = 'response') AS response_count,
        COUNT(*) FILTER (WHERE usage_kind = 'transcription') AS transcription_count,
        COUNT(*) FILTER (
          WHERE usage_kind = 'response' AND provider_status = 'cancelled'
        ) AS cancelled_response_count
      FROM sarah_realtime_voice_usage
      WHERE session_ref = capture.session_ref
    ) AS usage ON true
    WHERE capture.capture_receipt_ref IN (${literals});
  `);
  let parsed: unknown;
  try {
    parsed = JSON.parse(output);
  } catch {
    throw new Error("psql authority read returned invalid JSON");
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error("psql authority read returned an invalid result");
  }
  const result = parsed as { databaseName?: unknown; rows?: unknown };
  if (
    result.databaseName !== requiredEnvironment("SARAH_FAILURE_MATRIX_EXPECTED_PRODUCTION_DATABASE")
  ) {
    throw new Error("refusing Sarah failure-matrix authority read against the wrong database");
  }
  if (!Array.isArray(result.rows)) throw new Error("psql authority read returned invalid rows");
  return result.rows as readonly SarahLiveKitUnmeteredAuthorityCaptureRow[];
};

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
one public-safe receipt after a read-only production database authority check.
It performs no fault injection, mutation, provider disconnect, or other live or
destructive action. It requires:
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
  const { inputPath, receiptPath } = canonicalFailureMatrixPaths(
    repositoryRoot,
    args.input as string,
    args.receipt as string,
  );
  const observation = JSON.parse(
    await readFile(inputPath, "utf8"),
  ) as SarahLiveKitFailureMatrixObservation;
  validateSarahLiveKitFailureMatrixObservation(observation);
  const observedAtMs = Date.parse(observation.observedAt);
  const observationAgeMs = Date.now() - observedAtMs;
  if (
    !Number.isFinite(observedAtMs) ||
    observationAgeMs < -5 * 60_000 ||
    observationAgeMs > 24 * 60 * 60_000
  ) {
    throw new Error("private failure-matrix observation must be from the last 24 hours");
  }
  const authorityRows = await readProductionAuthorityRows(observation);
  validateSarahLiveKitFailureMatrixAuthorityRows(observation, authorityRows);
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
