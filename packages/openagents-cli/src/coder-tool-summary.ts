/**
 * A tool call in one line, for the row that names it.
 *
 * A collapsed call took three rows: the tool's name, its arguments as raw
 * JSON, and its result. The middle row is the one nobody reads as JSON —
 * `{"args":["issue","view","212","-R","OpenAgentsInc/openagents.com"]}` is a
 * command line wearing a costume, and eight of them in a row is a screen of
 * punctuation with the answers pushed off the bottom.
 *
 * So the call joins the row that names it, in the shape a person would have
 * typed: `openagents issue view 212 -R OpenAgentsInc/openagents.com`. The raw
 * arguments are still there under `ctrl+o`, which is what expanding a call is
 * for.
 *
 * Nothing here invents a summary it cannot make. A tool whose arguments have no
 * obvious subject falls back to the JSON, clipped by the caller, which is what
 * the row showed before.
 */

/** Fields worth showing whole, in the order a tool would mean them. */
const SUBJECTS = ["command", "path", "file", "pattern", "query", "name"] as const;

export const summarizeToolCall = (args: string): string => {
  const trimmed = args.trim();
  if (trimmed.length === 0) return "";

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    // Arguments still streaming in, or a tool that never sent JSON. The raw
    // text is better than nothing and the caller clips it.
    return trimmed;
  }

  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return trimmed;
  const record = parsed as Record<string, unknown>;

  // An argument vector, which is the case this exists for: the `openagents`
  // tool takes the command line as a list, and joining it back is the whole
  // translation.
  const vector = record["args"];
  if (Array.isArray(vector) && vector.every((part) => typeof part === "string")) {
    return (vector as ReadonlyArray<string>).map(quote).join(" ");
  }

  for (const key of SUBJECTS) {
    const value = record[key];
    if (typeof value === "string" && value.length > 0) return value;
  }

  // Nothing named. Keep every field, without the JSON punctuation that made
  // the row unreadable.
  const pairs = Object.entries(record).filter(([, value]) => value !== undefined);
  if (pairs.length === 0) return "";

  return pairs.map(([key, value]) => `${key}=${scalar(value)}`).join(" ");
};

/**
 * A shell-style quote, so a line with a space in it still reads as one
 * argument rather than as two.
 */
const quote = (part: string): string =>
  part.length === 0 || /[\s"']/.test(part) ? JSON.stringify(part) : part;

const scalar = (value: unknown): string => {
  if (typeof value === "string") return quote(value);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
};
