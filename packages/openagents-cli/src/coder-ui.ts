/**
 * The full-screen interface for `openagents coder`.
 *
 * This draws with ANSI escapes and Node's own standard streams, with no
 * rendering dependency. The design document proposes OpenTUI for this layer,
 * and OpenTUI cannot render here yet: its FFI is Bun-only (the package ships
 * `index.bun.js` beside a `.dylib` and no N-API entry point), so
 * `createCliRenderer` fails on Node with "OpenTUI native FFI is not available
 * for this runtime yet". The CLI installs as an npm package that people run on
 * Node, so the interface cannot require the Bun runtime. See `docs/2026-08-23-openagents-
 * coder-cli-spec.md` section 3 and its open questions.
 *
 * The layout:
 *
 *     ┌──────────────────────────────┐
 *     │ transcript, scrollable       │
 *     │   delegate preview inline    │
 *     │ delegate rows                │
 *     ├──────────────────────────────┤
 *     │ status line                  │
 *     │ composer                     │
 *     └──────────────────────────────┘
 *
 * Painting is differential. An earlier version cleared the whole screen and
 * repainted it several times a second while a reply streamed, which left a
 * stack of half-drawn frames wherever the terminal kept scrolled-off alternate
 * screen rows. Nothing here clears the screen, writes a newline, or moves the
 * cursor past the last row, so the terminal never scrolls and its own
 * scrollback is never written to. Scrolling the transcript is the interface's
 * own job instead.
 */

import { activityPhrase, fleetPhrase, fleetRows, latestActivities } from "./coder-fleet.js";
import { renderMarkdown, visibleWidth, wrapStyled } from "./coder-markdown.js";
import type { CoderEntry, CoderSession, CoderSnapshot, CoderToolCall } from "./coder-session.js";
import type { CoderTask, CoderTaskStatus } from "./coder-tasks.js";
import { RELOAD_EXIT_CODE, sourceCheckout } from "./coder-reload.js";
import type { SkillSelection } from "./coder-skills.js";

const ALT_SCREEN_ON = "\x1b[?1049h";
const ALT_SCREEN_OFF = "\x1b[?1049l";
const CURSOR_HIDE = "\x1b[?25l";
const CURSOR_SHOW = "\x1b[?25h";
const ERASE_LINE = "\x1b[K";
/**
 * Alternate scroll: the terminal turns the wheel into arrow keys while the
 * alternate screen is up. It costs one escape sequence and, unlike mouse
 * reporting, leaves the terminal's own text selection alone.
 */
/**
 * Ask the terminal to report modified keys unambiguously.
 *
 * Flag 1 of the keyboard protocol — disambiguate escape codes. Without it
 * shift+enter arrives as the same carriage return as enter, and the two cannot
 * be told apart at all. Ordinary text is unaffected; only keys that were
 * already ambiguous change shape, and a terminal that does not implement this
 * ignores it, which leaves enter doing the default and costs nothing.
 */
const PASTE_START = "\x1b[200~";
const PASTE_END = "\x1b[201~";

const KEYS_DISAMBIGUATE_ON = "\x1b[>1u";
const KEYS_DISAMBIGUATE_OFF = "\x1b[<u";

/**
 * Ask the terminal to bracket pasted text.
 *
 * Without it a paste is indistinguishable from typing, so every newline in it
 * is an enter: a twelve-line paste became twelve messages, each steering the
 * turn the one before it started. With it the terminal wraps the blob in
 * `\x1b[200~` and `\x1b[201~`, and the newlines inside are text.
 */
const BRACKETED_PASTE_ON = "\x1b[?2004h";
const BRACKETED_PASTE_OFF = "\x1b[?2004l";

const ALT_SCROLL_ON = "\x1b[?1007h";
const ALT_SCROLL_OFF = "\x1b[?1007l";

const DIM = "\x1b[2m";
const BOLD = "\x1b[1m";
const ITALIC = "\x1b[3m";
const RESET = "\x1b[0m";
const CYAN = "\x1b[36m";
const GREEN = "\x1b[32m";
const YELLOW = "\x1b[33m";
const MAGENTA = "\x1b[35m";
const RED = "\x1b[31m";

const STATUS_ROWS = 1;
/**
 * The status line sits under the delegate rows rather than under the composer.
 *
 * What the session is doing reads as a caption on the thing it describes, and
 * when children are running that thing is the fleet block above the line, not
 * the composer below it. The row is still paid for out of the transcript's
 * height, so the composer never moves.
 */
const COMPOSER_ROWS = 3;
/**
 * One blank row between the transcript and the composer.
 *
 * The composer sat directly under the last line of the reply, so a rule was
 * doing all the work of saying where reading stops and typing starts. A line of
 * nothing does it better and costs one row.
 */
const SPACER_ROWS = 1;
/**
 * Rows the fleet block may take before it scrolls internally.
 *
 * A fleet is a status display, not the content: a 30-way fan-out must not push
 * the transcript off the screen. Past this many children the block shows the
 * ones that are still working and counts the rest.
 */
const FLEET_ROWS_MAX = 8;
/**
 * Activity lines the preview box holds.
 *
 * Fixed at three because a preview that grows with the run is a transcript in
 * miniature, and the transcript already exists. Newer activity pushes the
 * older rows up and out: the box always names the latest things the children
 * are doing and never more than three of them.
 */
const PREVIEW_ROWS = 3;
/** Width of the role gutter, so every entry's text starts in one column. */
/**
 * Width of the marker column.
 *
 * The transcript used to name every entry down the left — `you`, `think`,
 * `coder`, `note`, `tool` — which is five words of chrome per turn saying what
 * the styling already said. What is worth a column is the one thing styling
 * cannot say: whether a reply is finished. So the column is a dot, and it is
 * two glyphs wide plus a space.
 */
const GUTTER = 4;
/**
 * How long a lone escape byte waits for the rest of a sequence.
 *
 * A bare `\x1b` and the first byte of an arrow key are the same byte, so the
 * only way to tell them apart is to wait. Terminals deliver the rest of an
 * arrow key in the same read or the one straight after, so this is below the
 * threshold where a keypress feels delayed, and it lets a single escape
 * interrupt rather than requiring two.
 */
const ESCAPE_WINDOW_MS = 40;

export interface CoderUiOptions {
  /**
   * The workspace's skills and the choice made about them, for `/skills`.
   *
   * Optional so a caller with no skills, and every test, can leave it out; the
   * screen then says there are none rather than being unreachable.
   */
  readonly skills?: SkillSelection | undefined;
  /** Re-declare the tools after a skill is switched. */
  readonly onSkillsChanged?: (() => void) | undefined;
  /**
   * Load a WASM plugin from a manifest path and say what happened.
   *
   * Experimental, for `/plugin load <manifest>`. The caller owns the plugin
   * registry and the tool re-declaration; this interface only relays the path
   * and shows the sentence that comes back.
   */
  readonly loadPlugin?: ((manifestPath: string) => string) | undefined;
  readonly stdin: NodeJS.ReadStream;
  readonly stdout: NodeJS.WriteStream;
}

/**
 * Put `left` at the start of a row and `right` at the end.
 *
 * Padding is computed on visible width so styling does not shift the right
 * edge. When the two would collide the right side is dropped rather than
 * wrapping the bar onto a second row.
 */
function justify(left: string, right: string, width: number): string {
  const used = visibleWidth(left) + visibleWidth(right);
  if (right.length === 0) return left;
  if (used + 2 > width) return left;
  return left + " ".repeat(width - used) + right;
}

/** A key hint. */
interface Hint {
  readonly text: string;
}

/**
 * Lay out the key hints, dropping them from the end until the row fits.
 *
 * Only the skills screen shows these now: the chat's keys moved into `/help`,
 * where a reader looks for them once rather than past them always. A hint that
 * outranked everything else on the row went with that move — it existed for the
 * key that stops the fleet, and the fleet's row is no longer here.
 */
function hints(keys: ReadonlyArray<Hint>, right: string, width: number): string {
  const shown = [...keys];
  for (;;) {
    const left = `${DIM}${shown.map((key) => key.text).join(" · ")}${RESET}`;
    if (shown.length > 0 && visibleWidth(left) + visibleWidth(right) + 2 <= width) {
      return justify(left, right, width);
    }
    if (shown.length === 0) break;
    shown.pop();
  }

  const plain = shown.map((key) => key.text).join(" · ");
  if (plain.length === 0) return right;
  const letters = [...plain];
  const clipped =
    letters.length > width ? `${letters.slice(0, Math.max(0, width - 1)).join("")}…` : plain;
  return `${DIM}${clipped}${RESET}`;
}

/**
 * The colour a fleet row's mark takes, so the column can be scanned.
 *
 * Failure is the only one that is loud. A fleet of fifteen where every row is
 * coloured is a fleet where nothing stands out, and the row a reader is looking
 * for is the one that went wrong.
 */
function fleetColor(status: CoderTaskStatus): string {
  if (status === "running") return YELLOW;
  if (status === "completed") return GREEN;
  if (status === "failed") return RED;
  return DIM;
}

/** Human-readable elapsed time, in the shape a status line wants. */
function elapsed(sinceMs: number, nowMs: number): string {
  const seconds = Math.max(0, Math.round((nowMs - sinceMs) / 1000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes}m ${seconds % 60}s`;
}

/** Collapse to one line and cut to a visible width, marking what was cut. */
function clip(text: string, width: number): string {
  return truncate(text.replace(/\s+/g, " ").trim(), width);
}

/** Cut to a visible width, keeping the leading indentation intact. */
function truncate(text: string, width: number): string {
  const glyphs = [...text.replace(/\t/g, "  ")];
  if (glyphs.length <= width) return glyphs.join("");
  return `${glyphs.slice(0, Math.max(1, width - 1)).join("")}…`;
}

/**
 * Match a complete escape sequence at `index`, or return undefined when the
 * bytes so far could still be the start of one. Covers CSI (`\x1b[…final`),
 * SS3 (`\x1bO…`), and the `\x1b[…~` forms that carry PageUp and PageDown.
 */
/**
 * The legacy byte a keyboard-protocol sequence stands for, if any.
 *
 * Asking the terminal to disambiguate escape codes buys shift+enter, and costs
 * every control key: with the protocol on, ctrl+c arrives as `\x1b[99;5u`
 * rather than as `\x03`, so a console that reads only the byte stops being
 * quittable. Decoding back to the byte keeps one set of handlers for both
 * spellings rather than two that can disagree.
 *
 * Only what this interface actually binds. An unrecognized sequence is left
 * alone for the caller to handle or ignore.
 */
function controlFromKeyboardProtocol(sequence: string): string | undefined {
  const match = /^\x1b\[(\d+)(?:;(\d+))?u$/.exec(sequence);
  if (match === null) return undefined;

  const code = Number(match[1]);
  // The modifier is a bitfield offset by one: 1 is none, 5 is ctrl, 2 is shift.
  const modifiers = Number(match[2] ?? "1") - 1;
  const ctrl = (modifiers & 4) !== 0;

  if (ctrl && code >= 97 && code <= 122) {
    // ctrl+a is 1, ctrl+c is 3, and so on down the alphabet.
    return String.fromCharCode(code - 96);
  }
  // Tab with no modifier at all. Shift+tab is a different key here and is
  // matched further down; decoding it to a bare tab cycled the model instead of
  // the reasoning level.
  if (modifiers === 0 && code === 9) return "\t";
  return undefined;
}

function matchEscapeSequence(text: string, index: number): string | undefined {
  if (text[index] !== "\x1b") return undefined;
  const second = text[index + 1];
  if (second === undefined) return undefined;

  if (second === "[") {
    let cursor = index + 2;
    while (cursor < text.length) {
      const char = text[cursor] ?? "";
      // Parameter and intermediate bytes, then a final byte ends the sequence.
      if (char >= "0" && char <= "?") {
        cursor += 1;
        continue;
      }
      return text.slice(index, cursor + 1);
    }
    return undefined;
  }

  if (second === "O") {
    return index + 2 < text.length ? text.slice(index, index + 3) : undefined;
  }

  // Anything else after the escape is not a sequence this interface reads, so
  // the escape stands on its own and the next byte is an ordinary key.
  return "\x1b";
}

/**
 * Run the interface until the user exits. Resolves with the process exit code.
 */
export function runCoderUi(session: CoderSession, options: CoderUiOptions): Promise<number> {
  const { stdin, stdout } = options;

  let composer = "";
  /**
   * Which screen has the keyboard. The chat is the interface; `/skills` is a
   * screen over it rather than a turn in it, because switching a skill off is
   * a change to what the next turn carries, not something to say to the model.
   */
  let screen: "chat" | "skills" = "chat";
  /** The row `/skills` acts on. */
  let skillRow = 0;
  /**
   * The first transcript line the viewport shows, or undefined while the
   * viewport follows the newest content.
   *
   * Holding an absolute line rather than a distance from the bottom is what
   * makes a scrolled-up reader stay put while a reply keeps arriving: the
   * bottom moves, the anchor does not.
   */
  let anchor: number | undefined;
  /** Tool calls the reader expanded with ctrl+o. */
  const expanded = new Set<string>();
  let exitCode = 0;
  let closed = false;
  let runningSince = Date.now();
  /**
   * Which half of a second the pulse is in.
   *
   * Advanced by the same ticker that moves the elapsed clock, so an unfinished
   * reply blinks without a timer of its own.
   */
  let pulse = true;
  /** Redraws the status line once a second so the elapsed time advances. */
  let ticker: NodeJS.Timeout | undefined;
  /** Rows as last painted, so only what changed is written. */
  let painted: string[] = [];
  /** Geometry from the last paint, which is what the scroll keys act on. */
  let lineCount = 0;
  let viewport = 1;
  /** Bytes held back because they may be the start of an escape sequence. */
  let pendingEscape = "";
  /** A paste whose end has not arrived yet. */
  let pendingPaste = "";
  let escapeTimer: NodeJS.Timeout | undefined;

  const write = (text: string) => {
    stdout.write(text);
  };

  return new Promise<number>((resolve) => {
    const finish = (code: number) => {
      if (closed) return;
      closed = true;
      exitCode = code;
      if (ticker !== undefined) {
        clearInterval(ticker);
        ticker = undefined;
      }
      if (escapeTimer !== undefined) {
        clearTimeout(escapeTimer);
        escapeTimer = undefined;
      }
      unsubscribe();
      // Leaving the interface ends the fleet with it: a child holds a process,
      // and a console that exits while fifteen of them keep spending would
      // leave the reader nothing to stop them with.
      session.stopTasks();
      session.close();
      stdin.off("data", onData);
      stdout.off("resize", onResize);
      if (stdin.isTTY) stdin.setRawMode(false);
      stdin.pause();
      write(
        CURSOR_SHOW + BRACKETED_PASTE_OFF + KEYS_DISAMBIGUATE_OFF + ALT_SCROLL_OFF + ALT_SCREEN_OFF,
      );
      resolve(exitCode);
    };

    /** Turn the transcript into printable rows, newest last. */
    const transcriptLines = (snapshot: CoderSnapshot, width: number): ReadonlyArray<string> => {
      const out: string[] = [];
      const body = Math.max(20, width - GUTTER - 1);

      // One blank row between entries. It is what keeps a tool call from
      // reading as part of the sentence before it.
      for (const entry of snapshot.entries) {
        if (out.length > 0) out.push("");
        out.push(...renderEntry(entry, body, snapshot.tasks));
      }
      return out;
    };

    const renderEntry = (entry: CoderEntry, width: number, tasks: ReadonlyArray<CoderTask>): ReadonlyArray<string> => {
      const color =
        entry.role === "you"
          ? CYAN
          : entry.role === "assistant"
            ? GREEN
            : entry.role === "tool"
              ? MAGENTA
              : entry.role === "reasoning"
                ? DIM
                : YELLOW;

      // A reply still arriving pulses; a finished one is solid. The dot is the
      // only thing in this column because it is the only thing the colour and
      // the styling do not already say.
      const glyph = entry.settled ? "●" : pulse ? "●" : "○";
      const head = `  ${color}${glyph}${RESET} `;
      const continuation = " ".repeat(GUTTER);
      const rows = entryRows(entry, width, tasks);
      const caret = "";

      return rows.map((row, index) => {
        const tail = index === rows.length - 1 ? caret : "";
        return `${index === 0 ? head : continuation}${row}${tail}`;
      });
    };

    const entryRows = (entry: CoderEntry, width: number, tasks: ReadonlyArray<CoderTask>): ReadonlyArray<string> => {
      if (entry.role === "tool" && entry.tool !== undefined) {
        return toolRows(entry.tool, width, expanded.has(entry.tool.callId), tasks);
      }
      if (entry.text.length === 0 && !entry.settled) return ["…"];
      // Reasoning is Markdown too, rendered inside dim italic. It was plain
      // text on the theory that emphasis nested in italic reads worse than the
      // source — but the source is what a reader actually got: models write
      // `**#160**` and numbered lists in their reasoning, and unrendered
      // markup is harder to read than rendered markup in any style.
      if (entry.role === "reasoning") {
        return renderMarkdown(entry.text, width, `${DIM}${ITALIC}`);
      }
      if (entry.role === "assistant") return renderMarkdown(entry.text, width);
      return wrapStyled(entry.text, width, entry.role === "notice" ? DIM : "");
    };

    const toolRows = (tool: CoderToolCall, width: number, open: boolean, tasks: ReadonlyArray<CoderTask>): ReadonlyArray<string> => {
      const mark =
        tool.status === "running"
          ? `${YELLOW}◐${RESET}`
          : tool.status === "failed"
            ? `${RED}✗${RESET}`
            : `${GREEN}✓${RESET}`;
      const rows = [`${mark} ${BOLD}${tool.name}${RESET}`];

      if (tool.name === "delegate" && tool.status === "running") {
        const phrase = fleetPhrase(tasks) ?? "starting children…";
        rows.push(`${DIM}→ ${phrase}${RESET}`);
        const activities = latestActivities(tasks, PREVIEW_ROWS);
        if (activities.length > 0) {
          const boxWidth = Math.max(10, width - 4);
          const frame = `${DIM}╭${"─".repeat(boxWidth + 2)}╮${RESET}`;
          const floor = `${DIM}╰${"─".repeat(boxWidth + 2)}╯${RESET}`;
          const lines = activities.map((activity) => {
            const text = truncate(activityPhrase(activity), boxWidth);
            const pad = " ".repeat(Math.max(0, boxWidth - [...text].length));
            return `${DIM}│${RESET} ${text}${pad} ${DIM}│${RESET}`;
          });
          rows.push(frame, ...lines, floor);
        }
      }

      if (open) {
        for (const line of tool.arguments.split("\n")) {
          rows.push(`${DIM}${truncate(line, width)}${RESET}`);
        }
        if (tool.error !== undefined) {
          rows.push(...wrapStyled(tool.error, width, RED));
        } else if (tool.output !== undefined) {
          // The arrow separates the call from its result, which otherwise read
          // as one JSON document split over a blank line.
          const lines = tool.output.split("\n");
          for (const [index, line] of lines.entries()) {
            const marker = index === 0 ? `${DIM}→${RESET} ` : "  ";
            rows.push(`${marker}${DIM}${truncate(line, Math.max(4, width - 2))}${RESET}`);
          }
        }
        return rows;
      }

      if (tool.name !== "delegate" || tool.status !== "running") {
        const args = clip(tool.arguments, Math.max(8, width - 4));
        if (args.length > 0) rows.push(`${DIM}${args}${RESET}`);
        const outcome =
          tool.error !== undefined
            ? `${RED}${clip(tool.error, Math.max(8, width - 4))}${RESET}`
            : tool.output !== undefined
              ? `${DIM}→ ${clip(tool.output, Math.max(8, width - 6))}${RESET}`
              : tool.status === "running" && tool.name !== "delegate"
                ? `${DIM}→ running…${RESET}`
                : "";
        if (outcome.length > 0) rows.push(outcome);
      }
      return rows;
    };

    /**
     * The fleet block: one row per child.
     *
     * Drawn above the status line rather than in the transcript, because it is
     * live state and the transcript is a record. A reader who scrolled back to
     * an earlier tool call still needs to see what the fleet is doing now.
     */
    const fleetLines = (snapshot: CoderSnapshot, width: number): ReadonlyArray<string> => {
      const tasks = snapshot.tasks;
      if (tasks.length === 0) return [];

      // Working children first when there are more than fit: a finished child
      // has already been reported on the transcript, so it is the one to drop.
      const shown =
        tasks.length <= FLEET_ROWS_MAX
          ? tasks
          : [
              ...tasks.filter((task) => task.status === "running" || task.status === "pending"),
              ...tasks.filter((task) => task.status !== "running" && task.status !== "pending"),
            ].slice(0, FLEET_ROWS_MAX);

      const out = fleetRows(shown, Math.max(20, width - 8)).map((row) => {
        const color = fleetColor(row.status);
        return `  ${DIM}${row.branch}${RESET} ${color}${row.mark}${RESET} ${DIM}${row.text}${RESET}`;
      });
      const hidden = tasks.length - shown.length;
      if (hidden > 0) out.push(`  ${DIM}   +${String(hidden)} more${RESET}`);
      return out;
    };

    /** The newest tool call, which is the one ctrl+o expands. */
    const focusedTool = (snapshot: CoderSnapshot): string | undefined => {
      for (let index = snapshot.entries.length - 1; index >= 0; index -= 1) {
        const callId = snapshot.entries[index]?.tool?.callId;
        if (callId !== undefined) return callId;
      }
      return undefined;
    };

    /**
     * The `/skills` screen: every skill found, and whether the model is told
     * about it.
     *
     * The description is shown because it is the whole of what a switched-on
     * skill costs and the whole of what the model chooses on. A reader deciding
     * whether to keep one needs to see the sentence the model sees.
     */
    const skillsLines = (width: number): ReadonlyArray<string> => {
      const rows: string[] = [];
      const all = options.skills?.all ?? [];

      rows.push(`${BOLD}Skills${RESET}`, "");
      if (all.length === 0) {
        rows.push(
          ...wrapStyled(
            "No skills were found. A skill is a directory holding a SKILL.md, under " +
              ".agents/skills in this repository or under your home directory.",
            width,
            DIM,
          ),
        );
        return rows;
      }

      rows.push(
        ...wrapStyled(
          "Switched-off skills are left out of the tool the model is given, so they cost " +
            "it nothing and it cannot call them. The choice is remembered for this workspace.",
          width,
          DIM,
        ),
        "",
      );

      for (const [at, skill] of all.entries()) {
        const on = options.skills?.isOn(skill.name) ?? true;
        const focused = at === skillRow;
        const mark = on ? `${GREEN}[on] ${RESET}` : `${DIM}[off]${RESET}`;
        const caret = focused ? `${CYAN}❯${RESET} ` : "  ";
        const name = focused ? `${BOLD}${skill.name}${RESET}` : skill.name;
        rows.push(`${caret}${mark} ${on ? name : `${DIM}${skill.name}${RESET}`}`);
        // The description is indented under its own row, dim, and only for the
        // row in hand: eight descriptions at once is the wall of text the
        // catalog exists to avoid.
        if (focused) {
          rows.push(
            ...wrapStyled(skill.description, Math.max(20, width - 8), DIM).map(
              (line) => `        ${line}`,
            ),
          );
        }
      }

      return rows;
    };

    const render = () => {
      if (closed) return;
      const snapshot = session.snapshot();
      const width = stdout.columns ?? 80;
      const height = stdout.rows ?? 24;

      if (screen === "skills") {
        const body = skillsLines(width);
        const rows: string[] = [];
        for (let row = 0; row < Math.max(1, height - 2); row += 1) rows.push(body[row] ?? "");
        rows.push(
          `${DIM}${"─".repeat(Math.max(0, width))}${RESET}`,
          hints(
            [{ text: "↑↓ move" }, { text: "space toggles" }, { text: "esc returns" }],
            "",
            width,
          ),
        );
        // No cursor to place: the screen is a list, not a field.
        paint(rows, rows.length, 1);
        return;
      }
      const transcriptHeight = Math.max(1, height - STATUS_ROWS - COMPOSER_ROWS - SPACER_ROWS);

      const fleet = fleetLines(snapshot, width);
      // The fleet takes its rows from the transcript, not from the chrome:
      // the composer stays where the reader's hands expect it. The status
      // line is priced into `transcriptHeight`, which keeps every frame the
      // same height whether children are running or not.
      const transcriptRows = Math.max(1, transcriptHeight - fleet.length);

      const lines = transcriptLines(snapshot, width);
      lineCount = lines.length;
      viewport = transcriptRows;

      const maxStart = Math.max(0, lines.length - transcriptRows);
      const start = anchor === undefined ? maxStart : Math.min(anchor, maxStart);
      // Kept for the status line: a reader scrolled up with nothing saying so
      // reads a still transcript as a stopped session.
      const above = start;

      const rows: string[] = [];
      for (let row = 0; row < transcriptRows; row += 1) rows.push(lines[start + row] ?? "");
      rows.push(...fleet);

      // Bottom chrome, in the order a reader scans it. The status line is the
      // main agent's state; the delegate preview lives inline under the
      // delegate tool call. The composer follows in its own region between two
      // rules.
      const rule = `${DIM}${"─".repeat(Math.max(0, width))}${RESET}`;
      const inner = Math.max(10, width - 4);

      // The elapsed time and nothing else. This said `streaming` until the
      // reply source became the inference proxy, which builds the whole body
      // and sends it once: a turn that shows one block after four silent
      // seconds was never streaming, and the status line must not say it was.
      const chatActivity = snapshot.running
        ? `${YELLOW}●${RESET} working… ${DIM}(${elapsed(runningSince, Date.now())})${RESET}`
        : `${DIM}○ ready${RESET}`;
      const scrolled = anchor === undefined ? "" : `${DIM} · scrolled ↑${String(above)}${RESET}`;
      const activity = chatActivity + scrolled;
      // Dropped from the left as the terminal narrows, because that is the
      // order of what a reader cannot recover elsewhere: they can see which
      // checkout they are in, they can ask git for the branch, and nothing on
      // screen but this says which model answers or what the thread has left.
      const facts = [snapshot.repository, snapshot.branch, snapshot.model];
      if (snapshot.reasoning !== undefined) facts.push(`thinking ${snapshot.reasoning}`);
      if (snapshot.budget !== undefined) facts.push(snapshot.budget);
      let where = "";
      for (let from = 0; from < facts.length; from += 1) {
        const candidate = `${DIM}${facts.slice(from).join(" · ")}${RESET}`;
        if (visibleWidth(activity) + visibleWidth(candidate) + 2 > inner) continue;
        where = candidate;
        break;
      }
      // A blank row, then the composer. The keys used to live on a second row
      // under it; they are in `/help` now, which is where a reader looks for
      // them once rather than past them always.
      rows.push("");
      rows.push(rule);
      // The composer shows its tail, never more characters than the row holds.
      // A row written past the last column makes the terminal wrap it, which
      // pushes the rule and the hints down a line and leaves the text they used
      // to occupy on screen with nothing to erase it.
      const composerRoom = Math.max(4, width - 4);
      // A pasted blob is shown as what it is rather than as its last row. The
      // composer holds every line; the reader needs to know how many there are
      // and that they will go as one message, not to read them here.
      const pasted = composer.split("\n");
      const summarised =
        pasted.length > 1
          ? `${pasted[0] ?? ""} ${DIM}[+${String(pasted.length - 1)} more ${
              pasted.length === 2 ? "line" : "lines"
            }]${RESET}`
          : composer;
      const typed = [...summarised];
      const visible =
        visibleWidth(summarised) > composerRoom
          ? `…${typed.slice(typed.length - composerRoom + 1).join("")}`
          : summarised;
      rows.push(`  › ${visible}`);
      rows.push(rule);

      // The status line sits under the composer. Main agent state, workspace,
      // model, and budget stay here so the transcript and any running work are
      // read first and the bottom chrome is the last thing the eye reaches.
      rows.push(`  ${justify(activity, where, inner)}`);

      paint(
        rows,
        transcriptRows + fleet.length + 3,
        4 + [...visible].length + 1,
      );
    };

    /**
     * Write only the rows that changed, erasing each to the end of the line.
     *
     * Nothing here emits a newline or writes past the last column, so the
     * terminal has no reason to scroll and no frame can reach its scrollback.
     */
    const paint = (rows: ReadonlyArray<string>, cursorRow: number, cursorColumn: number) => {
      const frame: string[] = [CURSOR_HIDE];
      for (let index = 0; index < rows.length; index += 1) {
        const next = rows[index] ?? "";
        if (painted[index] === next) continue;
        frame.push(`\x1b[${index + 1};1H`, ERASE_LINE, next);
      }
      // Rows the last frame had that this one does not. The fleet comes and
      // goes with the work, so a frame can be shorter than the one before it,
      // and whatever it left below would otherwise stay on screen with nothing
      // owning it.
      for (let index = rows.length; index < painted.length; index += 1) {
        frame.push(`\x1b[${index + 1};1H`, ERASE_LINE);
      }
      painted = [...rows];
      frame.push(`\x1b[${cursorRow};${cursorColumn}H`, CURSOR_SHOW);
      write(frame.join(""));
    };

    const onResize = () => {
      // Every row is laid out for the old width, so none of it can be reused.
      painted = [];
      render();
    };

    /** Move the viewport by whole lines, snapping back to follow at the end. */
    const scrollBy = (delta: number) => {
      const maxStart = Math.max(0, lineCount - viewport);
      const current = anchor ?? maxStart;
      const next = Math.max(0, Math.min(maxStart, current + delta));
      anchor = next >= maxStart ? undefined : next;
    };

    const submit = (mode: "steer" | "queue" = "steer") => {
      const prompt = composer;
      composer = "";
      anchor = undefined;
      // Only when a turn actually begins. Resetting on every submission made
      // `/export` mid-turn put the elapsed clock back to zero, which reads as
      // the turn having restarted when nothing happened to it at all.
      if (!session.running) runningSince = Date.now();
      render();

      // The elapsed time has to advance between chunks, not only when one
      // arrives, or a slow reply looks stalled.
      // Twice a second, because the same tick drives the pulse on an unfinished
      // reply as well as the elapsed clock, and a dot that blinks once a second
      // reads as a dot that is broken.
      ticker ??= setInterval(() => {
        session.pruneTasks();
        pulse = !pulse;
        if (session.running || session.snapshot().tasks.length > 0) render();
      }, 500);

      // A delegate line is not a turn: it returns as soon as the children are
      // submitted and each one reports later, so nothing here waits on it and
      // the ticker above keeps the fleet rows moving.
      // `/reload` restarts this session on the code as it is now. The rebuild
      // and the restart belong to the runner, which has the screen back by
      // then; the interface only asks, by exiting with a code of its own.
      if (/^\/reload\s*$/.test(prompt.trim())) {
        const root = sourceCheckout();
        if (root === undefined) {
          session.notice(
            "This session is not running from a source checkout, so there is nothing to " +
              "rebuild. `/reload` works where `openagents coder` was started from the repository.",
          );
          render();
          return;
        }
        // The transcript ends here. It is not written out on the way past:
        // reloading is something a reader does many times in a sitting, and a
        // command that quietly leaves a file behind each time is one they have
        // to clean up after. `/export` keeps a conversation when it is wanted.
        finish(RELOAD_EXIT_CODE);
        return;
      }

      // `/skills` opens a screen rather than sending a turn: it changes what
      // the next turn carries, so it is not something to say to the model.
      if (/^\/skills\s*$/.test(prompt.trim())) {
        screen = "skills";
        skillRow = 0;
        painted = [];
        render();
        return;
      }

      // `/plugin load` changes what the next turn carries, like `/skills`, so
      // it is not something to say to the model. Experimental.
      const pluginLoad = /^\/plugin\s+load\s+(.+)$/.exec(prompt.trim());
      if (pluginLoad !== null || /^\/plugin\b/.test(prompt.trim())) {
        const path = pluginLoad?.[1]?.trim();
        session.notice(
          path === undefined || path.length === 0
            ? "Usage: /plugin load <path-to-manifest.json>. Experimental: loads a WASM plugin " +
                "as a session tool."
            : options.loadPlugin === undefined
              ? "This session cannot load plugins."
              : options.loadPlugin(path),
        );
        render();
        return;
      }

      if (prompt.trimStart().startsWith("/delegate")) {
        void session.submit(prompt);
        render();
        return;
      }

      void session.submit(prompt, mode).finally(() => {
        if (ticker !== undefined) {
          clearInterval(ticker);
          ticker = undefined;
        }
        render();
      });
    };

    /** A lone escape: interrupt if there is something to interrupt, else clear. */
    const onEscape = () => {
      // Every settled bare escape lands here, whether it was recognised in the
      // chunk it arrived in or held for the window and released by the timer.
      // Leaving the screen out of this path is what made escape work in a test,
      // where another key follows immediately, and not in a terminal, where a
      // lone escape byte is all that ever arrives.
      if (screen === "skills") {
        screen = "chat";
        painted = [];
        render();
        return;
      }
      // Say that it happened. An interrupt that only stops the stream leaves a
      // reader looking at a settled reply with no way to tell whether the key
      // did anything, which reads as the key not working.
      if (session.interrupt()) session.notice("Interrupted.");
      else composer = "";
      render();
    };

    const toggleFocusedTool = () => {
      const callId = focusedTool(session.snapshot());
      if (callId === undefined) return;
      if (expanded.has(callId)) expanded.delete(callId);
      else expanded.add(callId);
      render();
    };

    /**
     * Handle one chunk of terminal input.
     *
     * A chunk is not a keypress. Fast typing coalesces, a paste arrives whole,
     * and an arrow key is several bytes, so the chunk is walked one key at a
     * time. Treating a chunk as a single key means `Enter` at the end of typed
     * text never submits, which is exactly what the first version did.
     */
    const onData = (chunk: string | Buffer) => {
      if (escapeTimer !== undefined) {
        clearTimeout(escapeTimer);
        escapeTimer = undefined;
      }
      let text =
        pendingPaste + pendingEscape + (typeof chunk === "string" ? chunk : chunk.toString("utf8"));
      pendingPaste = "";
      pendingEscape = "";
      let index = 0;
      let dirty = false;

      while (index < text.length) {
        const char = text[index] ?? "";

        // A paste is taken whole, before anything in here reads one of its
        // newlines as an enter. The terminal says where it ends; if the end has
        // not arrived yet the rest is held for the next chunk, because half a
        // paste submitted as a message is the bug this exists to stop.
        if (text.startsWith(PASTE_START, index)) {
          const from = index + PASTE_START.length;
          const to = text.indexOf(PASTE_END, from);
          if (to < 0) {
            pendingPaste = text.slice(index);
            break;
          }
          composer += text.slice(from, to);
          index = to + PASTE_END.length;
          dirty = true;
          continue;
        }

        // The skills screen takes the keyboard while it is up. Only the keys it
        // names do anything: a stray letter must not fall through into the
        // composer of a screen the reader cannot see.
        if (screen === "skills") {
          const count = options.skills?.all.length ?? 0;

          if (char === "\x1b") {
            const sequence = matchEscapeSequence(text, index);
            if (sequence === undefined) {
              pendingEscape = text.slice(index);
              break;
            }
            index += sequence.length;
            if (sequence === "\x1b") {
              onEscape();
              continue;
            } else if (sequence === "\x1b[A" || sequence === "\x1bOA") {
              skillRow = Math.max(0, skillRow - 1);
            } else if (sequence === "\x1b[B" || sequence === "\x1bOB") {
              skillRow = Math.min(Math.max(0, count - 1), skillRow + 1);
            } else {
              continue;
            }
            render();
            continue;
          }

          index += 1;
          if (char === " ") {
            const skill = options.skills?.all[skillRow];
            if (skill !== undefined) {
              options.skills?.toggle(skill.name);
              // Re-declared now rather than on the next turn, so `/system`
              // agrees with this screen the moment it is left.
              options.onSkillsChanged?.();
            }
            render();
          } else if (char === "\x03" || char === "\x04") {
            screen = "chat";
            painted = [];
            render();
          }
          continue;
        }

        if (char === "\x1b") {
          const sequence = matchEscapeSequence(text, index);
          if (sequence === undefined) {
            // The rest of the sequence has not arrived. Hold it: if nothing
            // follows within the window it was a bare escape after all.
            pendingEscape = text.slice(index);
            break;
          }
          index += sequence.length;
          // Before the bare escape is taken as an interrupt: some terminals
          // send escape then return for shift or alt enter, and an escape with
          // a return immediately behind it is not something a reader types by
          // accident.
          if (sequence === "\x1b" && text[index] === "\r") {
            index += 1;
            submit("queue");
            dirty = false;
            continue;
          }
          if (sequence === "\x1b") {
            onEscape();
            dirty = false;
            continue;
          }
          // A control key in its protocol spelling is handled as the byte it
          // stands for, so ctrl+c and ctrl+d keep working with the protocol on.
          const asControl = controlFromKeyboardProtocol(sequence);
          if (asControl !== undefined) {
            text = text.slice(0, index) + asControl + text.slice(index);
            continue;
          }

          // Enter reported through the keyboard protocol: `13` is the key and
          // the second parameter is the modifier, where 2 is shift. Shift+enter
          // queues; enter, with or without other modifiers, steers.
          const enter = /^\x1b\[13(?:;(\d+))?u$/.exec(sequence);
          if (enter !== null) {
            if (!session.running || composer.length > 0) {
              submit(enter[1] === "2" ? "queue" : "steer");
            }
            dirty = false;
            continue;
          }
          // Shift+tab, in both spellings: the classic back-tab and the one the
          // keyboard protocol reports. Tab moves the model, shift+tab moves how
          // hard it is asked to think.
          if (sequence === "\x1b[Z" || sequence === "\x1b[9;2u") {
            session.cycleReasoning();
            dirty = true;
            continue;
          }

          const page = Math.max(1, viewport - 1);
          if (sequence === "\x1b[5~") scrollBy(-page);
          else if (sequence === "\x1b[6~") scrollBy(page);
          else if (sequence === "\x1b[A" || sequence === "\x1bOA") scrollBy(-1);
          else if (sequence === "\x1b[B" || sequence === "\x1bOB") scrollBy(1);
          else if (sequence === "\x1b[1~" || sequence === "\x1b[H") scrollBy(-lineCount);
          else if (sequence === "\x1b[4~" || sequence === "\x1b[F") scrollBy(lineCount);
          else continue;
          dirty = true;
          continue;
        }

        if (char === "\r" || char === "\n") {
          index += 1;
          // Swallow a CRLF pair so a paste does not submit twice.
          if (char === "\r" && text[index] === "\n") index += 1;
          // Always. A turn already running is not a reason to drop what was
          // typed: an interface command runs at once, and anything else is
          // queued by the session and sent when the turn ends. Ignoring the key
          // is what made `/export` impossible mid-turn and steering impossible
          // at all.
          submit();
          dirty = false;
          continue;
        }

        if (char === "\x03") {
          if (session.interrupt()) {
            render();
          } else {
            finish(130);
          }
          return;
        }

        if (char === "\x04") {
          if (composer.length === 0) {
            finish(0);
            return;
          }
          index += 1;
          continue;
        }

        if (char === "\x0f") {
          toggleFocusedTool();
          dirty = false;
          index += 1;
          continue;
        }

        // Ctrl+X stops the children. Escape is not overloaded to do it: escape
        // interrupts the reply the reader is watching, and a key that means
        // "stop this" sometimes and "stop those fifteen" other times is how a
        // fleet gets killed by accident.
        if (char === "\x18") {
          session.stopTasks();
          dirty = true;
          index += 1;
          continue;
        }

        // Tab is a printable character to the run scanner below, so it has to
        // be claimed here or it lands in the composer as literal whitespace.
        if (char === "\t" && session.canCycleBackend) {
          session.cycleBackend();
          dirty = false;
          index += 1;
          continue;
        }

        if (char === "\x7f" || char === "\b") {
          composer = composer.slice(0, -1);
          dirty = true;
          index += 1;
          continue;
        }

        if (char === "\x15") {
          composer = "";
          dirty = true;
          index += 1;
          continue;
        }

        if (char < " " && char !== "\t") {
          index += 1;
          continue;
        }

        // Take the whole run of printable text at once so a paste is one
        // append rather than one per character. Surrogate pairs stay intact
        // because the run is sliced, not rebuilt from code units.
        let end = index;
        while (end < text.length) {
          const next = text[end] ?? "";
          if (next === "\x1b" || next === "\r" || next === "\n") break;
          if (next < " " && next !== "\t") break;
          end += 1;
        }
        composer += text.slice(index, end);
        // Typing means the reader wants to see what they are answering.
        anchor = undefined;
        dirty = true;
        index = end;
      }

      if (pendingEscape.length > 0) {
        escapeTimer = setTimeout(() => {
          escapeTimer = undefined;
          const held = pendingEscape;
          pendingEscape = "";
          // A lone escape byte and nothing after it. Anything longer was the
          // start of a sequence the terminal never finished, and is dropped.
          if (held === "\x1b") onEscape();
        }, ESCAPE_WINDOW_MS);
      }

      if (dirty) render();
    };

    const unsubscribe = session.onChange(render);

    write(ALT_SCREEN_ON + ALT_SCROLL_ON + KEYS_DISAMBIGUATE_ON + BRACKETED_PASTE_ON);
    if (stdin.isTTY) stdin.setRawMode(true);
    stdin.resume();
    stdin.setEncoding("utf8");
    stdin.on("data", onData);
    stdout.on("resize", onResize);

    // No banner. Four lines of keys and commands at the top of every session is
    // four lines a reader scrolls past for the rest of it, and `/help` says the
    // same thing when it is wanted.
    render();
  });
}
