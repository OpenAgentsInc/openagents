/**
 * The standing `capability` tool and the local plugin catalog it searches.
 *
 * A session starts with exactly one `capability` tool. The model searches the
 * catalog of installed, digest-pinned plugins by describing what it needs. This
 * package has no embedding path, so the tool does not rank or filter by
 * keyword; it returns the full catalog and lets the model pick by exact name.
 *
 * When the model calls `capability` with the exact catalog name, the plugin is
 * approved, loaded, and added to the session tools. Its own schema then becomes
 * available, so every later call uses the exact catalog name as the tool name.
 */

import { appendFile, mkdir } from "node:fs/promises";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import type { CoderTool } from "./coder-tools.js";
import {
  describeLoad,
  isRefusal,
  loadPluginFromManifest,
  type LoadedPlugin,
  type PluginApproval,
  type PluginManifest,
  type PluginRefusal,
  validateManifest,
} from "./coder-plugins.js";

/** One installed, digest-pinned plugin as the catalog sees it. */
export interface PluginCatalogEntry {
  readonly name: string;
  readonly version: string;
  readonly description: string;
  readonly manifestPath: string;
  readonly artifact: { readonly path: string; readonly digest: string };
  readonly capabilities: PluginManifest["capabilities"];
}

/** A recorded request for a capability the local catalog could not satisfy. */
export interface CapabilityGap {
  /** Epoch milliseconds. */
  readonly requestedAt: number;
  readonly query?: string | undefined;
  readonly requestedName?: string | undefined;
}

export interface CapabilityOptions {
  readonly catalog: ReadonlyArray<PluginCatalogEntry>;
  readonly approval: PluginApproval;
  /**
   * Record a no-match for later registry loop consumption. Called before the
   * tool returns, so the record is not lost if the caller then fails.
   */
  readonly recordGap: (gap: CapabilityGap) => void | Promise<void>;
  /**
   * Called when a plugin is approved and loaded so the session can declare its
   * dedicated tool. The manifest path is passed along for provenance records.
   */
  readonly onSelect: (plugin: LoadedPlugin, manifestPath: string) => void;
  /** Load a manifest into a verified, digest-pinned plugin. Defaults to the host loader. */
  readonly load?: (manifestPath: string) => LoadedPlugin | PluginRefusal;
}

/**
 * Walk upward from the caller's location until a `plugins/` directory is found,
 * then read every child `manifest.json` inside it. Invalid manifests are ignored;
 * this is discovery, not verification, and verification happens at load time.
 */
export function discoverPluginCatalog(from = import.meta.url): ReadonlyArray<PluginCatalogEntry> {
  const start = from.startsWith("file:") ? fileURLToPath(from) : from;
  let here = start;
  while (true) {
    const candidate = join(here, "plugins");
    if (existsSync(candidate) && statSync(candidate).isDirectory()) {
      const found: PluginCatalogEntry[] = [];
      for (const dir of readdirSync(candidate)) {
        const manifestPath = join(candidate, dir, "manifest.json");
        if (!existsSync(manifestPath)) continue;
        let raw: string;
        try {
          raw = readFileSync(manifestPath, "utf8");
        } catch {
          continue;
        }
        let parsed: unknown;
        try {
          parsed = JSON.parse(raw);
        } catch {
          continue;
        }
        const manifest = validateManifest(parsed);
        if (isRefusal(manifest)) continue;
        found.push({
          name: manifest.name,
          version: manifest.version,
          description: manifest.description,
          manifestPath,
          artifact: manifest.artifact,
          capabilities: manifest.capabilities,
        });
      }
      return found;
    }
    const parent = dirname(here);
    if (parent === here) break;
    here = parent;
  }
  return [];
}

/**
 * A default local record of capability gaps, for later registry loop consumption.
 *
 * Writes one JSON object per line to `~/.openagents/capability-gaps.jsonl`.
 * The recorder only writes; it does not read back. Callers that need to act on
 * the record can read the file themselves.
 */
export function defaultCapabilityGapRecorder(
  path = join(homedir(), ".openagents", "capability-gaps.jsonl"),
): (gap: CapabilityGap) => Promise<void> {
  return async (gap) => {
    await mkdir(dirname(path), { recursive: true });
    await appendFile(path, `${JSON.stringify(gap)}\n`, "utf8");
  };
}

function catalogDescription(catalog: ReadonlyArray<PluginCatalogEntry>): string {
  const entries = catalog
    .map((entry) => `- \`${entry.name}\` v${entry.version}: ${entry.description}`)
    .join("\n");
  return catalog.length === 0
    ? "The local catalog is empty."
    : `Installed capabilities:\n${entries}`;
}

/**
 * The one standing tool for discovering and loading plugin capabilities.
 *
 * The description and parameters contain only the `capability` tool: no
 * installed plugin names, no plugin parameter schemas, and no catalog enum.
 * Those appear only after a search with `query` returns the catalog and an
 * exact-name call with `name` loads the chosen plugin.
 */
/** Most candidates one search returns; the rest are counted, not listed. */
const SEARCH_LIMIT = 5;

/** A description's first sentence, for a one-line candidate row. */
const firstSentence = (text: string): string => {
  const at = text.indexOf(". ");
  return at > 0 ? text.slice(0, at + 1) : text;
};

/** "; and K more" when the catalog holds more than one answer showed. */
const remainder = (beyond: number): string =>
  beyond > 0 ? `\n…and ${String(beyond)} more; search again with other words.` : "";

/**
 * Lexical retrieval over the catalog: token overlap between the query and
 * each manifest's name and description, best first, at most [`SEARCH_LIMIT`].
 *
 * Retrieval narrows candidates; it never routes. The model still invokes by
 * the exact returned name and the load still verifies the digest, so the
 * no-keyword-routing law holds. An embedding index can replace this scoring
 * without changing the surface (OpenAgentsInc/openagents#42).
 */
const searchCatalog = (
  catalog: ReadonlyArray<PluginCatalogEntry>,
  query: string,
): PluginCatalogEntry[] =>
  matchCapabilities(catalog, query)
    .slice(0, SEARCH_LIMIT)
    .map((candidate) => candidate.entry);

/**
 * Score the catalog against free text, best first.
 *
 * Exported because retrieval is the harness's job, not only the model's
 * (OpenAgentsInc/openagents#42): the session runs this over each incoming
 * message and materializes what matches, so the model sees the right tool
 * without having consulted anything.
 */
export const matchCapabilities = (
  catalog: ReadonlyArray<PluginCatalogEntry>,
  text: string,
): Array<{ readonly entry: PluginCatalogEntry; readonly hits: number }> => {
  const terms = tokens(text);
  if (terms.length === 0) return [];
  const scored = catalog
    .map((entry) => {
      const haystack = new Set(tokens(`${entry.name} ${entry.description}`));
      const hits = terms.filter((term) => haystack.has(term)).length;
      return { entry, hits };
    })
    .filter((candidate) => candidate.hits > 0);
  scored.sort((left, right) => right.hits - left.hits);
  return scored;
};

/** Lowercased word stems of three letters or more; the rest is noise. */
const tokens = (text: string): string[] =>
  text
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((word) => word.length >= 3);

export function capabilityTool(options: CapabilityOptions): CoderTool {
  const { catalog, approval, recordGap, onSelect, load = loadPluginFromManifest } = options;
  return {
    name: "capability",
    // Constant-size on purpose (OpenAgentsInc/openagents#42): the catalog is
    // searched, never enumerated here, so the standing prompt does not grow
    // as capabilities are installed.
    description:
      "Discover and load installed plugin capabilities: sandboxed, sealed programs this " +
      "machine already holds for common agent work. Before writing a script for a task, " +
      "search here first — a capability that covers it is bounded, reviewable, and returns " +
      "structured output. Call with `query` describing what you need to get the best " +
      "matches; then call again with `name` set to the exact returned name to load it and " +
      "make its dedicated tool available. Every later call to the loaded capability uses " +
      "that exact name as the tool name.",
    parameters: {
      type: "object",
      properties: {
        query: {
          type: "string",
          description:
            "Describe the capability you need. No semantic embedding is available, so the full catalog is returned.",
        },
        name: {
          type: "string",
          description:
            "Exact catalog name of the capability to load. Use the exact name from a previous `query` result.",
        },
      },
      additionalProperties: false,
    },
    run: async (args, _signal) => {
      const query = typeof args["query"] === "string" ? args["query"].trim() : undefined;
      const name = typeof args["name"] === "string" ? args["name"].trim() : undefined;

      if (name !== undefined && name.length > 0) {
        const entry = catalog.find((candidate) => candidate.name === name);
        if (entry === undefined) {
          await recordGap({ requestedAt: Date.now(), requestedName: name, query });
          return `No capability named \`${name}\` is in the local catalog.\n\n${catalogDescription(catalog)}`;
        }

        const approvalResult = await approval.check({
          name: entry.name,
          digest: entry.artifact.digest,
          capabilities: entry.capabilities,
        });
        if (isRefusal(approvalResult)) {
          return `Capability \`${name}\` was not allowed (${approvalResult.code}): ${approvalResult.reason}`;
        }

        const outcome = load(entry.manifestPath);
        if (isRefusal(outcome)) {
          return describeLoad(outcome);
        }
        onSelect(outcome, entry.manifestPath);
        // The next move is named explicitly. A model that loaded a
        // capability mid-plan kept following the plan — improvising with
        // shell — while the tool it asked for sat ready.
        return (
          describeLoad(outcome) +
          `\n\nThe tool \`${entry.name}\` is available now. Call it directly for this work ` +
          "instead of a shell script: it is sandboxed, bounded, and returns structured JSON. " +
          "Its parameters are in its tool declaration."
        );
      }

      if (query !== undefined && query.length > 0) {
        const ranked = searchCatalog(catalog, query);
        if (ranked.length === 0) {
          await recordGap({ requestedAt: Date.now(), query });
          return catalog.length === 0
            ? "No capabilities are installed on this machine."
            : "Nothing installed matches that. The full catalog:\n\n" +
                `${catalogDescription(catalog.slice(0, SEARCH_LIMIT))}` +
                remainder(catalog.length - SEARCH_LIMIT) +
                "\n\nCall `capability` with `name` set to an exact name to load it.";
        }
        return (
          "Best matches, most relevant first:\n\n" +
          ranked
            .map((entry) => `- \`${entry.name}\` — ${firstSentence(entry.description)}`)
            .join("\n") +
          remainder(catalog.length - ranked.length) +
          "\n\nCall `capability` with `name` set to the exact name you want to load."
        );
      }

      return "Provide `query` to see the catalog or `name` to load a capability by exact catalog name.";
    },
  };
}
