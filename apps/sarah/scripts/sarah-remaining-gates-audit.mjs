import { execFile } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const outPath = join(
  process.cwd(),
  "docs",
  "evidence",
  "2026-07-08-sarah-remaining-gates.json",
);

async function runNodeScript(script, env = {}) {
  try {
    const { stdout, stderr } = await execFileAsync("node", [script], {
      env: { ...process.env, ...env },
      maxBuffer: 10 * 1024 * 1024,
    });
    return parseRun(script, 0, stdout, stderr);
  } catch (error) {
    return parseRun(
      script,
      typeof error.code === "number" ? error.code : 1,
      error.stdout ?? "",
      error.stderr ?? "",
    );
  }
}

function parseJson(text) {
  const trimmed = text.trim();
  if (!trimmed) return null;
  try {
    return JSON.parse(trimmed);
  } catch {
    const first = trimmed.indexOf("{");
    const last = trimmed.lastIndexOf("}");
    if (first >= 0 && last > first) {
      return JSON.parse(trimmed.slice(first, last + 1));
    }
    return null;
  }
}

function parseRun(script, exitCode, stdout, stderr) {
  return {
    script,
    exitCode,
    json: parseJson(stdout) ?? parseJson(stderr),
    stderr: stderr.trim() ? stderr.trim().slice(0, 1_000) : null,
  };
}

function summarizeS6(run) {
  const json = run.json;
  return {
    issue: 6,
    status: json?.status ?? "unknown",
    summary:
      json?.status === "passed"
        ? "OpenAgents handoff and checkout endpoints are both deployed."
        : "OpenAgents checkout endpoint is still missing or not deployed.",
    evidence: {
      handoffStatus: json?.endpoints?.handoff?.status ?? null,
      handoffProtected: json?.endpoints?.handoff?.protectedByAuth ?? null,
      checkoutStatus: json?.endpoints?.checkout?.status ?? null,
      missing: json?.missing ?? null,
    },
    remainingExitGate: json?.remainingExitGate ?? null,
  };
}

function summarizeS7(run) {
  const json = run.json;
  const blockedSummary =
    json?.reason === "missing_crm_mcp_token_or_admin_bearer"
      ? "Catalog gate needs a scoped MCP token or admin bearer to inspect production tools."
      : json?.reason === "catalog_gate_failed"
        ? "Catalog gate could not mint or use a scoped CRM MCP grant."
        : "OpenAgents CRM MCP catalog still lacks Sarah's required write tools.";
  return {
    issue: 7,
    status: json?.status ?? "unknown",
    summary:
      json?.status === "passed"
        ? "OpenAgents CRM MCP catalog exposes Sarah's required write tools."
        : blockedSummary,
    evidence: {
      toolCount: json?.toolCount ?? null,
      requiredTools: json?.requiredTools ?? null,
      missingTools: json?.missingTools ?? null,
      temporaryGrantRevoked: json?.temporaryGrant?.revocation?.revoked ?? null,
      reason: json?.reason ?? null,
      error: json?.error ?? null,
    },
    remainingExitGate: json?.remainingExitGate ?? null,
  };
}

function summarizeS8(run) {
  const json = run.json;
  return {
    issue: 8,
    status: json?.status ?? "unknown",
    summary:
      json?.status === "passed"
        ? "Public Resend Chat SDK adapter line matches Sarah's AI SDK major."
        : "Public Resend Chat SDK adapter line remains incompatible with Sarah's AI SDK 7 realtime stack.",
    evidence: {
      adapterVersion: json?.packages?.adapter?.version ?? null,
      chatVersion: json?.packages?.chat?.version ?? null,
      chatAiPeer: json?.packages?.chat?.peerDependencies?.ai ?? null,
      sarahAi: json?.packages?.sarah?.ai ?? null,
    },
    remainingExitGate: json?.remainingExitGate ?? null,
  };
}

function summarizeS11(run) {
  const json = run.json;
  return {
    issue: 11,
    status: run.exitCode === 0 && json?.checks ? "passed" : "blocked",
    summary:
      run.exitCode === 0
        ? "Production Sarah smoke target responded."
        : "Production Sarah smoke target is not serving the expected Sarah app.",
    evidence: {
      baseUrl: json?.baseUrl ?? "https://sarah.openagents.com",
      error: json?.error ?? null,
      publicPage: json?.checks?.publicPage ?? null,
      sessionConfig: json?.checks?.sessionConfig ?? null,
      token: json?.checks?.token ?? null,
    },
    remainingExitGate:
      run.exitCode === 0
        ? "Run the token-mint smoke, S-12 evals, and browser voice smoke against production before closing S-11."
        : "Configure production DNS/hosting/env for sarah.openagents.com, then rerun pnpm smoke:production.",
  };
}

const runs = {
  s6: await runNodeScript("scripts/sarah-s6-openagents-gate-smoke.mjs"),
  s7: await runNodeScript("scripts/sarah-s7-mcp-catalog-gate.mjs"),
  s8: await runNodeScript("scripts/sarah-s8-resend-adapter-compat.mjs"),
  s11: await runNodeScript("scripts/sarah-production-smoke.mjs"),
};

const issues = [
  summarizeS6(runs.s6),
  summarizeS7(runs.s7),
  summarizeS8(runs.s8),
  summarizeS11(runs.s11),
];
const openOriginalIssues = issues
  .filter((issue) => issue.status !== "passed")
  .map((issue) => issue.issue);
const audit = {
  schema: "sarah.remaining_gates_audit.v1",
  generatedAt: new Date().toISOString(),
  status: openOriginalIssues.length === 0 ? "passed" : "blocked",
  scope: "Original Sarah S-1..S-13 remaining open-lane gates",
  issues,
  rawRunExitCodes: Object.fromEntries(
    Object.entries(runs).map(([name, run]) => [name, run.exitCode]),
  ),
  openOriginalIssues,
};

await mkdir(dirname(outPath), { recursive: true });
await writeFile(outPath, `${JSON.stringify(audit, null, 2)}\n`);
console.log(JSON.stringify(audit, null, 2));

if (process.env.SARAH_REMAINING_GATES_REQUIRE_PASS === "1" && openOriginalIssues.length > 0) {
  process.exitCode = 2;
}
