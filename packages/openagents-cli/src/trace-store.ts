/**
 * Local trace discovery, summarization, and redaction.
 *
 * A trace is the public-safe ATIF projection of a coding-agent session. This
 * module owns the LOCAL half of `openagents trace`: finding candidate session
 * exports on this machine, summarizing one without dumping its payloads, and
 * producing a conservatively redacted sibling copy.
 *
 * Discovery is read-only and bounded by construction. Every directory walk
 * uses `lstat`, never follows a symlink, visits a capped number of entries to
 * a capped depth, and lists a capped number of files. Foreign stores -- the
 * Claude and Codex session directories -- are metadata-only: path, size,
 * mtime, kind. Nothing here ever writes into a foreign store.
 */

import { lstatSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { BIP39_ENGLISH_WORDS } from "./memory/bip39-wordlist.js";

/** Where a candidate trace came from. */
export type TraceSourceKind =
  | "openagents_export"
  | "claude_session"
  | "codex_session"
  | "trace_path";

/** One discovered file, metadata only. */
export interface TraceCandidate {
  readonly path: string;
  readonly kind: TraceSourceKind;
  readonly bytes: number;
  readonly modified_at: string;
}

/** What a single store scan did, so bounds are visible in the output. */
export interface TraceStoreScan {
  readonly root: string;
  readonly kind: TraceSourceKind;
  readonly present: boolean;
  /** Files that matched the store's extensions, before the listing cap. */
  readonly matched: number;
  /** Files included in the listing after the cap. */
  readonly listed: number;
  /** Symlinks seen and refused. Discovery never follows one. */
  readonly skipped_symlinks: number;
  /** Whether the walk stopped at its entry budget rather than the store's end. */
  readonly truncated: boolean;
}

/** A directory to scan and how to read it. */
export interface TraceStoreSpec {
  readonly root: string;
  readonly kind: TraceSourceKind;
  readonly extensions: ReadonlyArray<string>;
}

export interface DiscoveryBounds {
  /** Directory depth below the root; 0 scans only the root's own entries. */
  readonly maxDepth: number;
  /** Most files listed per store, newest first. */
  readonly maxFilesPerStore: number;
  /** Hard budget of directory entries visited per store. */
  readonly maxScanEntries: number;
}

export const defaultDiscoveryBounds: DiscoveryBounds = {
  maxDepth: 4,
  maxFilesPerStore: 20,
  maxScanEntries: 5000,
};

/** The stores `trace list` scans when no explicit path is given. */
export const defaultTraceStores = (home: string): ReadonlyArray<TraceStoreSpec> => [
  {
    root: join(home, ".openagents", "exports"),
    kind: "openagents_export",
    extensions: [".json"],
  },
  { root: join(home, ".claude", "projects"), kind: "claude_session", extensions: [".jsonl"] },
  { root: join(home, ".codex", "sessions"), kind: "codex_session", extensions: [".jsonl"] },
];

/** A store spec for a user-supplied directory of ATIF documents. */
export const pathTraceStore = (root: string): TraceStoreSpec => ({
  root,
  kind: "trace_path",
  extensions: [".json", ".jsonl"],
});

const matchesExtension = (name: string, extensions: ReadonlyArray<string>): boolean =>
  extensions.some((extension) => name.endsWith(extension));

/**
 * Scan one store within the given bounds.
 *
 * Iterative breadth-first walk. Symlinks -- file or directory -- are counted
 * and skipped, so a link planted in a session store can neither escape it nor
 * loop it. The entry budget bounds the walk on stores of any size.
 */
export const scanTraceStore = (
  spec: TraceStoreSpec,
  bounds: DiscoveryBounds = defaultDiscoveryBounds,
): { readonly scan: TraceStoreScan; readonly candidates: ReadonlyArray<TraceCandidate> } => {
  const rootStat = (() => {
    try {
      return lstatSync(spec.root);
    } catch {
      return undefined;
    }
  })();
  if (rootStat === undefined || rootStat.isSymbolicLink() || !rootStat.isDirectory()) {
    return {
      scan: {
        root: spec.root,
        kind: spec.kind,
        present: false,
        matched: 0,
        listed: 0,
        skipped_symlinks: rootStat?.isSymbolicLink() === true ? 1 : 0,
        truncated: false,
      },
      candidates: [],
    };
  }

  const found: Array<{ path: string; bytes: number; mtimeMs: number }> = [];
  let skippedSymlinks = 0;
  let visited = 0;
  let truncated = false;
  const queue: Array<{ directory: string; depth: number }> = [{ directory: spec.root, depth: 0 }];

  while (queue.length > 0) {
    const next = queue.shift();
    if (next === undefined) break;
    const names = (() => {
      try {
        return readdirSync(next.directory);
      } catch {
        return [] as ReadonlyArray<string>;
      }
    })();
    for (const name of names) {
      if (visited >= bounds.maxScanEntries) {
        truncated = true;
        break;
      }
      visited += 1;
      const path = join(next.directory, name);
      const stat = (() => {
        try {
          return lstatSync(path);
        } catch {
          return undefined;
        }
      })();
      if (stat === undefined) continue;
      if (stat.isSymbolicLink()) {
        skippedSymlinks += 1;
        continue;
      }
      if (stat.isDirectory()) {
        if (next.depth < bounds.maxDepth) queue.push({ directory: path, depth: next.depth + 1 });
        continue;
      }
      if (stat.isFile() && matchesExtension(name, spec.extensions)) {
        found.push({ path, bytes: stat.size, mtimeMs: stat.mtimeMs });
      }
    }
    if (truncated) break;
  }

  const newestFirst = [...found].sort((a, b) => b.mtimeMs - a.mtimeMs);
  const listed = newestFirst.slice(0, bounds.maxFilesPerStore);
  return {
    scan: {
      root: spec.root,
      kind: spec.kind,
      present: true,
      matched: found.length,
      listed: listed.length,
      skipped_symlinks: skippedSymlinks,
      truncated,
    },
    candidates: listed.map((file) => ({
      path: file.path,
      kind: spec.kind,
      bytes: file.bytes,
      modified_at: new Date(file.mtimeMs).toISOString(),
    })),
  };
};

/** What `trace show` reports about one document. Payloads stay in the file. */
export interface TraceSummary {
  readonly path: string;
  readonly format: "atif" | "jsonl" | "unknown";
  readonly bytes: number;
  readonly schema_version?: string;
  readonly session_id?: string;
  readonly agent?: { readonly name?: string; readonly model?: string };
  readonly steps?: number;
  readonly steps_by_source?: Readonly<Record<string, number>>;
  readonly models?: ReadonlyArray<string>;
  readonly tool_calls?: number;
  readonly total_prompt_tokens?: number;
  readonly total_completion_tokens?: number;
  readonly first_timestamp?: string;
  readonly last_timestamp?: string;
  readonly lines?: number;
}

const asRecord = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;

const asNumber = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined;

/**
 * Summarize one trace file: counts, models, and token totals, never payloads.
 *
 * An ATIF document gets the full summary. A line-delimited session log -- the
 * shape the foreign stores hold -- gets bytes and line count only, because
 * this slice does not parse foreign formats.
 */
export const summarizeTraceFile = (path: string): TraceSummary => {
  const text = readFileSync(path, "utf8");
  const bytes = Buffer.byteLength(text, "utf8");

  const document = (() => {
    try {
      return asRecord(JSON.parse(text));
    } catch {
      return undefined;
    }
  })();

  if (document !== undefined && Array.isArray(document["steps"])) {
    const steps = document["steps"].map(asRecord).filter((step) => step !== undefined);
    const bySource: Record<string, number> = {};
    const models = new Set<string>();
    let toolCalls = 0;
    let promptTokens = 0;
    let completionTokens = 0;
    let sawTokens = false;
    for (const step of steps) {
      const source = typeof step["source"] === "string" ? step["source"] : "unknown";
      bySource[source] = (bySource[source] ?? 0) + 1;
      if (typeof step["model_name"] === "string") models.add(step["model_name"]);
      if (Array.isArray(step["tool_calls"])) toolCalls += step["tool_calls"].length;
      const metrics = asRecord(step["metrics"]);
      if (metrics !== undefined) {
        const prompt = asNumber(metrics["prompt_tokens"]);
        const completion = asNumber(metrics["completion_tokens"]);
        if (prompt !== undefined || completion !== undefined) sawTokens = true;
        promptTokens += prompt ?? 0;
        completionTokens += completion ?? 0;
      }
    }
    const agent = asRecord(document["agent"]);
    const finalMetrics = asRecord(document["final_metrics"]);
    const totalPrompt = asNumber(finalMetrics?.["total_prompt_tokens"]);
    const totalCompletion = asNumber(finalMetrics?.["total_completion_tokens"]);
    const first = steps[0];
    const last = steps[steps.length - 1];
    return {
      path,
      format: "atif",
      bytes,
      ...(typeof document["schema_version"] === "string"
        ? { schema_version: document["schema_version"] }
        : {}),
      ...(typeof document["session_id"] === "string" ? { session_id: document["session_id"] } : {}),
      ...(agent === undefined
        ? {}
        : {
            agent: {
              ...(typeof agent["name"] === "string" ? { name: agent["name"] } : {}),
              ...(typeof agent["model_name"] === "string" ? { model: agent["model_name"] } : {}),
            },
          }),
      steps: steps.length,
      steps_by_source: bySource,
      models: [...models],
      tool_calls: toolCalls,
      ...(totalPrompt !== undefined
        ? { total_prompt_tokens: totalPrompt }
        : sawTokens
          ? { total_prompt_tokens: promptTokens }
          : {}),
      ...(totalCompletion !== undefined
        ? { total_completion_tokens: totalCompletion }
        : sawTokens
          ? { total_completion_tokens: completionTokens }
          : {}),
      ...(first !== undefined && typeof first["timestamp"] === "string"
        ? { first_timestamp: first["timestamp"] }
        : {}),
      ...(last !== undefined && typeof last["timestamp"] === "string"
        ? { last_timestamp: last["timestamp"] }
        : {}),
    };
  }

  if (path.endsWith(".jsonl")) {
    const lines = text.split("\n").filter((line) => line.trim().length > 0).length;
    return { path, format: "jsonl", bytes, lines };
  }

  return { path, format: "unknown", bytes };
};

/**
 * One redaction rule: a category name, a global pattern, and its replacement.
 *
 * The list is data so a test can plant a fake secret per category and assert
 * both that the secret is gone and that the count names the category. Order
 * matters: specific shapes run before broad ones so a bearer token is counted
 * as a bearer token, not as an environment value.
 */
export interface RedactionRule {
  readonly category: string;
  readonly pattern: RegExp;
  readonly replacement: string;
  /**
   * A rule that has to look at what it matched before deciding. It returns the
   * text to substitute, or the match unchanged to decline. Only the seed-phrase
   * rule needs this: a 12-word run is a cheap shape to find and an expensive one
   * to guess at, so the wordlist decides rather than the regex.
   */
  readonly resolve?: (match: string) => string;
}

const escapeForRegExp = (value: string): string => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Word counts a BIP-39 mnemonic can have; below twelve is prose, not a seed. */
const MIN_SEED_WORDS = 12;

/**
 * A run of short lowercase words the length of a mnemonic. This finds candidates
 * cheaply; {@link seedPhraseResolve} confirms against the word list before
 * anything is removed, so ordinary English is left alone.
 */
const SEED_PHRASE_SHAPE = /\b(?:[a-z]{3,8} ){11}[a-z]{3,8}(?:(?: [a-z]{3,8}){3})*\b/g;

/**
 * Redact the longest run of consecutive BIP-39 words inside a shape match, and
 * only when that run is a whole mnemonic. Surrounding prose survives, which is
 * what keeps this rule usable on a real session log.
 */
const seedPhraseResolve = (match: string): string => {
  const words = match.split(" ");
  let bestStart = -1;
  let bestLength = 0;
  let runStart = 0;
  let runLength = 0;
  for (let index = 0; index < words.length; index += 1) {
    if (BIP39_ENGLISH_WORDS.has(words[index] as string)) {
      if (runLength === 0) runStart = index;
      runLength += 1;
      if (runLength > bestLength) {
        bestLength = runLength;
        bestStart = runStart;
      }
    } else {
      runLength = 0;
    }
  }
  if (bestLength < MIN_SEED_WORDS) return match;
  return [
    words.slice(0, bestStart).join(" "),
    "[REDACTED:seed_phrase]",
    words.slice(bestStart + bestLength).join(" "),
  ]
    .filter((part) => part !== "")
    .join(" ");
};

/** The conservative rule set. `home` scopes the path rules to this machine. */
export const redactionRules = (home: string): ReadonlyArray<RedactionRule> => [
  {
    // The CLI now keeps one seed phrase per machine and tells people to write
    // it down, so a phrase pasted into a session is a shape this promise has to
    // cover. `npub` is deliberately not here: it is the public name.
    category: "seed_phrase",
    pattern: SEED_PHRASE_SHAPE,
    replacement: "[REDACTED:seed_phrase]",
    resolve: seedPhraseResolve,
  },
  {
    category: "private_key",
    pattern:
      /\b(?:nsec1[02-9ac-hj-np-z]{50,}|(?:xprv|yprv|zprv|tprv|uprv|vprv)[1-9A-HJ-NP-Za-km-z]{50,})\b/g,
    replacement: "[REDACTED:private_key]",
  },
  {
    category: "bearer_token",
    pattern: /\b[Bb]earer\s+[A-Za-z0-9._~+/=-]{8,}/g,
    replacement: "Bearer [REDACTED:bearer_token]",
  },
  {
    category: "api_key",
    pattern:
      /\b(?:sk-[A-Za-z0-9_-]{16,}|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|gho_[A-Za-z0-9]{20,}|glpat-[A-Za-z0-9_-]{16,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[A-Z0-9]{16}|AIza[A-Za-z0-9_-]{30,})\b/g,
    replacement: "[REDACTED:api_key]",
  },
  {
    category: "jwt",
    pattern: /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b/g,
    replacement: "[REDACTED:jwt]",
  },
  {
    category: "secret_field",
    pattern:
      /("[\w.-]*(?:token|secret|password|passwd|api[_-]?key|credential|private[_-]?key)[\w.-]*"\s*:\s*)"(?:[^"\\]|\\.)*"/gi,
    replacement: '$1"[REDACTED:secret_field]"',
  },
  {
    category: "env_value",
    pattern: /\b([A-Z][A-Z0-9_]{2,})=(["'])(?:(?!\2)[^\n]){4,}?\2/g,
    replacement: "$1=[REDACTED:env_value]",
  },
  {
    category: "env_value",
    pattern: /\b([A-Z][A-Z0-9_]{2,})=(?!\[REDACTED)[^\s"'`\\,}]{4,}/g,
    replacement: "$1=[REDACTED:env_value]",
  },
  {
    category: "home_path",
    pattern: new RegExp(escapeForRegExp(home), "g"),
    replacement: "~",
  },
  {
    category: "home_path",
    pattern: /(?:\/Users|\/home)\/[A-Za-z0-9._-]+/g,
    replacement: "~",
  },
];

export interface RedactionResult {
  readonly text: string;
  /** Matches per category. Counts only; the matched text is never returned. */
  readonly counts: Readonly<Record<string, number>>;
  readonly total: number;
}

/** Apply the rules to a text and count what each category removed. */
export const redactText = (text: string, rules: ReadonlyArray<RedactionRule>): RedactionResult => {
  const counts: Record<string, number> = {};
  let output = text;
  let total = 0;
  for (const rule of rules) {
    let matched = 0;
    output = output.replace(rule.pattern, (...args) => {
      const match = args[0] as string;
      if (rule.resolve !== undefined) {
        const resolved = rule.resolve(match);
        // A rule that declines has not redacted anything, so it must not count.
        if (resolved === match) return match;
        matched += 1;
        return resolved;
      }
      matched += 1;
      // Rebuild the replacement's capture references by hand: the replacement
      // string is data, and String.replace only expands `$1` for literal
      // second arguments.
      return rule.replacement.replace(/\$(\d)/g, (_, index: string) => {
        const capture = args[Number(index)] as unknown;
        return typeof capture === "string" ? capture : "";
      });
    });
    if (matched > 0) {
      counts[rule.category] = (counts[rule.category] ?? 0) + matched;
      total += matched;
    }
  }
  return { text: output, counts, total };
};

export interface RedactedFile {
  readonly input: string;
  readonly output: string;
  readonly counts: Readonly<Record<string, number>>;
  readonly total: number;
  /** Whether the redacted output still parses, when the input parsed. */
  readonly valid_json: boolean | null;
}

/** The sibling path a redaction writes: `foo.json` becomes `foo.redacted.json`. */
export const redactedPathFor = (path: string): string =>
  path.endsWith(".jsonl")
    ? `${path.slice(0, -".jsonl".length)}.redacted.jsonl`
    : path.endsWith(".json")
      ? `${path.slice(0, -".json".length)}.redacted.json`
      : `${path}.redacted.json`;

/** Redact one file into its sibling and report by count only. */
export const redactTraceFile = (path: string, home: string): RedactedFile => {
  const text = readFileSync(path, "utf8");
  const parsedBefore = (() => {
    try {
      JSON.parse(text);
      return true;
    } catch {
      return false;
    }
  })();
  const result = redactText(text, redactionRules(home));
  const output = redactedPathFor(path);
  writeFileSync(output, result.text, "utf8");
  const validJson = parsedBefore
    ? (() => {
        try {
          JSON.parse(result.text);
          return true;
        } catch {
          return false;
        }
      })()
    : null;
  return {
    input: path,
    output,
    counts: result.counts,
    total: result.total,
    valid_json: validJson,
  };
};
