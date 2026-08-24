/**
 * Rendering for a fleet of delegated children.
 *
 * This module turns tasks into rows and returns text. It writes no escapes and
 * touches no terminal, so the full-screen interface, `--plain`, and the
 * headless `openagents delegate` command all show the same fleet and cannot
 * disagree about it.
 *
 * The shape follows what a reader can actually use at each density:
 *
 * - One phrase for the status line, because a status line has one row no matter
 *   how many children are running.
 * - One row per child, because a child's whole state is its description, what
 *   it is doing now, and what it cost.
 * - The counters disappear once a child is done, because `Done` plus a total is
 *   the answer and a live tool count is not.
 *
 * A row never wraps. A fleet of fifteen children that wraps is thirty rows of
 * ragged text, and the transcript underneath it disappears.
 */

import type { CoderTask, CoderTaskStatus } from "./coder-tasks.js";
import { isTerminal } from "./coder-tasks.js";

/** One rendered child. The caller decides how `status` is coloured. */
export interface FleetRow {
  readonly status: CoderTaskStatus;
  /** `├─` for every child but the last, which gets `└─`. */
  readonly branch: string;
  readonly mark: string;
  readonly text: string;
}

const MARKS: Record<CoderTaskStatus, string> = {
  pending: "·",
  running: "◐",
  completed: "✓",
  failed: "✗",
  stopped: "■",
};

/**
 * The one-line summary for the status bar, or undefined when there is no fleet.
 *
 * Running children come first because they are what the reader is waiting on,
 * and the terminal counts follow only when there are any, so a plain fan-out
 * does not carry a trail of zeroes.
 */
export function fleetPhrase(tasks: ReadonlyArray<CoderTask>): string | undefined {
  if (tasks.length === 0) return undefined;

  const active = tasks.filter((task) => !isTerminal(task.status));
  const done = tasks.filter((task) => task.status === "completed");
  const failed = tasks.filter((task) => task.status === "failed");
  const unread = tasks.filter((task) => task.unread);

  const parts: string[] = [];
  if (active.length > 0) {
    parts.push(`${String(active.length)} ${active.length === 1 ? "agent" : "agents"}`);
  }
  if (done.length > 0) parts.push(`${String(done.length)} done`);
  if (failed.length > 0) parts.push(`${String(failed.length)} failed`);
  if (unread.length > 0) parts.push(`${String(unread.length)} unread`);
  if (parts.length === 0) return `${String(tasks.length)} agents finished`;
  return parts.join(" · ");
}

/**
 * What a child is doing, in one phrase.
 *
 * Three cases and no more, which is the whole reason a fleet of fifteen is
 * readable: it is either working on something, or it is done, or it went
 * wrong. A running child with no activity yet says `Initializing…` rather than
 * nothing, because an empty cell reads as a stalled child.
 */
export function taskActivity(task: CoderTask): string {
  if (task.status === "pending") return "Queued";
  if (task.status === "stopped") return "Stopped";
  if (task.status === "failed") return `Failed: ${collapse(task.error ?? "unknown error")}`;
  if (task.status === "completed") {
    const cost = [
      `${String(task.progress.toolUseCount)} ${task.progress.toolUseCount === 1 ? "tool use" : "tool uses"}`,
      `${formatTokens(task.progress.tokenCount)} tokens`,
      formatDuration((task.endedAt ?? task.startedAt) - task.startedAt),
    ].join(" · ");
    return `Done (${cost})`;
  }

  const activity = task.progress.lastActivity;
  if (activity === undefined) return "Initializing…";
  const target = activity.target === undefined ? "" : `(${collapse(activity.target)})`;
  return `${activity.toolName}${target}`;
}

/**
 * The counters, or an empty string once they no longer tell the reader
 * anything.
 *
 * A finished child's totals are already in its `Done` phrase, so repeating them
 * beside it is noise in the column a running child needs.
 */
export function taskCounters(task: CoderTask): string {
  if (isTerminal(task.status)) return "";
  const tools = task.progress.toolUseCount;
  const tokens = task.progress.tokenCount;
  const parts: Array<string> = [];
  if (tools > 0) parts.push(`${String(tools)} ${tools === 1 ? "tool" : "tools"}`);
  // Usage is omitted until the harness has reported some, because a bare `0`
  // in a counter column reads as a missing number rather than as "not yet".
  if (tokens > 0) parts.push(`${formatTokens(tokens)} tokens`);
  return parts.join(" · ");
}

/**
 * One row per child, in launch order.
 *
 * `width` is the room available for the row text, and every row is cut to it.
 * The description column is padded to a common width so the activity column
 * lines up, which is what lets a reader scan fifteen children for the one that
 * failed.
 */
export function fleetRows(tasks: ReadonlyArray<CoderTask>, width: number): ReadonlyArray<FleetRow> {
  const room = Math.max(20, width);
  const descriptionRoom = Math.min(28, Math.max(12, Math.floor(room * 0.35)));
  const longest = tasks.reduce((most, task) => Math.max(most, task.description.length), 0);
  const column = Math.min(descriptionRoom, longest);

  return tasks.map((task, index) => {
    const branch = index === tasks.length - 1 ? "└─" : "├─";
    const description = pad(cut(task.description, column), column);
    const counters = taskCounters(task);
    const activity = taskActivity(task);
    const tail = counters.length > 0 ? `${activity} · ${counters}` : activity;
    return {
      status: task.status,
      branch,
      mark: MARKS[task.status],
      text: cut(`${description}  ${tail}`, room),
    };
  });
}

/** The fleet as plain text, for `--plain` and for the headless command. */
export function fleetPlainLines(
  tasks: ReadonlyArray<CoderTask>,
  width: number,
): ReadonlyArray<string> {
  return fleetRows(tasks, Math.max(20, width - 6)).map(
    (row) => `  ${row.branch} ${row.mark} ${row.text}`,
  );
}

/** `8.2k` rather than `8214`, because the exact number is never the point. */
export function formatTokens(count: number): string {
  if (count < 1000) return String(count);
  if (count < 1_000_000) return `${(count / 1000).toFixed(1).replace(/\.0$/, "")}k`;
  return `${(count / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
}

export function formatDuration(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000));
  if (seconds < 60) return `${String(seconds)}s`;
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes)}m ${String(seconds % 60)}s`;
}

function collapse(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

function cut(text: string, width: number): string {
  const glyphs = [...collapse(text)];
  if (glyphs.length <= width) return glyphs.join("");
  return `${glyphs.slice(0, Math.max(1, width - 1)).join("")}…`;
}

function pad(text: string, width: number): string {
  const length = [...text].length;
  return length >= width ? text : text + " ".repeat(width - length);
}
