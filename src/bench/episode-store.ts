/**
 * Episode store for TerminalBench overnight runs.
 *
 * Stores Episode records in JSONL format for tracking benchmark iterations.
 * This is the foundation for the Archivist subagent in future phases.
 *
 * Storage location: .openagents/gym/episodes.jsonl
 */

import { existsSync, mkdirSync, readFileSync, appendFileSync, writeFileSync } from "fs";
import { join, dirname } from "path";
import * as S from "effect/Schema";

// --- Episode Schema ---

export const EpisodeStatus = S.Literal("success", "partial", "failed", "timeout", "error");
export type EpisodeStatus = S.Schema.Type<typeof EpisodeStatus>;

export const EpisodeSummary = S.Struct({
  total: S.Number,
  passed: S.Number,
  failed: S.Number,
  timeout: S.Number,
  error: S.Number,
  passRate: S.Number,
  avgTurns: S.Number,
  avgTokens: S.Number,
  totalDurationMs: S.Number,
});
export type EpisodeSummary = S.Schema.Type<typeof EpisodeSummary>;

export const Episode = S.Struct({
  id: S.String,
  runId: S.String,
  iteration: S.Number,
  model: S.String,
  suiteVersion: S.String,
  startedAt: S.String, // ISO timestamp
  finishedAt: S.String, // ISO timestamp
  status: EpisodeStatus,
  summary: EpisodeSummary,
  resultsPath: S.String,
  // Optional metadata for learning
  tags: S.optional(S.Array(S.String)),
  notes: S.optional(S.String),
  baselineComparison: S.optional(S.Struct({
    baselineEpisodeId: S.String,
    passRateDelta: S.Number,
    improved: S.Array(S.String),
    regressed: S.Array(S.String),
  })),
});
export type Episode = S.Schema.Type<typeof Episode>;

// --- Episode Store ---

export class EpisodeStore {
  private readonly storePath: string;

  constructor(gymDir: string) {
    this.storePath = join(gymDir, "episodes.jsonl");

    // Ensure directory exists
    const dir = dirname(this.storePath);
    if (!existsSync(dir)) {
      mkdirSync(dir, { recursive: true });
    }
  }

  /**
   * Record a new episode.
   */
  async record(episode: Episode): Promise<void> {
    const line = JSON.stringify(episode) + "\n";
    appendFileSync(this.storePath, line);
  }

  /**
   * Load all episodes from store.
   */
  async loadAll(): Promise<Episode[]> {
    if (!existsSync(this.storePath)) {
      return [];
    }

    const content = readFileSync(this.storePath, "utf-8");
    const lines = content.trim().split("\n").filter(Boolean);

    const episodes: Episode[] = [];
    for (const line of lines) {
      try {
        const parsed = JSON.parse(line);
        const decoded = S.decodeUnknownSync(Episode)(parsed);
        episodes.push(decoded);
      } catch (e) {
        console.warn(`Failed to parse episode line: ${e}`);
      }
    }

    return episodes;
  }

  /**
   * Query episodes with optional filters.
   */
  async query(filter: {
    runId?: string;
    model?: string;
    status?: EpisodeStatus;
    since?: Date;
    limit?: number;
  } = {}): Promise<Episode[]> {
    let episodes = await this.loadAll();

    if (filter.runId) {
      episodes = episodes.filter(e => e.runId === filter.runId);
    }

    if (filter.model) {
      episodes = episodes.filter(e => e.model === filter.model);
    }

    if (filter.status) {
      episodes = episodes.filter(e => e.status === filter.status);
    }

    if (filter.since) {
      const sinceTime = filter.since.getTime();
      episodes = episodes.filter(e => new Date(e.startedAt).getTime() >= sinceTime);
    }

    // Sort by startedAt descending (most recent first)
    episodes.sort((a, b) =>
      new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime()
    );

    if (filter.limit) {
      episodes = episodes.slice(0, filter.limit);
    }

    return episodes;
  }

  /**
   * Get the most recent baseline episode for a given model.
   * Returns the most recent successful episode.
   */
  async getBaseline(model?: string): Promise<Episode | null> {
    const queryOptions: Parameters<EpisodeStore["query"]>[0] = {
      status: "success",
      limit: 1,
    };
    if (model) {
      queryOptions.model = model;
    }
    const episodes = await this.query(queryOptions);
    return episodes[0] ?? null;
  }

  /**
   * Get episodes for a specific run.
   */
  async getRunEpisodes(runId: string): Promise<Episode[]> {
    const episodes = await this.query({ runId });
    // Sort by iteration
    episodes.sort((a, b) => a.iteration - b.iteration);
    return episodes;
  }

  /**
   * Get statistics across all episodes.
   */
  async getStats(): Promise<{
    totalEpisodes: number;
    totalRuns: number;
    byModel: Map<string, { count: number; avgPassRate: number }>;
    byStatus: Map<EpisodeStatus, number>;
    passRateTrend: Array<{ date: string; passRate: number }>;
  }> {
    const episodes = await this.loadAll();

    const totalEpisodes = episodes.length;
    const runIds = new Set(episodes.map(e => e.runId));
    const totalRuns = runIds.size;

    // By model
    const byModel = new Map<string, { count: number; totalPassRate: number }>();
    for (const ep of episodes) {
      const existing = byModel.get(ep.model) ?? { count: 0, totalPassRate: 0 };
      existing.count++;
      existing.totalPassRate += ep.summary.passRate;
      byModel.set(ep.model, existing);
    }
    const byModelStats = new Map<string, { count: number; avgPassRate: number }>();
    for (const [model, stats] of byModel) {
      byModelStats.set(model, {
        count: stats.count,
        avgPassRate: stats.count > 0 ? stats.totalPassRate / stats.count : 0,
      });
    }

    // By status
    const byStatus = new Map<EpisodeStatus, number>();
    for (const ep of episodes) {
      byStatus.set(ep.status, (byStatus.get(ep.status) ?? 0) + 1);
    }

    // Pass rate trend (daily average)
    const byDate = new Map<string, { count: number; totalPassRate: number }>();
    for (const ep of episodes) {
      const date = ep.startedAt.slice(0, 10); // YYYY-MM-DD
      const existing = byDate.get(date) ?? { count: 0, totalPassRate: 0 };
      existing.count++;
      existing.totalPassRate += ep.summary.passRate;
      byDate.set(date, existing);
    }
    const passRateTrend = Array.from(byDate.entries())
      .map(([date, stats]) => ({
        date,
        passRate: stats.count > 0 ? stats.totalPassRate / stats.count : 0,
      }))
      .sort((a, b) => a.date.localeCompare(b.date));

    return {
      totalEpisodes,
      totalRuns,
      byModel: byModelStats,
      byStatus,
      passRateTrend,
    };
  }

  /**
   * Clear all episodes (use with caution).
   */
  async clear(): Promise<void> {
    if (existsSync(this.storePath)) {
      writeFileSync(this.storePath, "");
    }
  }
}

// --- Helper Functions ---

/**
 * Create an Episode from benchmark results.
 */
export const createEpisode = (params: {
  runId: string;
  iteration: number;
  model: string;
  suiteVersion: string;
  startedAt: Date;
  finishedAt: Date;
  results: {
    total: number;
    passed: number;
    failed: number;
    timeout: number;
    error: number;
    avgTurns: number;
    avgTokens: number;
    totalDurationMs: number;
  };
  resultsPath: string;
  baselineEpisode?: Episode;
  improvedTasks?: string[];
  regressedTasks?: string[];
}): Episode => {
  const {
    runId,
    iteration,
    model,
    suiteVersion,
    startedAt,
    finishedAt,
    results,
    resultsPath,
    baselineEpisode,
    improvedTasks,
    regressedTasks,
  } = params;

  const passRate = results.total > 0 ? results.passed / results.total : 0;

  // Determine status
  let status: EpisodeStatus;
  if (results.error > 0 || results.total === 0) {
    status = "error";
  } else if (results.timeout > 0) {
    status = "timeout";
  } else if (passRate >= 0.8) {
    status = "success";
  } else if (passRate >= 0.3) {
    status = "partial";
  } else {
    status = "failed";
  }

  const baseEpisode: Episode = {
    id: `${runId}-${String(iteration).padStart(3, "0")}`,
    runId,
    iteration,
    model,
    suiteVersion,
    startedAt: startedAt.toISOString(),
    finishedAt: finishedAt.toISOString(),
    status,
    summary: {
      total: results.total,
      passed: results.passed,
      failed: results.failed,
      timeout: results.timeout,
      error: results.error,
      passRate,
      avgTurns: results.avgTurns,
      avgTokens: results.avgTokens,
      totalDurationMs: results.totalDurationMs,
    },
    resultsPath,
  };

  if (baselineEpisode) {
    return {
      ...baseEpisode,
      baselineComparison: {
        baselineEpisodeId: baselineEpisode.id,
        passRateDelta: passRate - baselineEpisode.summary.passRate,
        improved: improvedTasks ?? [],
        regressed: regressedTasks ?? [],
      },
    };
  }

  return baseEpisode;
};

/**
 * Generate a unique run ID.
 */
export const generateRunId = (): string => {
  const date = new Date();
  const dateStr = date.toISOString().slice(0, 10).replace(/-/g, "");
  const timeStr = date.toISOString().slice(11, 19).replace(/:/g, "");
  const random = Math.random().toString(36).slice(2, 6);
  return `tbrun-${dateStr}-${timeStr}-${random}`;
};
