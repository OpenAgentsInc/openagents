// QA-2 (#8907): durable Observer check registry.
//
// The Observer EXECUTES checks against live production surfaces instead of
// narrating what an agent read once (docs/transcripts/252*.md — "Observer
// reveals hidden stuff"). Each check is a typed record:
//   { id, surface, probe, expectation, cadence, severityOnDrift }
// executed by scripts/qa-observer.ts on a cadence, producing dated results
// artifacts under docs/qa/observer/results/.
//
// Honesty contract: a check that cannot run reports `unrunnable` with the
// reason (e.g. an admin-gated probe with no token in the environment). It
// never silently passes.

/** How severe drift on this check is. `high`+ makes the executor exit 1. */
export type ObserverSeverity = "low" | "medium" | "high" | "critical";

export const OBSERVER_SEVERITIES: readonly ObserverSeverity[] = [
  "low",
  "medium",
  "high",
  "critical",
];

/**
 * A probe is either an HTTP GET against the production API or a repo-local
 * command. `bearerEnv` names an env var holding a bearer token; when the var
 * is absent the check is `unrunnable` (never a silent pass, never a fake
 * drift). The token value itself is never logged or written to artifacts.
 */
export type ObserverProbe =
  | Readonly<{
      kind: "http";
      method: "GET";
      /** Path relative to the executor's base URL (default production). */
      path: string;
      /** Env var holding a bearer token; absent env => unrunnable. */
      bearerEnv?: string;
    }>
  | Readonly<{
      kind: "command";
      /** argv, executed from the repo root. Exit 0 => probe ran clean. */
      command: readonly string[];
    }>;

/**
 * Machine-evaluable expectation rules. `path` is a dot path into the parsed
 * JSON body (HTTP) or parsed JSON stdout (command).
 */
export type ExpectationRule =
  | Readonly<{ kind: "number_gt"; path: string; value: number }>
  | Readonly<{ kind: "timestamp_within_ms"; path: string; maxAgeMs: number }>
  | Readonly<{ kind: "array_non_empty"; path: string }>
  | Readonly<{ kind: "string_equals"; path: string; value: string }>
  | Readonly<{
      kind: "field_type";
      path: string;
      type: "string" | "number" | "boolean";
    }>
  | Readonly<{
      kind: "every_item_has_keys";
      path: string;
      keys: readonly string[];
    }>;

export const EXPECTATION_RULE_KINDS: readonly ExpectationRule["kind"][] = [
  "number_gt",
  "timestamp_within_ms",
  "array_non_empty",
  "string_equals",
  "field_type",
  "every_item_has_keys",
];

export type ObserverExpectation = Readonly<{
  /** Human sentence describing what "green" means for this surface. */
  description: string;
  rules: readonly ExpectationRule[];
}>;

export type ObserverCheck = Readonly<{
  /** Stable unique id, also the GitHub-issue drift marker. */
  id: string;
  /** The product surface under observation. */
  surface: string;
  probe: ObserverProbe;
  expectation: ObserverExpectation;
  /** How often the check is due, e.g. "15m", "1h", "6h", "24h". */
  cadence: string;
  severityOnDrift: ObserverSeverity;
  /** Honest caveats, e.g. admin gating. */
  notes?: string;
}>;

/** Parse a cadence like "15m" / "1h" / "24h" / "30s" into milliseconds. */
export const parseCadenceMs = (cadence: string): number | undefined => {
  const match = /^([1-9][0-9]*)(s|m|h|d)$/.exec(cadence);
  if (match === null) return undefined;
  const value = Number(match[1]);
  const unit = match[2];
  const unitMs =
    unit === "s" ? 1_000 : unit === "m" ? 60_000 : unit === "h" ? 3_600_000 : 86_400_000;
  return value * unitMs;
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((item) => typeof item === "string");

const probeProblems = (probe: unknown, ref: string): string[] => {
  if (!isRecord(probe)) return [`${ref}: probe must be an object`];
  if (probe.kind === "http") {
    const problems: string[] = [];
    if (probe.method !== "GET") problems.push(`${ref}: http probe method must be "GET"`);
    if (typeof probe.path !== "string" || !probe.path.startsWith("/")) {
      problems.push(`${ref}: http probe path must be a string starting with "/"`);
    }
    if (probe.bearerEnv !== undefined && typeof probe.bearerEnv !== "string") {
      problems.push(`${ref}: http probe bearerEnv must be a string when present`);
    }
    return problems;
  }
  if (probe.kind === "command") {
    return isStringArray(probe.command) && probe.command.length > 0
      ? []
      : [`${ref}: command probe requires a non-empty string argv`];
  }
  return [`${ref}: probe kind must be "http" or "command"`];
};

const ruleProblems = (rule: unknown, ref: string): string[] => {
  if (!isRecord(rule)) return [`${ref}: rule must be an object`];
  if (
    typeof rule.kind !== "string" ||
    !(EXPECTATION_RULE_KINDS as readonly string[]).includes(rule.kind)
  ) {
    return [`${ref}: unknown rule kind ${JSON.stringify(rule.kind)}`];
  }
  if (typeof rule.path !== "string" || rule.path === "") {
    return [`${ref}: rule path must be a non-empty string`];
  }
  switch (rule.kind) {
    case "number_gt":
      return typeof rule.value === "number" ? [] : [`${ref}: number_gt requires a numeric value`];
    case "timestamp_within_ms":
      return typeof rule.maxAgeMs === "number" && rule.maxAgeMs > 0
        ? []
        : [`${ref}: timestamp_within_ms requires a positive maxAgeMs`];
    case "string_equals":
      return typeof rule.value === "string"
        ? []
        : [`${ref}: string_equals requires a string value`];
    case "field_type":
      return rule.type === "string" || rule.type === "number" || rule.type === "boolean"
        ? []
        : [`${ref}: field_type requires type string|number|boolean`];
    case "every_item_has_keys":
      return isStringArray(rule.keys) && rule.keys.length > 0
        ? []
        : [`${ref}: every_item_has_keys requires non-empty string keys`];
    default:
      return [];
  }
};

/**
 * Decode an unknown value into the registry shape. Returns the typed checks
 * or the full list of problems — never a partially-valid registry.
 */
export const decodeObserverRegistry = (
  value: unknown,
): { checks: readonly ObserverCheck[] } | { problems: readonly string[] } => {
  if (!Array.isArray(value)) return { problems: ["registry must be an array of checks"] };
  const problems: string[] = [];
  const seenIds = new Set<string>();
  for (const [index, check] of value.entries()) {
    const ref =
      isRecord(check) && typeof check.id === "string" ? `check ${check.id}` : `check[${index}]`;
    if (!isRecord(check)) {
      problems.push(`${ref}: must be an object`);
      continue;
    }
    if (typeof check.id !== "string" || check.id === "") {
      problems.push(`${ref}: id must be a non-empty string`);
    } else if (seenIds.has(check.id)) {
      problems.push(`${ref}: duplicate id`);
    } else {
      seenIds.add(check.id);
    }
    if (typeof check.surface !== "string" || check.surface === "") {
      problems.push(`${ref}: surface must be a non-empty string`);
    }
    problems.push(...probeProblems(check.probe, ref));
    if (!isRecord(check.expectation) || typeof check.expectation.description !== "string") {
      problems.push(`${ref}: expectation requires a description`);
    } else if (!Array.isArray(check.expectation.rules) || check.expectation.rules.length === 0) {
      problems.push(`${ref}: expectation requires at least one rule`);
    } else {
      for (const [ruleIndex, rule] of check.expectation.rules.entries()) {
        problems.push(...ruleProblems(rule, `${ref} rule[${ruleIndex}]`));
      }
    }
    if (typeof check.cadence !== "string" || parseCadenceMs(check.cadence) === undefined) {
      problems.push(`${ref}: cadence must parse as <n>(s|m|h|d)`);
    }
    if (
      typeof check.severityOnDrift !== "string" ||
      !(OBSERVER_SEVERITIES as readonly string[]).includes(check.severityOnDrift)
    ) {
      problems.push(`${ref}: severityOnDrift must be one of ${OBSERVER_SEVERITIES.join("|")}`);
    }
    if (check.notes !== undefined && typeof check.notes !== "string") {
      problems.push(`${ref}: notes must be a string when present`);
    }
  }
  return problems.length > 0 ? { problems } : { checks: value as readonly ObserverCheck[] };
};

/**
 * The seed registry (issue #8907). All public checks run unauthenticated
 * against production. The khala-sync capture-health liveness surface
 * (`changelog last_version` vs `capture checkpoint pushed_through_version`,
 * #8556) is admin-bearer-gated — there is no public probeable equivalent —
 * so that check is honestly `unrunnable` unless OPENAGENTS_ADMIN_API_TOKEN
 * is present in the executor's environment.
 */
export const OBSERVER_CHECK_REGISTRY: readonly ObserverCheck[] = [
  {
    cadence: "15m",
    expectation: {
      description: "tokensServed is a positive number and generatedAt is fresh (<= 15m old)",
      rules: [
        { kind: "number_gt", path: "tokensServed", value: 0 },
        { kind: "timestamp_within_ms", maxAgeMs: 15 * 60_000, path: "generatedAt" },
      ],
    },
    id: "public.khala_tokens_served",
    probe: { kind: "http", method: "GET", path: "/api/public/khala-tokens-served" },
    severityOnDrift: "high",
    surface: "https://openagents.com/api/public/khala-tokens-served",
  },
  {
    cadence: "1h",
    expectation: {
      description: "30d daily history series is non-empty and every bucket has day + tokensServed",
      rules: [
        { kind: "array_non_empty", path: "series" },
        { keys: ["day", "tokensServed"], kind: "every_item_has_keys", path: "series" },
      ],
    },
    id: "public.khala_tokens_served_history",
    probe: { kind: "http", method: "GET", path: "/api/public/khala-tokens-served/history" },
    severityOnDrift: "high",
    surface: "https://openagents.com/api/public/khala-tokens-served/history",
  },
  {
    cadence: "1h",
    expectation: {
      description: "model mix has non-empty groups and positive totalTokens",
      rules: [
        { kind: "array_non_empty", path: "groups" },
        { kind: "number_gt", path: "totalTokens", value: 0 },
      ],
    },
    id: "public.khala_model_mix",
    probe: { kind: "http", method: "GET", path: "/api/public/khala-tokens-served/model-mix" },
    severityOnDrift: "medium",
    surface: "https://openagents.com/api/public/khala-tokens-served/model-mix",
  },
  {
    cadence: "1h",
    expectation: {
      description: "channel mix has non-empty groups and positive totalTokens",
      rules: [
        { kind: "array_non_empty", path: "groups" },
        { kind: "number_gt", path: "totalTokens", value: 0 },
      ],
    },
    id: "public.khala_channel_mix",
    probe: { kind: "http", method: "GET", path: "/api/public/khala-tokens-served/channel-mix" },
    severityOnDrift: "medium",
    surface: "https://openagents.com/api/public/khala-tokens-served/channel-mix",
  },
  {
    cadence: "15m",
    expectation: {
      description:
        "pylon-stats projection decodes: available boolean, status string, fresh generatedAtUnixMs number",
      rules: [
        { kind: "field_type", path: "available", type: "boolean" },
        { kind: "field_type", path: "status", type: "string" },
        { kind: "field_type", path: "generatedAtUnixMs", type: "number" },
        { kind: "timestamp_within_ms", maxAgeMs: 15 * 60_000, path: "generatedAtUnixMs" },
      ],
    },
    id: "public.pylon_stats",
    probe: { kind: "http", method: "GET", path: "/api/public/pylon-stats" },
    severityOnDrift: "medium",
    surface: "https://openagents.com/api/public/pylon-stats",
  },
  {
    cadence: "1h",
    expectation: {
      description:
        "forum launch-status decodes: non-empty gates each carrying id/label/severity/state",
      rules: [
        { kind: "array_non_empty", path: "gates" },
        { keys: ["id", "label", "severity", "state"], kind: "every_item_has_keys", path: "gates" },
      ],
    },
    id: "forum.launch_status",
    probe: { kind: "http", method: "GET", path: "/api/forum/launch-status" },
    severityOnDrift: "medium",
    surface: "https://openagents.com/api/forum/launch-status",
  },
  {
    cadence: "15m",
    expectation: {
      description:
        'capture-health reports status "healthy" (backlog draining; checkpoints advancing per #8556)',
      rules: [{ kind: "string_equals", path: "status", value: "healthy" }],
    },
    id: "khala_sync.capture_health",
    notes:
      "Admin-bearer-gated; there is no public probeable changelog-vs-checkpoint surface. " +
      "Unrunnable without OPENAGENTS_ADMIN_API_TOKEN in the executor environment.",
    probe: {
      bearerEnv: "OPENAGENTS_ADMIN_API_TOKEN",
      kind: "http",
      method: "GET",
      path: "/api/internal/khala-sync/capture-health",
    },
    severityOnDrift: "critical",
    surface: "https://openagents.com/api/internal/khala-sync/capture-health",
  },
];
