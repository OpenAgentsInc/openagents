/**
 * Foreign session resume picker for `openagents coder`.
 *
 * The scanner half of OpenAgentsInc/openagents.com#198 is a packet-v0 WASM
 * plugin under `plugins/foreign-sessions`. This module is the CLI picker
 * half: it builds the bounded, filtered scan request, interprets the
 * metadata-only result, renders a numbered list, and prints a resume command.
 *
 * It takes an `invoke` seam so tests can stand in a fake plugin and so the
 * real call loads the plugin and invokes it outside this module.
 */

const DAY_MS = 86_400_000;
const HOUR_MS = 3_600_000;

export const DEFAULT_MAX_AGE_DAYS = 30;
export const DEFAULT_PICKER_LIMIT = 10;

export interface ForeignSession {
  readonly source: "claude" | "codex";
  readonly session_id: string;
  readonly path: string;
  readonly cwd: string | undefined;
  readonly project_dir: string | undefined;
  readonly mtime_ms: number;
  readonly size_bytes: number;
  readonly record_count: number | undefined;
  readonly metadata_truncated: boolean;
}

export interface ForeignScanOutput {
  readonly sessions: ReadonlyArray<ForeignSession>;
  readonly scanned_dirs: number;
  readonly scanned_files: number;
  readonly skipped: {
    readonly malformed: number;
    readonly unreadable: number;
    readonly symlinked: number;
  };
  readonly oversized: number;
  readonly missing_sources: ReadonlyArray<string>;
  readonly scan_truncated: boolean;
  readonly read_budget_exhausted: boolean;
}

export interface ForeignScanRefusal {
  readonly code: string;
  readonly reason: string;
}

export interface ForeignResumeDeps {
  readonly now_ms: number;
  readonly cwd: string;
  readonly selection: number | undefined;
}

export type ForeignResumeInvoke = (
  input: Record<string, unknown>,
) => Promise<unknown>;

export interface ForeignResumeOptions {
  readonly max_age_days?: number | undefined;
  readonly limit?: number | undefined;
}

const asRecord = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

const asString = (value: unknown): string =>
  typeof value === "string" ? value : "";

const asNumber = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) ? value : 0;

const asBool = (value: unknown): boolean => value === true;

const asArray = (value: unknown): ReadonlyArray<unknown> =>
  Array.isArray(value) ? value : [];

function parseSession(value: unknown): ForeignSession | undefined {
  const record = asRecord(value);
  const source = record["source"];
  if (source !== "claude" && source !== "codex") return undefined;
  return {
    source,
    session_id: asString(record["session_id"]),
    path: asString(record["path"]),
    cwd: record["cwd"] === undefined ? undefined : asString(record["cwd"]),
    project_dir:
      record["project_dir"] === undefined
        ? undefined
        : asString(record["project_dir"]),
    mtime_ms: asNumber(record["mtime_ms"]),
    size_bytes: asNumber(record["size_bytes"]),
    record_count:
      record["record_count"] === undefined
        ? undefined
        : asNumber(record["record_count"]),
    metadata_truncated: asBool(record["metadata_truncated"]),
  };
}

function parseScanOutput(value: unknown): ForeignScanOutput {
  const record = asRecord(value);
  const rawSessions = asArray(record["sessions"]);
  const sessions = rawSessions
    .map(parseSession)
    .filter((s): s is ForeignSession => s !== undefined);
  const rawSkipped = asRecord(record["skipped"]);
  const rawMissing = asArray(record["missing_sources"]);

  return {
    sessions,
    scanned_dirs: asNumber(record["scanned_dirs"]),
    scanned_files: asNumber(record["scanned_files"]),
    skipped: {
      malformed: asNumber(rawSkipped["malformed"]),
      unreadable: asNumber(rawSkipped["unreadable"]),
      symlinked: asNumber(rawSkipped["symlinked"]),
    },
    oversized: asNumber(record["oversized"]),
    missing_sources: rawMissing.filter((m): m is string => typeof m === "string"),
    scan_truncated: asBool(record["scan_truncated"]),
    read_budget_exhausted: asBool(record["read_budget_exhausted"]),
  };
}

type ScanResult =
  | { readonly kind: "ok"; readonly output: ForeignScanOutput }
  | { readonly kind: "refusal"; readonly refusal: ForeignScanRefusal }
  | { readonly kind: "error"; readonly message: string };

function normalizeScanResult(value: unknown): ScanResult {
  const record = asRecord(value);

  if (record["refusal"] !== undefined) {
    const refusal = asRecord(record["refusal"]);
    const code = asString(refusal["code"]);
    const reason = asString(refusal["reason"]);
    if (code.length > 0 && reason.length > 0) {
      return { kind: "refusal", refusal: { code, reason } };
    }
    return { kind: "error", message: "The scanner returned a malformed refusal." };
  }

  if (record["ok"] !== undefined) {
    return { kind: "ok", output: parseScanOutput(record["ok"]) };
  }

  return { kind: "error", message: "The scanner returned an unrecognised packet." };
}

function buildPacket(
  deps: ForeignResumeDeps,
  options: ForeignResumeOptions,
): Record<string, unknown> {
  return {
    now_ms: deps.now_ms,
    cwd_filter: deps.cwd,
    max_age_days: options.max_age_days ?? DEFAULT_MAX_AGE_DAYS,
    limit: options.limit ?? DEFAULT_PICKER_LIMIT,
  };
}

export function formatAge(mtime_ms: number, now_ms: number): string {
  const diff = Math.max(0, now_ms - mtime_ms);
  const days = Math.floor(diff / DAY_MS);
  if (days >= 1) {
    return `${String(days)} day${days === 1 ? "" : "s"} ago`;
  }
  const hours = Math.floor(diff / HOUR_MS);
  if (hours >= 1) {
    return `${String(hours)} hour${hours === 1 ? "" : "s"} ago`;
  }
  return "just now";
}

function resumeCommand(session: ForeignSession): string {
  const id = session.session_id;
  if (session.source === "claude") {
    if (session.cwd !== undefined && session.cwd.length > 0) {
      return `cd "${session.cwd}" && claude --resume ${id}`;
    }
    return `claude --resume ${id}`;
  }
  if (session.cwd !== undefined && session.cwd.length > 0) {
    return `cd "${session.cwd}" && codex resume ${id}`;
  }
  return `codex resume ${id}`;
}

function describeSession(session: ForeignSession, now_ms: number): string {
  const age = formatAge(session.mtime_ms, now_ms);
  const records =
    session.record_count === undefined ? "metadata only" : `${String(session.record_count)} records`;
  const truncated = session.metadata_truncated ? " · truncated" : "";
  const cwd = session.cwd ?? "(cwd unknown)";
  return `${session.source.padEnd(6)}  ${session.session_id}  ${cwd}  ${age}  ${records}${truncated}`;
}

function describeList(
  output: ForeignScanOutput,
  now_ms: number,
  cwd: string,
): string {
  const sessions = output.sessions;
  const header = `Recent foreign sessions for this directory (${cwd}):`;

  if (sessions.length === 0) {
    const reasons: string[] = [];
    if (output.missing_sources.length > 0) {
      reasons.push(
        `the scanner could not read the ${output.missing_sources.join(" or ")} state store`,
      );
    }
    if (output.scan_truncated) {
      reasons.push("the scan was truncated");
    }
    if (output.read_budget_exhausted) {
      reasons.push("the file-read budget was exhausted");
    }
    const reason = reasons.length > 0 ? ` (${reasons.join("; ")})` : "";
    return `${header}\n\nNo recent foreign sessions were found${reason}.`;
  }

  const lines = sessions
    .map((session, index) => `  ${String(index + 1).padStart(2)}. ${describeSession(session, now_ms)}`)
    .join("\n");

  const notes: string[] = [];
  if (output.scan_truncated) {
    notes.push("The scan hit a bound and may be partial.");
  }
  if (output.read_budget_exhausted) {
    notes.push("The file-read budget was exhausted; some sessions may be metadata-only.");
  }
  const note = notes.length > 0 ? `\n\n${notes.join(" ")}` : "";

  return `${header}\n\n${lines}\n\nRun /resume <number> to see the resume command for that session.${note}`;
}

function describeSelection(session: ForeignSession, now_ms: number): string {
  const age = formatAge(session.mtime_ms, now_ms);
  return [
    "Resume context:",
    `  source:      ${session.source}`,
    `  session id:  ${session.session_id}`,
    `  cwd:         ${session.cwd ?? "(unknown)"}`,
    `  age:         ${age}`,
    `  records:     ${session.record_count === undefined ? "(unknown)" : String(session.record_count)}`,
    session.metadata_truncated ? "  metadata:    truncated" : undefined,
    "",
    "Run this to resume in the foreign tool:",
    `  ${resumeCommand(session)}`,
  ]
    .filter((line): line is string => line !== undefined)
    .join("\n");
}

/**
 * Run one `/resume` turn: ask the scanner, then either list sessions or
 * describe the selected one. Returns a single notice string.
 *
 * The `invoke` seam is given the packet the real plugin expects:
 * `now_ms`, `cwd_filter`, `max_age_days`, `limit`. Sources default to both.
 */
export async function runForeignResume(
  deps: ForeignResumeDeps,
  invoke: ForeignResumeInvoke,
  options?: ForeignResumeOptions,
): Promise<string> {
  const packet = buildPacket(deps, options ?? {});

  let raw: unknown;
  try {
    raw = await invoke(packet);
  } catch (cause) {
    return `The scanner could not run: ${cause instanceof Error ? cause.message : String(cause)}`;
  }

  const result = normalizeScanResult(raw);

  if (result.kind === "error") {
    return result.message;
  }

  if (result.kind === "refusal") {
    return `The scanner refused (${result.refusal.code}): ${result.refusal.reason}`;
  }

  const output = result.output;
  const sessions = output.sessions;

  if (deps.selection !== undefined) {
    if (deps.selection < 1 || deps.selection > sessions.length) {
      return `There is no session at ${String(deps.selection)}.${
        sessions.length === 0
          ? ""
          : ` Choose a number from 1 to ${String(sessions.length)}.`
      }\n\n${describeList(output, deps.now_ms, deps.cwd)}`;
    }
    return describeSelection(sessions[deps.selection - 1]!, deps.now_ms);
  }

  return describeList(output, deps.now_ms, deps.cwd);
}
