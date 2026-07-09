
type PromiseRecord = {
  authorityBoundary?: string;
  blockerRefs?: string[];
  claim?: string;
  promiseId?: string;
  safeCopy?: string;
  state?: string;
  unsafeCopy?: string;
};

type PromiseRegistry = {
  generatedAt?: string;
  promises?: PromiseRecord[];
  registryVersion?: string;
  schemaVersion?: string;
  states?: Record<string, string>;
  version?: string;
};

type RegistryCacheEntry = {
  expiresAt: number;
  grounding: string;
};

const REGISTRY_URL =
  process.env.PROMISE_REGISTRY_URL ??
  "https://openagents.com/api/public/product-promises";
const TTL_MS = 5 * 60 * 1000;
const MAX_COPY_LENGTH = 220;

let registryCache: RegistryCacheEntry | null = null;

function cleanLine(value: string | undefined) {
  return value?.replace(/\s+/g, " ").trim();
}

function truncate(value: string | undefined, maxLength = MAX_COPY_LENGTH) {
  const cleaned = cleanLine(value);
  if (!cleaned) return undefined;
  if (cleaned.length <= maxLength) return cleaned;
  return `${cleaned.slice(0, maxLength - 1)}...`;
}

function recordLine(record: PromiseRecord) {
  const copy = truncate(record.safeCopy ?? record.claim);
  const blockers =
    record.blockerRefs && record.blockerRefs.length > 0
      ? ` blockers=${record.blockerRefs.slice(0, 3).join(",")}`
      : "";

  return `- ${record.promiseId}: ${copy ?? "No safe public copy provided."}${blockers}`;
}

function recordsForState(records: PromiseRecord[], state: string) {
  return records
    .filter((record) => record.state === state && record.promiseId)
    .sort((left, right) =>
      (left.promiseId ?? "").localeCompare(right.promiseId ?? ""),
    );
}

function buildRegistryGrounding(registry: PromiseRegistry) {
  const records = registry.promises ?? [];
  const countByState = records.reduce<Record<string, number>>((counts, record) => {
    const state = record.state ?? "unknown";
    counts[state] = (counts[state] ?? 0) + 1;
    return counts;
  }, {});
  const stateSummary = Object.entries(countByState)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([state, count]) => `${state}:${count}`)
    .join(", ");
  const green = recordsForState(records, "green");
  const yellow = recordsForState(records, "yellow");
  const notLive = records
    .filter((record) =>
      ["planned", "red", "withdrawn", "degraded"].includes(
        record.state ?? "",
      ),
    )
    .filter((record) => record.promiseId)
    .sort((left, right) =>
      `${left.state}:${left.promiseId}`.localeCompare(
        `${right.state}:${right.promiseId}`,
      ),
    );

  return [
    "",
    "## Live Promise Registry Grounding",
    `Registry source: ${REGISTRY_URL}`,
    `Registry version: ${registry.registryVersion ?? registry.version ?? "unknown"}; schema: ${registry.schemaVersion ?? "unknown"}; generatedAt: ${registry.generatedAt ?? "unknown"}; counts: ${stateSummary || "none"}.`,
    "",
    "Claims guard:",
    "- You may describe green promise records as live only within their safeCopy and authorityBoundary.",
    "- Yellow means limited, operator-assisted, gated, or caveated. Say the caveat plainly.",
    "- Planned, red, degraded, and withdrawn records are not live sellable capabilities. Label them roadmap, blocked, degraded, or retired. Do not pitch them as available.",
    "- If a capability is not listed below or you are unsure which record applies, say OpenAgents cannot promise that yet and offer to escalate.",
    "- If asked to compare against a non-green record, mention the promiseId and the state when it helps the prospect understand the boundary.",
    "",
    "Green/live records:",
    ...green.map(recordLine),
    "",
    "Yellow/operator-assisted or limited records:",
    ...yellow.map(recordLine),
    "",
    "Not-live roadmap/blocked/retired records:",
    ...notLive.map(recordLine),
  ].join("\n");
}

function degradedGrounding(error: unknown) {
  const message = error instanceof Error ? error.message : "unknown error";

  return [
    "",
    "## Live Promise Registry Grounding",
    `Registry source: ${REGISTRY_URL}`,
    `Registry status: unavailable (${message}).`,
    "",
    "Claims guard:",
    "- The promise registry could not be fetched for this session.",
    "- Narrow claims, never widen them: do not present any unverified capability as live.",
    "- You may say OpenAgents has an AI sales employee surface in this session and can discuss the prospect's use case.",
    "- For product capabilities, pricing, roadmap, world-first, payout, settlement, inference, training, hosting, marketplace, email, or checkout claims, say you need to check the live registry or escalate to a human.",
  ].join("\n");
}

export async function getPromiseRegistryGrounding() {
  const now = Date.now();
  if (registryCache && registryCache.expiresAt > now) {
    return registryCache.grounding;
  }

  try {
    const response = await fetch(REGISTRY_URL, {
      headers: { accept: "application/json" },
    });

    if (!response.ok) {
      throw new Error(`registry fetch failed: ${response.status}`);
    }

    const registry = (await response.json()) as PromiseRegistry;
    const grounding = buildRegistryGrounding(registry);
    registryCache = { expiresAt: now + TTL_MS, grounding };
    return grounding;
  } catch (error) {
    const grounding = degradedGrounding(error);
    registryCache = { expiresAt: now + 30_000, grounding };
    return grounding;
  }
}
