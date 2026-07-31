#!/usr/bin/env node

/**
 * Operator entry point for recording a Sarah LiveKit release-gate journey.
 *
 * The human-only gate rows cannot be produced by any automated run, so this CLI
 * is the path by which an operator's three-desktop journey, forced-transport
 * matrix, and failure drills become bound evidence instead of a memory.
 *
 * The intended sequence is:
 *
 *   1. `--row <id> --template <file>` writes a skeleton with every observation
 *      the row requires, each pre-filled as `not_observed`. Nothing can be
 *      forgotten, because the recorder rejects a missing key outright.
 *   2. Run the journey. Fill in each observation you actually made.
 *   3. `--row <id> --observations <file> --receipt <path> --apply` validates the
 *      filled skeleton and writes a content-addressed receipt.
 *
 * The receipt preserves what was observed. It does not admit the row.
 */

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  GATE_ROW_IDS,
  REQUIRED_OBSERVATIONS,
  recordSarahLiveKitGateObservation,
  type GateBinding,
  type GateObservation,
  type GateRowId,
} from "./gate-observation.js";

const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";
const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const receiptRoot = resolve(repositoryRoot, "docs/ops/receipts/livekit/gate");

type Arguments = Readonly<{
  apply: boolean;
  row?: string;
  template?: string;
  observations?: string;
  binding?: string;
  operatorRef?: string;
  receipt?: string;
}>;

const usage = () => {
  process.stderr.write(`Usage:
  # 1. Emit a skeleton naming every observation the row requires.
  pnpm --dir apps/sarah-livekit-agent gate-observation -- \\
    --row <${GATE_ROW_IDS.join("|")}> \\
    --template /tmp/<row>-observations.json

  # 2. Run the journey and fill in the skeleton.

  # 3. Record it as a bound, content-addressed receipt.
  pnpm --dir apps/sarah-livekit-agent gate-observation -- \\
    --row <row> \\
    --observations /tmp/<row>-observations.json \\
    --binding /tmp/<candidate>-binding.json \\
    --operator-ref <operator identity, digested before recording> \\
    --receipt docs/ops/receipts/livekit/gate/<name>.json --apply

Listing a row with no other argument prints the observations it requires.

A live record requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}.
Observation and binding inputs must live outside the repository so a draft
cannot be committed by accident. The operator ref is digested before it is
recorded and never appears in the receipt.
`);
};

const parseArguments = (values: readonly string[]): Arguments => {
  const parsed: {
    apply: boolean;
    row?: string;
    template?: string;
    observations?: string;
    binding?: string;
    operatorRef?: string;
    receipt?: string;
  } = { apply: false };
  const fields = new Map<string, keyof Omit<typeof parsed, "apply">>([
    ["--row", "row"],
    ["--template", "template"],
    ["--observations", "observations"],
    ["--binding", "binding"],
    ["--operator-ref", "operatorRef"],
    ["--receipt", "receipt"],
  ] as const);
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    // `pnpm --dir apps/sarah-livekit-agent gate-observation -- ...`, the
    // invocation this file's own usage block and the runbook both document,
    // forwards the `--` separator. Rejecting it meant the recorder could not be
    // reached by its documented command at all — which is why
    // docs/ops/receipts/livekit/gate/ never existed. `failure-matrix-cli.ts`
    // has always skipped it; this one and `acceptance-cli.ts` did not.
    if (value === "--") continue;
    if (value === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (value === "--help" || value === "-h") {
      usage();
      process.exit(0);
    }
    const field = fields.get(value ?? "");
    const next = values[index + 1];
    if (field === undefined || next === undefined || next.startsWith("--")) {
      throw new Error(`unsupported or incomplete argument ${value}`);
    }
    parsed[field] = next;
    index += 1;
  }
  return parsed;
};

const requiredRow = (value: string | undefined): GateRowId => {
  if (value === undefined || !GATE_ROW_IDS.includes(value as GateRowId)) {
    throw new Error(`--row must be one of ${GATE_ROW_IDS.join(", ")}`);
  }
  return value as GateRowId;
};

const requiredArgument = (value: string | undefined, name: string): string => {
  if (value === undefined || value.trim() === "") throw new Error(`${name} is required`);
  return value;
};

/** Read an operator input, refusing any path inside the repository. */
const readExternalJson = async (path: string, name: string): Promise<unknown> => {
  const resolved = resolve(path);
  if (resolved.startsWith(`${repositoryRoot}/`)) {
    throw new Error(`${name} must live outside the repository`);
  }
  return JSON.parse(await readFile(resolved, "utf8")) as unknown;
};

/**
 * A skeleton with every required observation pre-filled as unobserved.
 *
 * Handing the operator the complete list up front is the difference between an
 * evidence gap that is discovered now and one discovered after the journey.
 */
const template = (row: GateRowId): readonly GateObservation[] =>
  REQUIRED_OBSERVATIONS[row].map((key) => ({
    key,
    finding: "not_observed" as const,
    reason: "REPLACE: say why this was not observed, or replace this whole entry",
    observedAtMs: 0,
  }));

const run = async () => {
  const args = parseArguments(process.argv.slice(2));
  const row = requiredRow(args.row);

  if (args.template !== undefined) {
    const resolved = resolve(args.template);
    if (resolved.startsWith(`${repositoryRoot}/`)) {
      throw new Error("--template must be written outside the repository");
    }
    await mkdir(dirname(resolved), { recursive: true });
    await writeFile(resolved, `${JSON.stringify(template(row), null, 2)}\n`, {
      encoding: "utf8",
      mode: 0o600,
    });
    process.stdout.write(
      `${JSON.stringify({ row, templatePath: resolved, required: REQUIRED_OBSERVATIONS[row] })}\n`,
    );
    return;
  }

  if (!args.apply) {
    process.stdout.write(
      `${JSON.stringify(
        {
          schema: "openagents.sarah.livekit-gate-observation.dry-run.v1",
          row,
          receiptWritten: false,
          requiredObservations: REQUIRED_OBSERVATIONS[row],
          note:
            "Every observation listed must be recorded explicitly. One that was not made must " +
            'carry finding "not_observed" and a reason; it cannot simply be omitted.',
        },
        null,
        2,
      )}\n`,
    );
    return;
  }

  if (process.env.OA_LIVEKIT_OWNER_GATE !== OWNER_GATE) {
    throw new Error(`--apply requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}`);
  }

  const observations = (await readExternalJson(
    requiredArgument(args.observations, "--observations"),
    "--observations",
  )) as readonly GateObservation[];
  if (!Array.isArray(observations)) {
    throw new Error("--observations must contain a JSON array of observations");
  }
  const binding = (await readExternalJson(
    requiredArgument(args.binding, "--binding"),
    "--binding",
  )) as GateBinding;

  const receiptPath = resolve(repositoryRoot, requiredArgument(args.receipt, "--receipt"));
  if (!receiptPath.startsWith(`${receiptRoot}/`)) {
    throw new Error("receipt path must be under docs/ops/receipts/livekit/gate");
  }

  // The raw operator ref is digested here and never reaches the receipt.
  const { digestGateRef } = await import("./gate-observation.js");
  const operatorRefDigest = digestGateRef(requiredArgument(args.operatorRef, "--operator-ref"));

  const receipt = recordSarahLiveKitGateObservation(
    { row, binding, operatorRefDigest, observations },
    { now: Date.now },
  );

  await mkdir(dirname(receiptPath), { recursive: true });
  await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  process.stdout.write(
    `${JSON.stringify({
      row: receipt.row,
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
