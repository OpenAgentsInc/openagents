import { execFile } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const OPENAGENTS_APP_DIR =
  process.env.SARAH_OPENAGENTS_APP_DIR ??
  "/Users/christopherdavid/work/openagents-sarah-checkout/apps/openagents.com/workers/api";
const PENDING_MIGRATION = "0309_provider_account_token_custody_auth_deleted.sql";
const outPath = join(
  process.cwd(),
  "docs",
  "evidence",
  "2026-07-08-openagents-d1-deploy-blocker.json",
);

async function runWrangler(args) {
  try {
    const { stdout, stderr } = await execFileAsync("bunx", ["wrangler", ...args], {
      cwd: OPENAGENTS_APP_DIR,
      maxBuffer: 10 * 1024 * 1024,
    });
    return { exitCode: 0, stdout, stderr };
  } catch (error) {
    return {
      exitCode: typeof error.code === "number" ? error.code : 1,
      stdout: error.stdout ?? "",
      stderr: error.stderr ?? "",
    };
  }
}

function parseJson(text) {
  const trimmed = text.trim();
  if (!trimmed) return null;
  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

function summarizeDatabases(databases) {
  const rows = Array.isArray(databases) ? databases : [];
  const totalBytes = rows.reduce((sum, row) => sum + Number(row.file_size ?? 0), 0);
  const byName = Object.fromEntries(
    rows
      .filter((row) => typeof row.name === "string")
      .map((row) => [
        row.name,
        {
          fileSizeBytes: Number(row.file_size ?? 0),
          uuid: row.uuid ?? null,
          createdAt: row.created_at ?? null,
        },
      ]),
  );

  return { totalBytes, byName };
}

function summarizeMigrations(run) {
  const stdout = run.stdout.trim();
  const stderr = run.stderr.trim();
  const warningCount = (stderr.match(/\[WARNING\]/g) ?? []).length;
  const pendingMigrations = stdout.includes(PENDING_MIGRATION) ? [PENDING_MIGRATION] : [];
  return {
    exitCode: run.exitCode,
    pendingMigrationDetected: pendingMigrations.length > 0,
    pendingMigrations,
    warningCount,
    stderrSnippet: run.exitCode === 0 ? null : stderr.slice(0, 2_000),
  };
}

const listRun = await runWrangler(["d1", "list", "--json"]);
const databases = parseJson(listRun.stdout) ?? [];
const d1 = summarizeDatabases(databases);
const stagingRun = await runWrangler([
  "d1",
  "migrations",
  "list",
  "openagents-autopilot-staging",
  "--env",
  "staging",
  "--remote",
]);
const productionRun = await runWrangler([
  "d1",
  "migrations",
  "list",
  "openagents-autopilot",
  "--remote",
]);
const migrations = {
  staging: summarizeMigrations(stagingRun),
  production: summarizeMigrations(productionRun),
};
const pendingMigrationDetected =
  migrations.staging.pendingMigrationDetected || migrations.production.pendingMigrationDetected;
const relevantDatabaseSizes = {
  "openagents-autopilot": d1.byName["openagents-autopilot"]?.fileSizeBytes ?? null,
  "openagents-autopilot-staging":
    d1.byName["openagents-autopilot-staging"]?.fileSizeBytes ?? null,
  "openagents-moltbook-index":
    d1.byName["openagents-moltbook-index"]?.fileSizeBytes ?? null,
  aman_kb: d1.byName.aman_kb?.fileSizeBytes ?? null,
};
const audit = {
  schema: "sarah.openagents_d1_deploy_blocker_audit.v1",
  generatedAt: new Date().toISOString(),
  openAgentsAppDir: OPENAGENTS_APP_DIR,
  status: pendingMigrationDetected ? "blocked" : "needs_deploy_attempt",
  readOnly: true,
  d1: {
    listExitCode: listRun.exitCode,
    totalBytes: d1.totalBytes,
    relevantDatabaseSizes,
    databaseCount: Object.keys(d1.byName).length,
  },
  migrations,
  pendingMigration: PENDING_MIGRATION,
  remainingExitGate: pendingMigrationDetected
    ? "Raise/free Cloudflare D1 account storage, then rerun the sanctioned OpenAgents deploy path so this migration applies before the Worker upload."
    : "Rerun the sanctioned OpenAgents deploy path and Sarah S-6/S-7 gates; this probe no longer sees the previously pending migration.",
};

await mkdir(dirname(outPath), { recursive: true });
await writeFile(outPath, `${JSON.stringify(audit, null, 2)}\n`);
console.log(JSON.stringify(audit, null, 2));
