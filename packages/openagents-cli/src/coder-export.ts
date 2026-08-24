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
 */

import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import type { CoderEntry, CoderSnapshot } from "./coder-session.js";

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
  tool_calls?: ReadonlyArray<{
    tool_call_id: string;
    function_name: string;
    arguments: Record<string, unknown>;
  }>;
  observation?: { results: ReadonlyArray<{ source_call_id: string; content: string }> };
}

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

/** Fold the transcript into ATIF steps. */
const stepsOf = (entries: ReadonlyArray<CoderEntry>, model: string): ReadonlyArray<AtifStep> => {
  const steps: AtifStep[] = [];
  /** Reasoning arrives before the turn it belongs to and attaches to it. */
  let pendingReasoning: string | undefined;

  for (const entry of entries) {
    const timestamp = new Date(entry.at).toISOString();

    if (entry.role === "notice") continue;

    if (entry.role === "reasoning") {
      pendingReasoning = pendingReasoning === undefined ? entry.text : `${pendingReasoning}\n${entry.text}`;
      continue;
    }

    if (entry.role === "you") {
      if (INTERFACE_COMMANDS.test(entry.text.trim())) continue;
      steps.push({ step_id: steps.length + 1, timestamp, source: "user", message: entry.text });
      continue;
    }

    if (entry.role === "tool" && entry.tool !== undefined) {
      const { callId, name, arguments: args, output, error } = entry.tool;
      steps.push({
        step_id: steps.length + 1,
        timestamp,
        source: "agent",
        message: "",
        model_name: model,
        ...(pendingReasoning === undefined ? {} : { reasoning_content: pendingReasoning }),
        tool_calls: [
          { tool_call_id: callId, function_name: name, arguments: argumentsOf(args) },
        ],
        observation: {
          results: [{ source_call_id: callId, content: error ?? output ?? "" }],
        },
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
      });
      pendingReasoning = undefined;
    }
  }

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
  },
): ExportedTrajectory {
  const at = options.now ?? new Date();
  const directory = options.directory ?? join(homedir(), ".openagents", "exports");
  const steps = stepsOf(snapshot.entries, options.model);

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
    final_metrics: { total_steps: steps.length },
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

  return { path, copied: copyToClipboard(path), steps: steps.length };
}
