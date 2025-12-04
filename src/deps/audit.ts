import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";

export interface AuditSummary {
  status: "ok" | "failed";
  vulnerabilities: Record<string, number>;
  raw?: unknown;
  error?: string;
}

export interface UpgradeSummary {
  count: number;
  packages: Array<{ name: string; current: string; latest: string }>;
  raw?: unknown;
  error?: string;
}

export interface DepAuditReport {
  generatedAt: string;
  audit: AuditSummary;
  upgrades: UpgradeSummary;
}

const runJsonCommand = (cmd: string, args: string[]): { stdout?: string; error?: string } => {
  const proc = spawnSync(cmd, args, { encoding: "utf8", shell: false });
  if (proc.error) return { error: proc.error.message };
  if (proc.stdout) return { stdout: proc.stdout };
  if (proc.stderr) return { error: proc.stderr };
  return { error: "No output" };
};

export const summarizeNpmAudit = (raw: string | undefined): AuditSummary => {
  if (!raw) {
    return { status: "failed", vulnerabilities: {}, error: "No audit output" };
  }
  try {
    const parsed = JSON.parse(raw) as any;
    const vuln = parsed?.metadata?.vulnerabilities ?? {};
    const summary: Record<string, number> = {
      info: vuln.info ?? 0,
      low: vuln.low ?? 0,
      moderate: vuln.moderate ?? 0,
      high: vuln.high ?? 0,
      critical: vuln.critical ?? 0,
    };
    const total = Object.values(summary).reduce((a, b) => a + b, 0);
    return { status: total === 0 ? "ok" : "failed", vulnerabilities: summary, raw: parsed };
  } catch (err) {
    return { status: "failed", vulnerabilities: {}, error: (err as Error).message };
  }
};

export const summarizeNcu = (raw: string | undefined): UpgradeSummary => {
  if (!raw) return { count: 0, packages: [], error: "No ncu output" };
  try {
    const parsed = JSON.parse(raw) as Record<string, { current: string; latest: string }>;
    const packages = Object.entries(parsed).map(([name, info]) => ({
      name,
      current: info.current,
      latest: info.latest,
    }));
    return { count: packages.length, packages, raw: parsed };
  } catch (err) {
    return { count: 0, packages: [], error: (err as Error).message };
  }
};

export const runDepAudit = (): DepAuditReport => {
  const auditResult = runJsonCommand("npm", ["audit", "--json", "--production"]);
  const ncuResult = runJsonCommand("bunx", ["npm-check-updates", "--jsonAll"]);

  const audit = summarizeNpmAudit(auditResult.stdout);
  if (auditResult.error) {
    audit.status = "failed";
    audit.error = auditResult.error;
  }

  const upgrades = summarizeNcu(ncuResult.stdout);
  if (ncuResult.error) {
    upgrades.error = ncuResult.error;
  }

  return {
    generatedAt: new Date().toISOString(),
    audit,
    upgrades,
  };
};

export const writeReport = (report: DepAuditReport, outputPath: string): void => {
  const dir = path.dirname(outputPath);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(outputPath, JSON.stringify(report, null, 2));
};
