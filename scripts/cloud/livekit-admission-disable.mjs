#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { assertPublicSafe, validateDeploymentBundle } from "./livekit-ops-policy.mjs";

const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";
const PROJECT = "openagentsgemini";
const REGION = "us-central1";
const SERVICE = "openagents-monolith";
const COMMIT = /^[0-9a-f]{40}$/u;
const COUNTS_QUERY = `
SELECT
  COUNT(*) FILTER (
    WHERE binding.state IN ('prepared', 'active')
      AND session.state IN ('reserved', 'connected')
  ) AS active_room_count,
  COUNT(*) FILTER (
    WHERE session.state IN ('reserved', 'connected')
      OR (
        session.state = 'accounting_uncertain'
        AND session.credit_mode <> 'owner_waived_unmetered'
      )
  ) AS pending_settlement_count
FROM sarah_livekit_room_bindings AS binding
INNER JOIN sarah_realtime_voice_sessions AS session USING (session_ref);
`;

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-admission-disable.mjs \\
    --bundle infra/livekit/bundle.json \\
    --deployed-revision <40-hex> \\
    --output <private-admission-disable-receipt.json> [--apply]

Default mode prints the exact read-only checks. --apply requires the LiveKit
owner gate, verifies that production Cloud Run serves admissions disabled, and
reads only aggregate LiveKit room/settlement counts through psql. It writes an
exclusive mode-0600 receipt outside the repository. Supply the database
connection through standard libpq PGHOST/PGPORT/PGUSER/PGPASSWORD/PGDATABASE
environment variables; credentials never enter argv, stdout, or the receipt.
`);
};

const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const readJson = (path, label) => {
  try {
    return JSON.parse(readFileSync(resolve(path), "utf8"));
  } catch (error) {
    throw new Error(`${label} is not readable JSON`, { cause: error });
  }
};

const parseArgs = (values) => {
  const parsed = { apply: false };
  for (let index = 0; index < values.length; index += 1) {
    const argument = values[index];
    if (argument === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    if (!["--bundle", "--deployed-revision", "--output"].includes(argument)) {
      throw new Error(`unsupported argument ${argument}`);
    }
    const value = values[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
    parsed[
      {
        "--bundle": "bundle",
        "--deployed-revision": "deployedRevision",
        "--output": "output",
      }[argument]
    ] = value;
    index += 1;
  }
  for (const field of ["bundle", "deployedRevision", "output"]) {
    if (!parsed[field]) throw new Error(`missing required argument ${field}`);
  }
  assert(COMMIT.test(parsed.deployedRevision), "deployed revision must be a full Git commit");
  return parsed;
};

const isWithin = (parent, candidate) => {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
};

const exactCommand = (bin, args, environment = process.env) =>
  spawnSync(bin, args, {
    encoding: "utf8",
    env: environment,
    maxBuffer: 1024 * 1024,
    shell: false,
    timeout: 60_000,
  });

const requireSuccess = (execution, label) => {
  if (execution.error) throw new Error(`${label} could not execute`, { cause: execution.error });
  if (execution.status !== 0)
    throw new Error(`${label} failed with exit status ${execution.status}`);
  return execution.stdout ?? "";
};

const latestServingRevision = (service) => {
  const conditions = service.status?.conditions ?? [];
  assert(
    conditions.some((condition) => condition.type === "Ready" && condition.status === "True"),
    "production Cloud Run service is not Ready",
  );
  const latestReadyRevisionName = service.status?.latestReadyRevisionName;
  assert(
    typeof latestReadyRevisionName === "string" && latestReadyRevisionName !== "",
    "production Cloud Run service has no latest Ready revision",
  );
  const traffic = service.status?.traffic ?? [];
  // Tagged revisions remain in this array without a traffic percentage. Only
  // entries with a positive percentage participate in the serving split.
  const servingTraffic = traffic.filter((entry) => (entry.percent ?? 0) > 0);
  assert(
    servingTraffic.length === 1 &&
      servingTraffic[0]?.revisionName === latestReadyRevisionName &&
      servingTraffic[0]?.percent === 100,
    "production Cloud Run traffic is not wholly on the latest Ready revision",
  );
  return latestReadyRevisionName;
};

const assertAdmissionDisabled = (revision, expectedRevisionName) => {
  assert(
    revision.metadata?.name === expectedRevisionName,
    "Cloud Run revision observation does not match the serving revision",
  );
  assert(
    (revision.status?.conditions ?? []).some(
      (condition) => condition.type === "Ready" && condition.status === "True",
    ),
    "serving Cloud Run revision is not Ready",
  );
  const containers = revision.spec?.containers;
  assert(Array.isArray(containers) && containers.length === 1, "Cloud Run container shape drifted");
  const environment = new Map((containers[0]?.env ?? []).map((entry) => [entry.name, entry.value]));
  assert(
    environment.get("SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED") === "false",
    "production LiveKit admission is not disabled",
  );
};

const drainedCounts = (value) => {
  const match = /^\s*(\d+)\s*,\s*(\d+)\s*$/u.exec(value);
  assert(match !== null, "LiveKit aggregate count query returned an invalid shape");
  const activeRoomCount = Number(match[1]);
  const pendingSettlementCount = Number(match[2]);
  assert(activeRoomCount === 0, "production still has active LiveKit rooms");
  assert(pendingSettlementCount === 0, "production still has pending LiveKit settlements");
  return { activeRoomCount, pendingSettlementCount };
};

export const collectAdmissionDisableReceipt = ({
  bundle,
  deployedRevision,
  runCommand = exactCommand,
  now = () => new Date().toISOString(),
}) => {
  const serviceExecution = runCommand("gcloud", [
    "run",
    "services",
    "describe",
    SERVICE,
    "--project",
    PROJECT,
    "--region",
    REGION,
    "--format=json",
  ]);
  let service;
  try {
    service = JSON.parse(requireSuccess(serviceExecution, "read production Cloud Run service"));
  } catch (error) {
    throw new Error("production Cloud Run service observation is invalid", { cause: error });
  }
  const latestReadyRevisionName = latestServingRevision(service);
  const revisionExecution = runCommand("gcloud", [
    "run",
    "revisions",
    "describe",
    latestReadyRevisionName,
    "--project",
    PROJECT,
    "--region",
    REGION,
    "--format=json",
  ]);
  let revision;
  try {
    revision = JSON.parse(
      requireSuccess(revisionExecution, "read serving production Cloud Run revision"),
    );
  } catch (error) {
    throw new Error("serving production Cloud Run revision observation is invalid", {
      cause: error,
    });
  }
  assertAdmissionDisabled(revision, latestReadyRevisionName);

  const databaseEnvironment = Object.fromEntries(
    ["PGHOST", "PGPORT", "PGUSER", "PGPASSWORD", "PGDATABASE"]
      .filter((name) => process.env[name] !== undefined)
      .map((name) => [name, process.env[name]]),
  );
  for (const name of ["PGHOST", "PGUSER", "PGPASSWORD", "PGDATABASE"]) {
    assert(
      databaseEnvironment[name] !== undefined,
      `${name} is required for the aggregate drain read`,
    );
  }
  const countExecution = runCommand(
    "psql",
    [
      "--no-psqlrc",
      "--no-align",
      "--tuples-only",
      "--field-separator=,",
      "--command",
      COUNTS_QUERY,
    ],
    { ...process.env, ...databaseEnvironment },
  );
  const counts = drainedCounts(requireSuccess(countExecution, "read LiveKit drain counts"));
  const receipt = {
    schemaVersion: "openagents.livekit_admission_disable.v1",
    stage: "production",
    sourceBaseRevision: bundle.sourceBaseRevision,
    deployedRevision,
    observedAt: now(),
    resourceRef: "livekit-admission-ref://production/livekit-room-v1",
    newAdmissionDisabled: true,
    newDispatchDisabled: true,
    ...counts,
  };
  assertPublicSafe(receipt);
  return receipt;
};

const run = () => {
  const args = parseArgs(process.argv.slice(2));
  const bundle = validateDeploymentBundle(readJson(args.bundle, "deployment bundle"));
  const repositoryRoot = resolve(import.meta.dirname, "../..");
  const output = resolve(args.output);
  assert(
    !isWithin(repositoryRoot, output),
    "private admission receipt must stay outside the repository",
  );
  if (!args.apply) {
    process.stdout.write(
      `${JSON.stringify(
        {
          mode: "dry-run",
          stage: "production",
          service: SERVICE,
          checks: [
            "latest Ready Cloud Run revision receives 100% traffic",
            "SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED is exactly false",
            "active LiveKit room count is zero",
            "pending LiveKit settlement count is zero",
          ],
          cloudMutationExecuted: false,
          databaseMutationExecuted: false,
          receiptWritten: false,
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
  const receipt = collectAdmissionDisableReceipt({
    bundle,
    deployedRevision: args.deployedRevision,
  });
  mkdirSync(dirname(output), { recursive: true, mode: 0o700 });
  writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  process.stdout.write(
    `${JSON.stringify({
      schemaVersion: receipt.schemaVersion,
      observedAt: receipt.observedAt,
      activeRoomCount: receipt.activeRoomCount,
      pendingSettlementCount: receipt.pendingSettlementCount,
      output,
    })}\n`,
  );
};

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  try {
    run();
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    usage();
    process.exitCode = 1;
  }
}
