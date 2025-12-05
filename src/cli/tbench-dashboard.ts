#!/usr/bin/env bun
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import * as S from "effect/Schema";
import { BenchmarkResults, TaskMetrics } from "../bench/metrics.js";
import { TerminalBenchSuite } from "../bench/terminal-bench.js";

export interface RunSummary {
  runId: string;
  completedAt: string;
  model: string;
  successRate: number;
  verificationPassRate: number;
  totalTokens: number;
  totalTasks: number;
  totalDurationMs: number;
  categoryStats: Record<string, { success: number; total: number }>;
}

const decodeJson = <A>(schema: S.Schema<A>, content: string): A => {
  const parsed = JSON.parse(content);
  return S.decodeUnknownSync(schema)(parsed);
};

const loadCategoryMap = (suitePath: string | undefined): Record<string, string> => {
  if (!suitePath || !existsSync(suitePath)) {
    return {};
  }

  try {
    const suiteContent = readFileSync(suitePath, "utf8");
    const suite = decodeJson(TerminalBenchSuite, suiteContent);
    return Object.fromEntries(suite.tasks.map((t) => [t.id, t.category]));
  } catch {
    return {};
  }
};

const taskCategory = (task: TaskMetrics, categoryMap: Record<string, string>): string => {
  return categoryMap[task.taskId] ?? "uncategorized";
};

export const computeRunSummary = (
  results: S.Schema.Type<typeof BenchmarkResults>,
  categoryMap: Record<string, string>,
): RunSummary => {
  const totalTokens = results.tasks.reduce(
    (sum, t) => sum + t.totalTokenUsage.input + t.totalTokenUsage.output,
    0,
  );

  const categoryStats: Record<string, { success: number; total: number }> = {};
  for (const task of results.tasks) {
    const category = taskCategory(task, categoryMap);
    const bucket = categoryStats[category] ?? { success: 0, total: 0 };
    bucket.total += 1;
    if (task.outcome === "success") {
      bucket.success += 1;
    }
    categoryStats[category] = bucket;
  }

  return {
    runId: results.meta.runId,
    completedAt: results.meta.completedAt,
    model: results.meta.model,
    successRate: results.summary.taskCompletionRate,
    verificationPassRate: results.summary.verificationPassRate,
    totalTokens,
    totalTasks: results.summary.totalTasks,
    totalDurationMs: results.summary.totalDurationMs,
    categoryStats,
  };
};

const buildDashboardHtml = (runs: RunSummary[]) => {
  const sorted = [...runs].sort(
    (a, b) => new Date(a.completedAt).getTime() - new Date(b.completedAt).getTime(),
  );
  const aggregatedCategories: Record<string, { success: number; total: number }> = {};
  for (const run of sorted) {
    for (const [category, stats] of Object.entries(run.categoryStats)) {
      const bucket = aggregatedCategories[category] ?? { success: 0, total: 0 };
      bucket.success += stats.success;
      bucket.total += stats.total;
      aggregatedCategories[category] = bucket;
    }
  }

  const dataJson = JSON.stringify(sorted);
  const aggJson = JSON.stringify(aggregatedCategories);

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Terminal-Bench Dashboard</title>
  <style>
    :root { color-scheme: light dark; font-family: "Inter", system-ui, -apple-system, sans-serif; }
    body { margin: 0; padding: 24px; background: #0b1220; color: #e8f0ff; }
    h1 { margin: 0 0 12px; font-size: 24px; }
    .muted { color: #94a3b8; }
    .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 12px; margin: 16px 0; }
    .card { padding: 14px; border-radius: 12px; background: linear-gradient(135deg, #111827, #0f172a); border: 1px solid #1f2937; }
    .card h3 { margin: 0 0 6px; font-size: 14px; color: #9fb4ff; letter-spacing: 0.03em; }
    .card .value { font-size: 22px; font-weight: 600; }
    .section { margin-top: 24px; }
    table { width: 100%; border-collapse: collapse; margin-top: 8px; }
    th, td { padding: 8px; border-bottom: 1px solid #1e293b; text-align: left; }
    th { color: #93c5fd; font-weight: 600; }
    .pill { display: inline-block; padding: 2px 8px; border-radius: 999px; background: #1e293b; color: #cbd5e1; font-size: 12px; }
    .bar { height: 6px; background: #1f2937; border-radius: 999px; overflow: hidden; }
    .bar > div { height: 100%; background: linear-gradient(90deg, #22d3ee, #6366f1); }
    .trend { display: grid; gap: 10px; }
    .trend-item { display: grid; grid-template-columns: 1fr auto; gap: 8px; align-items: center; }
    .category-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 10px; }
    .small { font-size: 12px; color: #cbd5e1; }
  </style>
</head>
<body>
  <h1>Terminal-Bench Dashboard</h1>
  <p class="muted" id="meta"></p>

  <div class="cards" id="summary-cards"></div>

  <div class="section">
    <h2>Run Trend</h2>
    <div id="trend" class="trend"></div>
  </div>

  <div class="section">
    <h2>Per-Run Details</h2>
    <table id="runs-table">
      <thead>
        <tr>
          <th>Run</th><th>Date</th><th>Model</th><th>Success</th><th>Verification</th><th>Tokens</th><th>Duration</th>
        </tr>
      </thead>
      <tbody></tbody>
    </table>
  </div>

  <div class="section">
    <h2>Category Performance (all runs)</h2>
    <div id="categories" class="category-grid"></div>
  </div>

  <script>
    const runs = ${dataJson};
    const aggregatedCategories = ${aggJson};

    const fmtPercent = (v) => (v * 100).toFixed(1) + "%";
    const fmtDate = (iso) => new Date(iso).toLocaleString();
    const fmtTokens = (v) => Math.round(v).toLocaleString() + " tok";
    const fmtDuration = (ms) => (ms / 60000).toFixed(1) + " min";

    const summaryEl = document.getElementById("summary-cards");
    const metaEl = document.getElementById("meta");
    const trendEl = document.getElementById("trend");
    const tableBody = document.querySelector("#runs-table tbody");
    const categoriesEl = document.getElementById("categories");

    if (!runs.length) {
      metaEl.textContent = "No runs found. Provide a directory with benchmark results JSON files.";
    } else {
      const last = runs[runs.length - 1];
      metaEl.textContent = \`\${runs.length} runs | Last run: \${last.runId} (\${fmtDate(last.completedAt)})\`;

      const avgSuccess = runs.reduce((s, r) => s + r.successRate, 0) / runs.length;
      const avgVerification = runs.reduce((s, r) => s + r.verificationPassRate, 0) / runs.length;
      const totalTokens = runs.reduce((s, r) => s + r.totalTokens, 0);

      const cards = [
        { label: "Avg Success Rate", value: fmtPercent(avgSuccess) },
        { label: "Avg Verification", value: fmtPercent(avgVerification) },
        { label: "Total Tokens", value: fmtTokens(totalTokens) },
        { label: "Runs", value: runs.length.toString() },
      ];

      summaryEl.innerHTML = cards.map(c => \`
        <div class="card">
          <h3>\${c.label}</h3>
          <div class="value">\${c.value}</div>
        </div>\`).join("");

      trendEl.innerHTML = runs.map(run => {
        const pct = Math.max(0, Math.min(100, run.successRate * 100));
        return \`
          <div class="trend-item">
            <div>
              <div><strong>\${run.runId}</strong> · <span class="muted">\${fmtDate(run.completedAt)}</span></div>
              <div class="bar"><div style="width:\${pct}%"></div></div>
            </div>
            <div class="pill">\${fmtPercent(run.successRate)}</div>
          </div>\`;
      }).join("");

      tableBody.innerHTML = runs.map(run => \`
        <tr>
          <td><code>\${run.runId}</code></td>
          <td>\${fmtDate(run.completedAt)}</td>
          <td>\${run.model}</td>
          <td>\${fmtPercent(run.successRate)}</td>
          <td>\${fmtPercent(run.verificationPassRate)}</td>
          <td>\${fmtTokens(run.totalTokens)}</td>
          <td>\${fmtDuration(run.totalDurationMs)}</td>
        </tr>\`).join("");

      const categoryCards = Object.entries(aggregatedCategories)
        .sort((a, b) => b[1].total - a[1].total)
        .map(([category, stats]) => {
          const rate = stats.total ? (stats.success / stats.total) : 0;
          const pct = Math.max(0, Math.min(100, rate * 100));
          return \`
            <div class="card">
              <h3>\${category}</h3>
              <div class="value">\${fmtPercent(rate)}</div>
              <div class="small">\${stats.success} / \${stats.total} passed</div>
              <div class="bar" style="margin-top:8px;"><div style="width:\${pct}%"></div></div>
            </div>\`;
        }).join("");
      categoriesEl.innerHTML = categoryCards || "<p class='muted'>No category data available.</p>";
    }
  </script>
</body>
</html>`;
};

const parseArgs = () => {
  const args = process.argv.slice(2);
  const get = (flag: string, fallback?: string) => {
    const idx = args.indexOf(flag);
    return idx >= 0 ? args[idx + 1] : fallback;
  };
  return {
    resultsDir: get("--results-dir", get("--dir", "./results")) ?? "./results",
    output: get("--output", "terminal-bench-dashboard.html") ?? "terminal-bench-dashboard.html",
    suitePath: get("--suite", "tasks/terminal-bench-2.json") ?? "tasks/terminal-bench-2.json",
  };
};

const main = () => {
  const { resultsDir, output, suitePath } = parseArgs();
  const resolvedDir = resolve(resultsDir);
  if (!existsSync(resolvedDir)) {
    console.error(`Results directory not found: ${resolvedDir}`);
    process.exit(1);
  }

  const categoryMap = loadCategoryMap(suitePath ? resolve(suitePath) : undefined);
  const files = readdirSync(resolvedDir).filter((f) => f.endsWith(".json"));
  const runs: RunSummary[] = [];

  for (const file of files) {
    try {
      const content = readFileSync(join(resolvedDir, file), "utf8");
      const results = decodeJson(BenchmarkResults, content);
      runs.push(computeRunSummary(results, categoryMap));
    } catch (error) {
      console.warn(`Skipping ${file}: ${(error as Error).message}`);
    }
  }

  const html = buildDashboardHtml(runs);
  const outputPath = resolve(output);
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, html, "utf8");
  console.log(`Dashboard written to ${outputPath} (${runs.length} runs included)`);
};

if (import.meta.main) {
  main();
}

export { buildDashboardHtml };
