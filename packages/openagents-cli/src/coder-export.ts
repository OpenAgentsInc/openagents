/**
 * Write the conversation as an ATIF trajectory.
 *
 * ATIF -- the Agent Trajectory Interchange Format -- is the shape the rest of
 * this system already speaks: `OpenAgents.DataRights.AtifExport` writes an
 * account's conversation as ATIF v1.7, and the trajectories other agents ship
 * carry the same envelope. Writing anything else here would make a coder
 * session the one thing in the workspace that cannot be read by the tools that
 * read everything else.
 *
 * The mapping is deliberately narrow. A `you` entry is a `user` step; an
 * assistant entry is an `agent` step; reasoning attaches to the step it
 * preceded; a tool entry becomes that step's `tool_calls` and `observation`.
 * Notices are the interface talking to the reader, not the model, so they are
 * not steps -- they are recorded under `extra` where a reader can still see
 * them without a consumer mistaking them for turns.
 *
 * Plugin loads are the exception that proves the rule: the notice stays a
 * notice, but the act itself changed what the agent could do, so it also
 * exports as a `source: "system"` step carrying the typed record -- which
 * plugin, which exact artifact, what bounds -- in its observation's `extra`.
 */

import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import type { CoderEntry, CoderPluginEvent, CoderSnapshot } from "./coder-session.js";

const SCHEMA_VERSION = "ATIF-v1.7";

/**
 * Lines the interface answers itself.
 *
 * They appear in the transcript because the reader typed them, but no model
 * ever saw them, so they are not steps: a trajectory that records `/export` as
 * a turn is describing the act of exporting rather than the work. `/delegate`
 * is not here, because it starts real work whose results the model does read.
 */
const INTERFACE_COMMANDS = /^\/(export|system|skills)\s*$/;
const AGENT_NAME = "openagents-coder";

/** One ATIF step, as the format defines it. */
interface AtifStep {
  step_id: number;
  timestamp: string;
  source: "system" | "user" | "agent";
  message: string;
  model_name?: string;
  reasoning_content?: string;
  metrics?: { prompt_tokens?: number; completion_tokens?: number };
  llm_call_count?: number;
  tool_calls?: ReadonlyArray<{
    tool_call_id: string;
    function_name: string;
    arguments: Record<string, unknown>;
    /** Per-call metadata the format leaves open. Plugin provenance goes here. */
    extra?: Record<string, unknown>;
  }>;
  observation?: {
    results: ReadonlyArray<{
      /** Null for a system-initiated operation, which no tool call sourced. */
      source_call_id: string | null;
      content: string;
      /** Result-level metadata the format leaves open. */
      extra?: Record<string, unknown>;
    }>;
  };
}

/**
 * The trajectory's totals.
 *
 * Summed from what the sources reported and omitted when nothing did: a
 * `total_prompt_tokens` of 0 on a session that never measured any would be a
 * measurement, and this has none to give.
 */
const sumMetrics = (
  entries: ReadonlyArray<CoderEntry>,
): { total_prompt_tokens?: number; total_completion_tokens?: number } => {
  const measured = entries.filter((entry) => entry.metrics !== undefined);
  if (measured.length === 0) return {};
  return {
    total_prompt_tokens: measured.reduce((sum, entry) => sum + (entry.metrics?.promptTokens ?? 0), 0),
    total_completion_tokens: measured.reduce(
      (sum, entry) => sum + (entry.metrics?.completionTokens ?? 0),
      0,
    ),
  };
};

export interface ExportedTrajectory {
  /** Where the file was written. */
  readonly path: string;
  /** Whether the path reached the system clipboard. */
  readonly copied: boolean;
  /** How many steps it holds. */
  readonly steps: number;
}

/** A file name that sorts by time and says what it is. */
const fileName = (repository: string, at: Date): string => {
  const stamp = at.toISOString().replace(/[:.]/g, "-").replace("Z", "Z");
  const safe = repository.replace(/[^A-Za-z0-9._-]/g, "-");
  return `${stamp}-${safe}-atif.json`;
};

/**
 * Arguments as an object, which is what ATIF's `arguments` field is.
 *
 * The interface holds them as the JSON source it renders. Source that will not
 * parse is kept rather than dropped, under a key that says it is unparsed: a
 * trajectory that silently loses what a model asked for is worse than one that
 * says it could not read it.
 */
const argumentsOf = (source: string): Record<string, unknown> => {
  try {
    const parsed = JSON.parse(source) as unknown;
    return typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : { value: parsed };
  } catch {
    return { unparsed_arguments: source };
  }
};

/** A step's metrics, present only when the source reported any. */
const metricsOf = (entry: CoderEntry): Partial<AtifStep> => {
  const metrics = entry.metrics;
  if (metrics === undefined) return {};
  const figures = {
    ...(metrics.promptTokens === undefined ? {} : { prompt_tokens: metrics.promptTokens }),
    ...(metrics.completionTokens === undefined
      ? {}
      : { completion_tokens: metrics.completionTokens }),
  };
  return {
    ...(Object.keys(figures).length === 0 ? {} : { metrics: figures }),
    ...(metrics.calls === undefined ? {} : { llm_call_count: metrics.calls }),
  };
};

/**
 * The typed half of a plugin lifecycle step, in the shape ATIF's `extra`
 * fields carry it.
 *
 * The digest is written whole. The notices truncate it for a human eye;
 * anything a machine reads gets the full `sha256:<hex>`, because a truncated
 * digest identifies nothing.
 */
const pluginEventExtra = (event: CoderPluginEvent): Record<string, unknown> => {
  const plugin = event.plugin;
  return {
    event: event.event,
    ...(event.code === undefined ? {} : { code: event.code }),
    plugin: {
      ...(plugin.name === undefined ? {} : { name: plugin.name }),
      ...(plugin.version === undefined ? {} : { version: plugin.version }),
      ...(plugin.artifactDigest === undefined ? {} : { artifact_digest: plugin.artifactDigest }),
      ...(plugin.bytes === undefined ? {} : { bytes: plugin.bytes }),
      ...(plugin.abi === undefined ? {} : { abi: plugin.abi }),
      ...(plugin.timeoutMs === undefined ? {} : { timeout_ms: plugin.timeoutMs }),
      ...(plugin.capabilities === undefined ? {} : { capabilities: plugin.capabilities }),
      manifest_path: plugin.manifestPath,
      ...(plugin.toolName === undefined ? {} : { tool_name: plugin.toolName }),
    },
  };
};

/** Fold the transcript into ATIF steps. */
const stepsOf = (
  entries: ReadonlyArray<CoderEntry>,
  pluginEvents: ReadonlyArray<CoderPluginEvent>,
  model: string,
): ReadonlyArray<AtifStep> => {
  const steps: AtifStep[] = [];
  /** Reasoning arrives before the turn it belongs to and attaches to it. */
  let pendingReasoning: string | undefined;
  /** The next plugin event still waiting for its place among the turns. */
  let nextEvent = 0;

  // A plugin load is a system-initiated capability change, and ATIF v1.5+
  // gives it a home: a `source: "system"` step whose observation carries the
  // typed record. It lands where it happened — between the turns on either
  // side of it — so a consumer replaying the steps sees the capability appear
  // before the call that used it.
  const emitEventsThrough = (at: number) => {
    while (nextEvent < pluginEvents.length) {
      const event = pluginEvents[nextEvent];
      if (event === undefined || event.at > at) break;
      steps.push({
        step_id: steps.length + 1,
        timestamp: new Date(event.at).toISOString(),
        source: "system",
        message: event.message,
        observation: {
          results: [
            { source_call_id: null, content: event.message, extra: pluginEventExtra(event) },
          ],
        },
      });
      nextEvent += 1;
    }
  };

  for (const entry of entries) {
    emitEventsThrough(entry.at);
    const timestamp = new Date(entry.at).toISOString();

    if (entry.role === "notice") continue;

    if (entry.role === "reasoning") {
      pendingReasoning =
        pendingReasoning === undefined ? entry.text : `${pendingReasoning}\n${entry.text}`;
      continue;
    }

    if (entry.role === "you") {
      if (INTERFACE_COMMANDS.test(entry.text.trim())) continue;
      steps.push({ step_id: steps.length + 1, timestamp, source: "user", message: entry.text });
      continue;
    }

    if (entry.role === "tool" && entry.tool !== undefined) {
      const { callId, name, arguments: args, output, error, plugin } = entry.tool;
      steps.push({
        step_id: steps.length + 1,
        timestamp,
        source: "agent",
        message: "",
        model_name: model,
        ...(pendingReasoning === undefined ? {} : { reasoning_content: pendingReasoning }),
        tool_calls: [
          {
            tool_call_id: callId,
            function_name: name,
            arguments: argumentsOf(args),
            // A plugin-backed call names the exact artifact that answered it,
            // digest whole, so a trace feeds usage attribution the same way a
            // thread's `tool.ran` event does.
            ...(plugin === undefined
              ? {}
              : {
                  extra: {
                    plugin: {
                      name: plugin.name,
                      version: plugin.version,
                      artifact_digest: plugin.artifactDigest,
                    },
                  },
                }),
          },
        ],
        observation: {
          results: [{ source_call_id: callId, content: error ?? output ?? "" }],
        },
        ...metricsOf(entry),
      });
      pendingReasoning = undefined;
      continue;
    }

    if (entry.role === "assistant") {
      // An assistant entry opened and never filled is the caret the interface
      // shows while a chunk is in flight, not a turn the model took.
      if (entry.text.length === 0) continue;
      steps.push({
        step_id: steps.length + 1,
        timestamp,
        source: "agent",
        message: entry.text,
        model_name: model,
        ...(pendingReasoning === undefined ? {} : { reasoning_content: pendingReasoning }),
        ...metricsOf(entry),
      });
      pendingReasoning = undefined;
    }
  }

  // A load after the last turn still happened in this session.
  emitEventsThrough(Number.POSITIVE_INFINITY);

  return steps;
};

/**
 * Put a path on the system clipboard.
 *
 * Best effort by design: a clipboard that is not there is not a reason to fail
 * an export that has already been written. The caller is told whether it
 * landed, and prints the path either way.
 */
const copyToClipboard = (text: string): boolean => {
  const candidates: ReadonlyArray<readonly [string, ReadonlyArray<string>]> =
    process.platform === "darwin"
      ? [["pbcopy", []]]
      : process.platform === "win32"
        ? [["clip", []]]
        : [
            ["wl-copy", []],
            ["xclip", ["-selection", "clipboard"]],
            ["xsel", ["--clipboard", "--input"]],
          ];

  for (const [command, args] of candidates) {
    try {
      const result = spawnSync(command, [...args], { input: text });
      if (result.status === 0) return true;
    } catch {
      continue;
    }
  }
  return false;
};

/**
 * Write the conversation as ATIF and put its path on the clipboard.
 *
 * Written under the home directory rather than the repository: a trajectory is
 * a record of what happened, not a change to the work, and it should not turn
 * up in anyone's `git status`.
 */
export function exportTrajectory(
  snapshot: CoderSnapshot,
  options: {
    readonly model: string;
    readonly toolDefinitions?: ReadonlyArray<Record<string, unknown>> | undefined;
    readonly version: string;
    readonly now?: Date | undefined;
    readonly directory?: string | undefined;
    /**
     * Whether the path goes to the system clipboard. On by default.
     *
     * Off for anything that is not a person exporting: a test that took the
     * clipboard replaced a reader's own export path with one pointing at a file
     * the test then deleted, and the reader pasted it and was told the path did
     * not exist.
     */
    readonly copy?: boolean | undefined;
  },
): ExportedTrajectory {
  const at = options.now ?? new Date();
  const directory = options.directory ?? join(homedir(), ".openagents", "exports");
  const steps = stepsOf(snapshot.entries, snapshot.pluginEvents ?? [], options.model);

  const document = {
    schema_version: SCHEMA_VERSION,
    session_id: `${snapshot.repository}-${at.toISOString()}`,
    trajectory_id: `${snapshot.repository}-${at.toISOString()}`,
    agent: {
      name: AGENT_NAME,
      version: options.version,
      model_name: options.model,
      ...(options.toolDefinitions === undefined
        ? {}
        : { tool_definitions: options.toolDefinitions }),
    },
    steps,
    final_metrics: {
      ...sumMetrics(snapshot.entries),
      total_steps: steps.length,
    },
    extra: {
      exporter: "openagents.coder.atif_export.v1",
      exported_at: at.toISOString(),
      repository: snapshot.repository,
      branch: snapshot.branch,
      // The interface's own lines. Kept because they carry refusals and
      // failures that explain the steps either side of them, and separate
      // because they were never sent to the model.
      notices: snapshot.entries
        .filter((entry) => entry.role === "notice")
        .map((entry) => ({ timestamp: new Date(entry.at).toISOString(), text: entry.text })),
    },
  };

  mkdirSync(directory, { recursive: true });
  const path = join(directory, fileName(snapshot.repository, at));
  writeFileSync(path, `${JSON.stringify(document, undefined, 2)}\n`, "utf8");

  const copy = options.copy ?? true;
  return { path, copied: copy && copyToClipboard(path), steps: steps.length };
}
