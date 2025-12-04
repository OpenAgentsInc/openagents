import * as fs from "node:fs";

export interface TrimConfig {
  /**
   * Maximum allowed file size before trimming is enforced.
   */
  maxBytes: number;
  /**
   * Maximum allowed line count before trimming is enforced.
   */
  maxLines: number;
  /**
   * Target line count after trimming (including the trim marker).
   */
  trimToLines: number;
  /**
   * Number of most recent lines to keep when trimming.
   */
  tailLines: number;
  /**
   * Event types that should always be preserved even if they are not in the tail window.
   */
  criticalTypes: ReadonlySet<string>;
  /**
   * Event type name for the trim marker that documents what was dropped.
   */
  markerType: string;
}

const DEFAULT_NOW = () => new Date().toISOString();

const serialize = (lines: string[]) => lines.join("\n") + "\n";

const parseType = (line: string): string | null => {
  try {
    const parsed = JSON.parse(line);
    const maybeType = (parsed as any)?.type;
    return typeof maybeType === "string" ? maybeType : null;
  } catch {
    return null;
  }
};

export interface TrimResult {
  trimmed: boolean;
  lines: string[];
  dropped: number;
  kept: number;
}

/**
 * Trim JSONL lines to keep the head, tail, and all critical events while inserting a marker.
 */
export const trimJsonlLines = (
  lines: string[],
  config: TrimConfig,
  now: () => string = DEFAULT_NOW,
): TrimResult => {
  const normalized = lines.filter((line) => line.trim().length > 0);
  const currentSize = Buffer.byteLength(serialize(normalized), "utf8");

  if (normalized.length === 0) {
    return { trimmed: false, lines: [], dropped: 0, kept: 0 };
  }

  if (normalized.length <= config.maxLines && currentSize <= config.maxBytes) {
    return { trimmed: false, lines: normalized, dropped: 0, kept: normalized.length };
  }

  const total = normalized.length;
  const criticalIndices = new Set<number>();

  normalized.forEach((line, idx) => {
    const type = parseType(line);
    if (type && config.criticalTypes.has(type)) {
      criticalIndices.add(idx);
    }
  });

  const build = (tailCount: number) => {
    const startOfTail = Math.max(0, total - tailCount);
    const keep = new Set<number>([0]); // Always keep the first event (run/session start)
    criticalIndices.forEach((idx) => keep.add(idx));
    for (let i = startOfTail; i < total; i++) {
      keep.add(i);
    }

    const ordered = Array.from(keep).sort((a, b) => a - b);
    const keptOriginal = ordered.length;
    const dropped = Math.max(total - keptOriginal, 0);
    const marker = JSON.stringify({
      type: config.markerType,
      ts: now(),
      dropped,
      kept: keptOriginal,
      reason: "trimmed_for_size",
    });
    const output = [normalized[0], marker, ...ordered.slice(1).map((idx) => normalized[idx])];
    const size = Buffer.byteLength(serialize(output), "utf8");

    return { output, size, dropped, keptOriginal };
  };

  let tailCount = Math.min(config.tailLines, total);
  let built = build(tailCount);

  while ((built.output.length > config.trimToLines || built.size > config.maxBytes) && tailCount > 1) {
    const nextTail = Math.max(1, Math.floor(tailCount * 0.7));
    if (nextTail === tailCount) break;
    tailCount = nextTail;
    built = build(tailCount);
  }

  if (built.dropped === 0) {
    return { trimmed: false, lines: normalized, dropped: 0, kept: total };
  }

  return {
    trimmed: true,
    lines: built.output,
    dropped: built.dropped,
    kept: built.keptOriginal,
  };
};

export const RUN_LOG_TRIM_CONFIG: TrimConfig = {
  maxBytes: 5_000_000, // ~5MB
  maxLines: 5_000,
  trimToLines: 1_500,
  tailLines: 1_200,
  criticalTypes: new Set([
    "run_start",
    "task_selected",
    "verify_start",
    "verify_ok",
    "verify_fail",
    "commit_pushed",
    "task_closed",
    "run_end",
    "timeout",
    "retry_prompt",
  ]),
  markerType: "log_trimmed",
};

export const SESSION_TRIM_CONFIG: TrimConfig = {
  maxBytes: 4_000_000, // ~4MB
  maxLines: 4_000,
  trimToLines: 900,
  tailLines: 700,
  criticalTypes: new Set(["session_start", "user_message", "session_end"]),
  markerType: "log_trimmed",
};

export const maybeTrimFileSync = (
  filePath: string,
  config: TrimConfig,
  now: () => string = DEFAULT_NOW,
): TrimResult => {
  if (!fs.existsSync(filePath)) {
    return { trimmed: false, lines: [], dropped: 0, kept: 0 };
  }

  const content = fs.readFileSync(filePath, "utf8");
  const lines = content.split("\n").filter((line) => line.trim().length > 0);
  const result = trimJsonlLines(lines, config, now);

  if (result.trimmed) {
    fs.writeFileSync(filePath, serialize(result.lines), "utf8");
  }

  return result;
};
