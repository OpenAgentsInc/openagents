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

import type { CoderTask, CoderTaskStatus, CoderToolActivity } from "./coder-tasks.js";
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
 * What a child is doing, in one phrase.
 *
 * Three cases and no more, which is the whole reason a fleet of fifteen is
 * readable: it is either working on something, or it is done, or it went
 * wrong. A running child with no activity yet says `Initializing…` rather than
 * nothing, because an empty cell reads as a stalled child.
 */
export function taskActivity(task: CoderTask, now: number = Date.now()): string {
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

  // A running child says how long it has been running, because the reader's
  // question is never "what is it doing" alone — it is "is this slow or is it
  // stuck". A recon child that thinks for ninety seconds before its first tool
  // call showed `Initializing…` for all ninety, which reads as stuck.
  const running = formatDuration(now - task.startedAt);
  const activity = task.progress.lastActivity;
  if (activity === undefined) return `Initializing… (${running})`;
  return `${activityPhrase(activity)} (${running})`;
}

/**
 * What an activity says on its own, with no clock and no status around it.
 *
 * The preview box wants the doing, not the duration: the duration belongs to
 * the fleet row, which already carries it, and repeating it one row down says
 * the same thing twice.
 */
export function activityPhrase(activity: CoderToolActivity): string {
  const target = activity.target === undefined ? "" : `(${collapse(activity.target)})`;
  return `${activity.toolName}${target}`;
}

/**
 * A renderer turns one tool activity into one or more bounded display rows.
 */
export type CoderActivityRenderer = (
  activity: CoderToolActivity,
  width: number,
) => ReadonlyArray<string>;

/** Show a shell or command as `name: <command>`, cut to the available width. */
const shellActivityRenderer: CoderActivityRenderer = (activity, width) => {
  if (activity.target === undefined) return [activity.toolName];
  return [cut(`${activity.toolName}: ${activity.target}`, width)];
};

/** Show a file read or write as `name: <path>:<start>-<end>` or `name: <path> (<size>)`. */
const fileActivityRenderer: CoderActivityRenderer = (activity, width) => {
  if (activity.target === undefined) return [activity.toolName];
  let body = `${activity.toolName}: ${activity.target}`;
  if (activity.meta?.range !== undefined) {
    body += `:${activity.meta.range.start}-${activity.meta.range.end}`;
  } else if (activity.meta?.size !== undefined) {
    body += ` (${activity.meta.size})`;
  }
  return [cut(body, width)];
};

/** Show a search as `name: <pattern>` plus a hit count. */
const searchActivityRenderer: CoderActivityRenderer = (activity, width) => {
  if (activity.target === undefined) return [activity.toolName];
  const hits = activity.meta?.hitCount;
  const body =
    hits === undefined
      ? `${activity.toolName}: ${activity.target}`
      : `${activity.toolName}: ${activity.target} (${hits} ${hits === 1 ? "hit" : "hits"})`;
  return [cut(body, width)];
};

/**
 * Tool-name lookup for the row renderer.
 *
 * Activity names come from the tools this session declares (shell, openagents,
 * skill, delegate) and from the child harnesses that emit them (bash, read,
 * write, edit, grep, search, repo_grep). Each name is mapped to the renderer
 * that best shows its argument.
 */
export const activityRenderers: Readonly<Record<string, CoderActivityRenderer>> = Object.freeze({
  shell: shellActivityRenderer,
  bash: shellActivityRenderer,
  read: fileActivityRenderer,
  write: fileActivityRenderer,
  edit: fileActivityRenderer,
  search: searchActivityRenderer,
  grep: searchActivityRenderer,
  repo_grep: searchActivityRenderer,
});

/**
 * The bounded, multi-row display for a single activity.
 *
 * Unregistered tools fall back to the exact one-line `activityPhrase` so the
 * fleet and the detail view stay readable for anything that shows up later.
 *
 * `taskActivity` keeps the existing one-line status phrase; surfaces that want
 * more than one row can call `activityRows` directly.
 */
export function activityRows(
  activity: CoderToolActivity,
  width: number,
): ReadonlyArray<string> {
  const renderer = activityRenderers[activity.toolName];
  if (renderer === undefined) return [activityPhrase(activity)];
  return renderer(activity, Math.max(4, width));
}

/**
 * The newest activities across a fleet, oldest first, at most `count` of them.
 *
 * Within a child, `recentActivities` is oldest to newest. Across children the
 * activities are appended in launch order, which is exact for the one-child
 * case a preview is usually watching and stable for a fan-out: no timestamp
 * exists on an activity to order by, so launch order is the only honest one.
 *
 * Terminal children are left out. Their last actions were their outcome's
 * business, and that outcome is announced on the transcript; keeping the lines
 * here would leave a preview of work nothing is doing anymore.
 */
export function latestActivities(
  tasks: ReadonlyArray<CoderTask>,
  count: number,
): ReadonlyArray<CoderToolActivity> {
  const out: CoderToolActivity[] = [];
  for (const task of tasks) {
    if (isTerminal(task.status)) continue;
    out.push(...task.progress.recentActivities);
  }
  return out.slice(Math.max(0, out.length - Math.max(0, count)));
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
export function fleetRows(
  tasks: ReadonlyArray<CoderTask>,
  width: number,
  now: number = Date.now(),
): ReadonlyArray<FleetRow> {
  const room = Math.max(20, width);
  const descriptionRoom = Math.min(28, Math.max(12, Math.floor(room * 0.35)));
  const longest = tasks.reduce((most, task) => Math.max(most, task.description.length), 0);
  const column = Math.min(descriptionRoom, longest);

  return tasks.map((task, index) => {
    const branch = index === tasks.length - 1 ? "└─" : "├─";
    const description = pad(cut(task.description, column), column);
    const counters = taskCounters(task);
    const activity = taskActivity(task, now);
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
