#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { buildLiveKitCostCapture } from "./livekit-cost-collector-lib.mjs";

const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";
const PROJECT = "openagentsgemini";
const TABLE =
  /^[a-z][a-z0-9-]{4,28}\.[A-Za-z_][A-Za-z0-9_]{0,1023}\.[A-Za-z_][A-Za-z0-9_]{0,1023}$/u;
const BILLING_ACCOUNT = /^[0-9A-Z]{6}(?:-[0-9A-Z]{6}){2}$/u;
const MAXIMUM_OUTPUT_BYTES = 32 * 1024 * 1024;

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-cost-collector.mjs \\
    --billing-export-table <project.dataset.table> \\
    --billing-account <XXXXXX-XXXXXX-XXXXXX> \\
    --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD-exclusive> \\
    --source-base-revision <40-hex> --deployed-revision <40-hex> \\
    --fixed-floor-monthly-usd <number> \\
    --output <private-capture.json> --apply

Runs read-only BigQuery and Billing Budget queries using the current authorized
Google Cloud identity. The output is a closed, aggregate cost capture. It
includes gross Google cost and negative credits separately. Missing export
data, IAM, attribution, or budget policy fails closed and writes nothing.
`);
};

const parseArgs = (argumentsValue) => {
  const parsed = { apply: false };
  const names = new Map([
    ["--billing-account", "billingAccount"],
    ["--billing-export-table", "billingExportTable"],
    ["--deployed-revision", "deployedRevision"],
    ["--fixed-floor-monthly-usd", "fixedFloorMonthlyUsd"],
    ["--output", "output"],
    ["--source-base-revision", "sourceBaseRevision"],
    ["--window-end", "windowEnd"],
    ["--window-start", "windowStart"],
  ]);
  for (let index = 0; index < argumentsValue.length; index += 1) {
    const argument = argumentsValue[index];
    if (argument === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    const name = names.get(argument);
    if (!name) throw new Error(`unsupported argument ${argument}`);
    const value = argumentsValue[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
    if (parsed[name] !== undefined) throw new Error(`${argument} is duplicated`);
    parsed[name] = value;
    index += 1;
  }
  for (const name of names.values()) {
    if (parsed[name] === undefined) throw new Error(`missing required argument ${name}`);
  }
  if (!parsed.apply) throw new Error("cost collection requires --apply");
  if (process.env.OA_LIVEKIT_OWNER_GATE !== OWNER_GATE) {
    throw new Error(`--apply requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}`);
  }
  if (
    !TABLE.test(parsed.billingExportTable) ||
    !parsed.billingExportTable.startsWith(`${PROJECT}.`)
  ) {
    throw new Error("billing export table must be in the admitted project");
  }
  if (!BILLING_ACCOUNT.test(parsed.billingAccount)) throw new Error("billing account is invalid");
  return parsed;
};

const query = (command, argumentsValue, label) => {
  const result = spawnSync(command, argumentsValue, {
    encoding: "utf8",
    env: process.env,
    maxBuffer: MAXIMUM_OUTPUT_BYTES,
    shell: false,
    timeout: 120_000,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`${label} is unavailable through the authorized identity`);
  }
  try {
    return JSON.parse(result.stdout);
  } catch (error) {
    throw new Error(`${label} did not return JSON`, { cause: error });
  }
};

const billingSql = ({ billingExportTable, windowStart, windowEnd }) => `
SELECT
  CAST(DATE(usage_start_time) AS STRING) AS usageDate,
  project.id AS projectId,
  service.id AS serviceId,
  service.description AS serviceDescription,
  sku.id AS skuId,
  sku.description AS skuDescription,
  resource.name AS resourceName,
  TO_JSON_STRING(labels) AS labels,
  TO_JSON_STRING(system_labels) AS systemLabels,
  CAST(SUM(cost) AS STRING) AS grossCostUsd,
  CAST(SUM(COALESCE((SELECT SUM(credit.amount) FROM UNNEST(credits) AS credit), 0)) AS STRING) AS creditUsd
FROM \`${billingExportTable}\`
WHERE project.id = '${PROJECT}'
  AND DATE(usage_start_time) >= DATE '${windowStart}'
  AND DATE(usage_start_time) < DATE '${windowEnd}'
GROUP BY usageDate, projectId, serviceId, serviceDescription, skuId, skuDescription,
         resourceName, labels, systemLabels
ORDER BY usageDate, serviceId, skuId, resourceName
`;

const writeExclusive = (path, value) => {
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
};

try {
  const parsed = parseArgs(process.argv.slice(2));
  const exportRows = query(
    "bq",
    ["query", "--format=json", "--use_legacy_sql=false", billingSql(parsed)],
    "BigQuery billing export",
  );
  const budgets = query(
    "gcloud",
    [
      "billing",
      "budgets",
      "list",
      `--billing-account=${parsed.billingAccount}`,
      "--format=json",
      "--quiet",
    ],
    "Billing Budget inventory",
  );
  const capture = buildLiveKitCostCapture({
    sourceBaseRevision: parsed.sourceBaseRevision,
    deployedRevision: parsed.deployedRevision,
    observedAt: new Date().toISOString(),
    fixedFloorMonthlyUsd: Number(parsed.fixedFloorMonthlyUsd),
    exportRows,
    budgets,
    windowStart: parsed.windowStart,
    windowEnd: parsed.windowEnd,
  });
  const output = resolve(parsed.output);
  writeExclusive(output, capture);
  process.stdout.write(
    `${JSON.stringify({
      schemaVersion: capture.schemaVersion,
      observedAt: capture.observedAt,
      billingRowCount: capture.billingRows.length,
      output,
    })}\n`,
  );
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  usage();
  process.exitCode = 1;
}
