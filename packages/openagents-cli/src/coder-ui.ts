import { homedir } from "node:os";
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
 *     ┌───────────────────────┬──────────┐
 *     │ transcript, scrollable │ children │
 *     │                        │  ├─ one  │
 *     │                        │  └─ two  │
 *     ├───────────────────────┴──────────┤
 *     │ composer                         │
 *     │ status line                      │
 *     └──────────────────────────────────┘
 *
 * The right column appears only while children are running and only on a
 * terminal wide enough to give it room without squeezing the transcript. On a
 * narrow terminal the same rows go inline under the `delegate` call instead,
 * because a fleet that has nowhere to go is worse than one read in the feed.
 *
 * Painting is differential. An earlier version cleared the whole screen and
 * repainted it several times a second while a reply streamed, which left a
 * stack of half-drawn frames wherever the terminal kept scrolled-off alternate
 * screen rows. Nothing here clears the screen, writes a newline, or moves the
 * cursor past the last row, so the terminal never scrolls and its own
 * scrollback is never written to. Scrolling the transcript is the interface's
 * own job instead.
 */

import { readChildTranscript } from "./coder-child-transcript.js";
import { summarizeToolCall } from "./coder-tool-summary.js";
import { activityRows, fleetRows, latestActivities, taskActivity } from "./coder-fleet.js";
import { renderMarkdown, visibleWidth, wrapStyled } from "./coder-markdown.js";
import type { CoderEntry, CoderSession, CoderSnapshot, CoderToolCall } from "./coder-session.js";
import type { CoderTask, CoderTaskStatus } from "./coder-tasks.js";
import { coderTierLabel } from "./coder-tiers.js";
import { RELOAD_EXIT_CODE, sourceCheckout } from "./coder-reload.js";
import {
  backspaceComposer,
  expandComposerPrompt,
  handleIncomingPasteChunk,
  type PastedTextContent,
} from "./coder-paste.js";
import type { PastedImageContent } from "./coder-image.js";
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
/** Reverse video, for the sidebar's selector bar. */
const REVERSE = "\x1b[7m";
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

/**
 * The right column's width, and the narrowest terminal that gets one.
 *
 * A fleet row needs about thirty columns before its description and its
 * activity both survive being cut. Below the threshold the transcript would
 * pay for the sidebar in wrapped lines, so there is no sidebar and the fleet
 * stays inline.
 */
const SIDEBAR_WIDTH = 34;
const SIDEBAR_MINIMUM_TERMINAL = 100;

/** How many rows of one tool result the child screen shows. */
const CHILD_OUTPUT_ROWS = 12;
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
   * Pick and describe a foreign coding-agent session to resume.
   *
   * For `/resume` and `/resume <number>`. The caller loads the foreign-sessions
   * plugin and invokes it; this interface only asks for the result and shows it
   * as a notice.
   */
  readonly resume?: ((selection: number | undefined) => Promise<string>) | undefined;
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
 * The face the splash writes CODER in, drawn by hand in block glyphs.
 *
 * Hand-drawn because this interface renders with no dependencies, and a figlet
 * package to say one word would be the first. Six rows, because shorter art
 * stops reading as a wordmark and taller art crowds a small terminal.
 */
const SPLASH_WORD = [
  " ██████╗ ██████╗ ██████╗ ███████╗██████╗ ",
  "██╔════╝██╔═══██╗██╔══██╗██╔════╝██╔══██╗",
  "██║     ██║   ██║██║  ██║█████╗  ██████╔╝",
  "██║     ██║   ██║██║  ██║██╔══╝  ██╔══██╗",
  "╚██████╗╚██████╔╝██████╔╝███████╗██║  ██║",
  " ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝",
];

/** What the splash asks for. The status line already names the workspace. */
const SPLASH_HINT = "type a prompt below to begin";

/**
 * One cell of the splash's binary texture.
 *
 * Seeded from the cell's own position rather than from a random source, so the
 * same viewport always shows the same field. The paint loop diffs rows against
 * the last frame, and texture that shifted on every repaint would rewrite the
 * whole block each time and read as static rather than as a backdrop.
 */
function splashBit(row: number, column: number): string {
  // Two multiply-xor rounds, because one leaves the low bits of neighbouring
  // columns correlated and the field visibly striped.
  let hash = (Math.imul(row + 1, 2654435761) ^ Math.imul(column + 1, 0x9e3779b9)) >>> 0;
  hash = Math.imul(hash ^ (hash >>> 16), 0x45d9f3b) >>> 0;
  return ((hash ^ (hash >>> 16)) & 1) === 0 ? "0" : "1";
}

/** A full row of that texture, one digit per column. */
function splashTexture(row: number, width: number): string {
  let cells = "";
  for (let column = 0; column < width; column += 1) cells += splashBit(row, column);
  return cells;
}

/**
 * What the transcript's viewport shows before the first entry exists.
 *
 * A session opens onto nothing — no banner, by the note at the bottom of this
 * file — and nineteen blank rows read as a program that has not started. The
 * splash is not state and not a lifecycle: it is only what an empty transcript
 * looks like, so the first entry replaces it the way any frame replaces the
 * one before, and it never comes back. Same size in, same rows out, because
 * the renderer repaints on state changes and resize and nothing else.
 *
 * Each size gets the largest rendering that fits whole: the wordmark in a
 * frame with the binary field, a one-line box, or the bare word. Nothing here
 * may emit a row wider than the viewport — the terminal would wrap it, which
 * shifts every row below and leaves text nothing will erase.
 */
function splashLines(width: number, height: number): ReadonlyArray<string> {
  const wordWidth = [...(SPLASH_WORD[0] ?? "")].length;

  /** Wrap inner rows in the frame and center the block in the viewport. */
  const framed = (inner: ReadonlyArray<string>, innerWidth: number): ReadonlyArray<string> => {
    const rows = [
      `${DIM}┌${"─".repeat(innerWidth)}┐${RESET}`,
      ...inner,
      `${DIM}└${"─".repeat(innerWidth)}┘${RESET}`,
    ];
    const left = " ".repeat(Math.max(0, Math.floor((width - innerWidth - 2) / 2)));
    const above = Math.max(0, Math.floor((height - rows.length) / 2));
    return [...Array.from({ length: above }, () => ""), ...rows.map((row) => left + row)];
  };

  /** A framed row holding centered content, given the content's visible width. */
  const padded = (innerWidth: number, visible: number, styled: string): string => {
    const leftPad = Math.max(0, Math.floor((innerWidth - visible) / 2));
    const rightPad = Math.max(0, innerWidth - visible - leftPad);
    return `${DIM}│${RESET}${" ".repeat(leftPad)}${styled}${" ".repeat(rightPad)}${DIM}│${RESET}`;
  };

  /** A framed row of the binary field, edge to edge. */
  const texture = (row: number, innerWidth: number): string =>
    `${DIM}│${splashTexture(row, innerWidth)}│${RESET}`;

  // Four columns of air either side of the wordmark, so the letters do not
  // touch the field above them. Height needs the six rows of the word plus the
  // frame, the texture, the air, and the hint.
  const fullInner = wordWidth + 8;
  if (width >= fullInner + 2 && height >= SPLASH_WORD.length + 7) {
    const inner: string[] = [texture(0, fullInner), padded(fullInner, 0, "")];
    for (const line of SPLASH_WORD) {
      inner.push(padded(fullInner, wordWidth, `${CYAN}${line}${RESET}`));
    }
    inner.push(padded(fullInner, 0, ""));
    inner.push(padded(fullInner, SPLASH_HINT.length, `${DIM}${SPLASH_HINT}${RESET}`));
    inner.push(texture(1, fullInner));
    return framed(inner, fullInner);
  }

  // Too narrow or too short for the wordmark: the word spelled out in the same
  // frame, three rows tall, which fits down to a seventeen-column terminal.
  const compactInner = 15;
  if (width >= compactInner + 2 && height >= 5) {
    const word = "C O D E R";
    return framed(
      [
        texture(0, compactInner),
        padded(compactInner, [...word].length, `${CYAN}${BOLD}${word}${RESET}`),
        texture(1, compactInner),
      ],
      compactInner,
    );
  }

  // A viewport with no room for a frame at all still says whose screen it is.
  return [truncate("CODER", Math.max(1, width))];
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
  let screen: "chat" | "skills" | "child" = "chat";

  /**
   * Which half of the chat screen has the arrow keys.
   *
   * The composer, until the reader presses right. The sidebar is a list and
   * the composer is a field, and the two want the same four keys, so one of
   * them holds them at a time and the other shows that it does not.
   */
  let focus: "composer" | "sidebar" = "composer";
  /** The selected child, as an index into the sidebar's own order. */
  let sidebarRow = 0;
  /** The child whose transcript fills the screen, while one does. */
  let childId: string | undefined;
  /** Scroll position within that transcript, or the end when undefined. */
  let childAnchor: number | undefined;
  /** How many rows the child screen can show, for paging it. */
  let childViewport = 1;
  let childLines = 0;
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
  const pastedContents = new Map<number, PastedTextContent>();
  const pastedImages = new Map<number, PastedImageContent>();
  let nextPasteId = 1;
  let nextImageId = 1;
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
    const transcriptLines = (
      snapshot: CoderSnapshot,
      width: number,
      // The children are on screen already, in the column to the right. Drawing
      // them inline as well would say everything twice, and the feed is where
      // the reader is following the conversation rather than the fleet.
      sidebar: boolean,
    ): ReadonlyArray<string> => {
      const out: string[] = [];
      const body = Math.max(20, width - GUTTER - 1);

      // One blank row between entries. It is what keeps a tool call from
      // reading as part of the sentence before it.
      for (const entry of snapshot.entries) {
        // An entry that settled with nothing in it draws nothing. The dot is
        // the interface saying somebody spoke, and a dot beside an empty line
        // says it about a turn that never happened. A turn still streaming is
        // a different case: it has nothing yet and shows a caret.
        if (drawsNothing(entry)) continue;
        if (out.length > 0) out.push("");
        out.push(...renderEntry(entry, body, sidebar ? [] : snapshot.tasks));
      }
      return out;
    };

    /** Whether an entry would render as a bullet and a blank line. */
    const drawsNothing = (entry: CoderEntry): boolean =>
      entry.role !== "tool" && entry.settled && entry.text.length === 0;

    const renderEntry = (
      entry: CoderEntry,
      width: number,
      tasks: ReadonlyArray<CoderTask>,
    ): ReadonlyArray<string> => {
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

      // Follow Claude Code message markers: ⏺ for settled assistant/user turns, pulse ⏺/○ when streaming.
      const glyph = entry.settled ? "⏺" : pulse ? "⏺" : "○";
      const head = `  ${color}${glyph}${RESET} `;
      const continuation = " ".repeat(GUTTER);
      const rows = entryRows(entry, width, tasks);
      const caret = "";

      return rows.map((row, index) => {
        const tail = index === rows.length - 1 ? caret : "";
        return `${index === 0 ? head : continuation}${row}${tail}`;
      });
    };

    const entryRows = (
      entry: CoderEntry,
      width: number,
      tasks: ReadonlyArray<CoderTask>,
    ): ReadonlyArray<string> => {
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
      // A steered message the model has not been given yet, dim and italic:
      // the same styling reasoning gets, for the same reason — it is on screen
      // but it is not part of the conversation yet.
      if (entry.role === "you" && entry.pending === true) {
        return wrapStyled(entry.text, width, `${DIM}${ITALIC}`);
      }
      return wrapStyled(entry.text, width, entry.role === "notice" ? DIM : "");
    };

    const toolRows = (
      tool: CoderToolCall,
      width: number,
      open: boolean,
      tasks: ReadonlyArray<CoderTask>,
    ): ReadonlyArray<string> => {
      const mark =
        tool.status === "running"
          ? `${YELLOW}◐${RESET}`
          : tool.status === "failed"
            ? `${RED}✗${RESET}`
            : `${GREEN}✓${RESET}`;
      // Format tool header following Claude Code UI conventions: Tool(summary)
      const summary = clip(
        summarizeToolCall(tool.arguments, tool.name),
        Math.max(8, width - tool.name.length - 6),
      );
      const headerText = summary.length === 0 ? tool.name : `${tool.name}(${summary})`;
      const rows = [`${mark} ${BOLD}${headerText}${RESET}`];

      if (tool.name === "delegate" && tool.status === "running") {
        // Working children first when there are more than fit: a finished child
        // has already been reported on the transcript, so it is the one to drop.
        const shown =
          tasks.length <= FLEET_ROWS_MAX
            ? tasks
            : [
                ...tasks.filter((task) => task.status === "running" || task.status === "pending"),
                ...tasks.filter((task) => task.status !== "running" && task.status !== "pending"),
              ].slice(0, FLEET_ROWS_MAX);

        const childRows = fleetRows(shown, Math.max(20, width - 9));
        for (const [index, task] of shown.entries()) {
          const child = childRows[index];
          if (child === undefined) continue;
          const color = fleetColor(child.status);
          rows.push(
            `${DIM}${child.branch}${RESET} ${color}${child.mark}${RESET} ${DIM}${child.text}${RESET}`,
          );

          // Three lines of the child's own latest activity, nested under it.
          const activities = latestActivities([task], PREVIEW_ROWS);
          for (const activity of activities) {
            const rendered = activityRows(activity, Math.max(4, width - 12));
            const text = rendered[0] ?? "";
            rows.push(`${DIM}  → ${text}${RESET}`);
          }
        }

        const hidden = tasks.length - shown.length;
        if (hidden > 0) rows.push(`${DIM}   +${String(hidden)} more${RESET}`);
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
            const marker = index === 0 ? `${DIM}⎿ ${RESET}` : "  ";
            rows.push(`${marker}${DIM}${truncate(line, Math.max(4, width - 2))}${RESET}`);
          }
        }
        return rows;
      }

      if (tool.name !== "delegate" || tool.status !== "running") {
        const outcome =
          tool.error !== undefined
            ? `${RED}${clip(tool.error, Math.max(8, width - 4))}${RESET}`
            : tool.output !== undefined
              ? `${DIM}⎿ ${clip(tool.output, Math.max(8, width - 6))}${RESET}`
              : tool.status === "running" && tool.name !== "delegate"
                ? `${DIM}⎿ running…${RESET}`
                : "";
        if (outcome.length > 0) rows.push(outcome);
      }
      return rows;
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

    /**
     * The right column: what every child is doing, while any of them is.
     *
     * The same rows the inline block draws, one per child with its own latest
     * activity under it, but given a column of their own they do not push the
     * conversation off screen — a fan-out of fifteen used to take the whole
     * viewport at the moment the reader most wanted to see what the parent
     * had said.
     */
    /**
     * The order the column shows children in, and the order the selector moves
     * through. One function, because a selector that walks a different order
     * than the one on screen selects the wrong child.
     */
    const sidebarOrder = (tasks: ReadonlyArray<CoderTask>): ReadonlyArray<CoderTask> => [
      ...tasks.filter((task) => task.status === "running" || task.status === "pending"),
      ...tasks.filter((task) => task.status !== "running" && task.status !== "pending"),
    ];

    const sidebarLines = (
      tasks: ReadonlyArray<CoderTask>,
      height: number,
    ): ReadonlyArray<string> => {
      const inner = SIDEBAR_WIDTH - 2;
      const running = tasks.filter(
        (task) => task.status === "running" || task.status === "pending",
      ).length;

      const heading =
        running === 0
          ? `${BOLD}subagents${RESET}`
          : `${BOLD}subagents${RESET} ${DIM}${String(running)} working${RESET}`;

      const rows: string[] = [
        heading,
        focus === "sidebar" ? `${DIM}↑↓ select · enter opens · ← back${RESET}` : "",
      ];

      // Working children first. A finished one has already been reported on
      // the transcript, so it is the one to drop when the column runs out.
      const ordered = sidebarOrder(tasks);

      const built = fleetRows(ordered, inner);

      for (const [index, task] of ordered.entries()) {
        const row = built[index];
        if (row === undefined) continue;
        // Two rows per child at least — the row itself and one activity — so
        // the count is what decides how many fit rather than the cut.
        if (rows.length + 2 > height) break;

        const color = fleetColor(row.status);
        // The selector bar. Only while the column holds the keys, because a
        // highlight on a list that does not answer them is a lie about where
        // typing goes.
        const selected = focus === "sidebar" && index === sidebarRow;
        const name = truncate(task.description, inner - 2);
        rows.push(
          selected
            ? `${REVERSE}${color}${row.mark}${RESET}${REVERSE} ${name}${RESET}`
            : `${color}${row.mark}${RESET} ${name}`,
        );

        const activities = latestActivities([task], PREVIEW_ROWS);

        if (activities.length === 0) {
          // A child that has not called a tool yet still has a state, and an
          // empty cell under its name reads as a stalled child.
          rows.push(`${DIM}  ${truncate(taskActivity(task), inner - 2)}${RESET}`);
        }

        for (const activity of activities) {
          if (rows.length + 1 > height) break;
          const rendered = activityRows(activity, Math.max(4, inner - 4));
          const text = rendered[0] ?? "";
          rows.push(`${DIM}  → ${text}${RESET}`);
        }

        rows.push("");
      }

      const shown = ordered.filter((_task, index) => built[index] !== undefined).length;
      const hidden = tasks.length - Math.min(shown, tasks.length);
      if (hidden > 0 && rows.length < height) {
        rows.push(`${DIM}+${String(hidden)} more${RESET}`);
      }

      return rows;
    };

    /**
     * One child's transcript, filling the screen.
     *
     * A child reports one paragraph when it finishes, and until then the fleet
     * row says only what tool it is on. This is the rest of it: every event the
     * harness wrote, read from the file it is still appending to, so a reader
     * can watch a child work instead of waiting for its summary.
     */
    const childScreenLines = (task: CoderTask, width: number): ReadonlyArray<string> => {
      const rows: string[] = [];
      const body = Math.max(20, width - 4);

      rows.push(
        `${BOLD}${task.description}${RESET} ${DIM}${task.agent} · ${coderTierLabel(task.model)} · ${task.status}${RESET}`,
        "",
      );

      const entries = readChildTranscript(task.transcriptPath);

      if (entries.length === 0) {
        rows.push(
          task.transcriptPath === undefined
            ? `${DIM}This child has not started writing yet.${RESET}`
            : `${DIM}Nothing written yet.${RESET}`,
        );
      }

      for (const entry of entries) {
        switch (entry.kind) {
          case "started":
            rows.push(`${DIM}started in ${entry.cwd} on ${coderTierLabel(entry.model)}${RESET}`, "");
            break;

          case "tool":
            rows.push(
              `${YELLOW}▸${RESET} ${BOLD}${entry.name}${RESET}` +
                (entry.target === undefined
                  ? ""
                  : ` ${DIM}${truncate(entry.target, body - 6)}${RESET}`),
            );
            break;

          case "output":
            // Bounded per result. A child that cats a large file must not push
            // everything it did before that off the top of the screen.
            for (const line of entry.text.split("\n").slice(0, CHILD_OUTPUT_ROWS)) {
              rows.push(`  ${DIM}${truncate(line, body - 2)}${RESET}`);
            }
            if (entry.text.split("\n").length > CHILD_OUTPUT_ROWS) {
              rows.push(`  ${DIM}…${RESET}`);
            }
            break;

          case "text":
            rows.push("", ...wrapStyled(entry.text, body, ""), "");
            break;

          case "error":
            rows.push(...wrapStyled(entry.text, body, RED));
            break;
        }
      }

      // The answer, once there is one. It is what the parent was given, and a
      // reader who opened the child came for exactly this.
      if (task.result !== undefined && task.result.length > 0) {
        rows.push("", `${GREEN}result${RESET}`, ...wrapStyled(task.result, body, ""));
      }
      if (task.error !== undefined) {
        rows.push("", `${RED}failed${RESET}`, ...wrapStyled(task.error, body, RED));
      }

      return rows.map((row) => `  ${row}`);
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
      // One child, filling the screen. Left or escape returns to the chat, and
      // the chat is untouched underneath: this is a screen over it, not a
      // place the session went.
      if (screen === "child") {
        const task = snapshot.tasks.find((candidate) => candidate.id === childId);

        if (task === undefined) {
          // The child was cleared while its transcript was open.
          screen = "chat";
        } else {
          const room = Math.max(1, height - 2);
          const lines = childScreenLines(task, width);
          childLines = lines.length;
          childViewport = room;

          const last = Math.max(0, lines.length - room);
          const from = childAnchor === undefined ? last : Math.min(childAnchor, last);

          const rows: string[] = [];
          for (let row = 0; row < room; row += 1) rows.push(lines[from + row] ?? "");

          rows.push(
            `${DIM}${"─".repeat(Math.max(0, width))}${RESET}`,
            hints(
              [{ text: "↑↓ scroll" }, { text: "← back" }, { text: "esc back" }],
              `${DIM}${task.description}${RESET}`,
              width,
            ),
          );

          paint(rows, rows.length, 1);
          return;
        }
      }

      const transcriptHeight = Math.max(1, height - STATUS_ROWS - COMPOSER_ROWS - SPACER_ROWS);

      // The column appears when there is something to put in it and room to
      // put it. Both conditions are live: it opens when the first child starts
      // and closes when the last one is cleared, and a terminal resized narrow
      // gives the width back to the transcript.
      const sidebar = snapshot.tasks.length > 0 && width >= SIDEBAR_MINIMUM_TERMINAL;
      const transcriptWidth = sidebar ? width - SIDEBAR_WIDTH - 1 : width;

      // Before the first entry exists there is no conversation to draw, so
      // the viewport shows the splash instead of nothing. Gated on the entries
      // rather than on what they render to: a transcript whose entries all
      // settled empty is a conversation that happened, and greeting it again
      // would say that it had not.
      const lines =
        snapshot.entries.length === 0
          ? splashLines(transcriptWidth, transcriptHeight)
          : transcriptLines(snapshot, transcriptWidth, sidebar);
      lineCount = lines.length;
      viewport = transcriptHeight;

      const maxStart = Math.max(0, lines.length - transcriptHeight);
      const start = anchor === undefined ? maxStart : Math.min(anchor, maxStart);
      // Kept for the status line: a reader scrolled up with nothing saying so
      // reads a still transcript as a stopped session.
      const above = start;

      const column = sidebar ? sidebarLines(snapshot.tasks, transcriptHeight) : [];

      const rows: string[] = [];
      for (let row = 0; row < transcriptHeight; row += 1) {
        const left = lines[start + row] ?? "";
        if (!sidebar) {
          rows.push(left);
          continue;
        }

        // Padded to the column, because a styled row is longer in bytes than
        // it is on screen and the divider has to land in the same place on
        // every row or it is not a divider.
        const padded = left + " ".repeat(Math.max(0, transcriptWidth - visibleWidth(left)));
        rows.push(`${padded}${DIM}│${RESET} ${column[row] ?? ""}`);
      }

      // Bottom chrome, in the order a reader scans it. The status line is the
      // main agent's state; the delegate preview lives inline under the
      // delegate tool call. The composer follows in its own region between two
      // rules.
      const rule = `${DIM}${"─".repeat(Math.max(0, width))}${RESET}`;
      const inner = Math.max(10, width - 4);

      // Status line left side:
      // - Directory (replacing home directory with ~)
      // - Branch (with branch icon if present and not "no branch")
      // - Active state / scrolled indicator (no "ready" indicator when idle)
      const home = homedir();
      const formattedRepo =
        snapshot.repository === home
          ? "~"
          : snapshot.repository.startsWith(`${home}/`)
            ? `~${snapshot.repository.slice(home.length)}`
            : snapshot.repository;
      const formattedBranch =
        snapshot.branch === "no branch" || snapshot.branch.length === 0
          ? snapshot.branch
          : `⎇ ${snapshot.branch}`;

      const leftParts: string[] = [];
      if (formattedRepo.length > 0) leftParts.push(formattedRepo);
      if (formattedBranch.length > 0) leftParts.push(formattedBranch);
      if (snapshot.running) {
        leftParts.push(`${YELLOW}●${RESET} ${DIM}working… (${elapsed(runningSince, Date.now())})${RESET}`);
      }
      if (anchor !== undefined) {
        leftParts.push(`${DIM}scrolled ↑${String(above)}${RESET}`);
      }

      // Status line right side:
      // Model, reasoning, budget, goal.
      const rightParts = [snapshot.model];
      if (snapshot.reasoning !== undefined) rightParts.push(`thinking ${snapshot.reasoning}`);
      if (snapshot.budget !== undefined) rightParts.push(snapshot.budget);
      if (snapshot.goal !== undefined && snapshot.goal.status === "active") {
        const goalSnippet = snapshot.goal.objective.length > 25
          ? snapshot.goal.objective.slice(0, 22) + "…"
          : snapshot.goal.objective;
        rightParts.push(`goal: "${goalSnippet}"`);
      }

      let chosenLeft = "";
      let chosenRight = "";
      for (let leftFrom = 0; leftFrom <= leftParts.length; leftFrom += 1) {
        const leftCandidate =
          leftParts.slice(leftFrom).length > 0
            ? `${DIM}${leftParts.slice(leftFrom).join(" · ")}${RESET}`
            : "";
        let matched = false;
        for (let rightFrom = 0; rightFrom < rightParts.length; rightFrom += 1) {
          const rightCandidate = `${DIM}${rightParts.slice(rightFrom).join(" · ")}${RESET}`;
          const used =
            visibleWidth(leftCandidate) +
            visibleWidth(rightCandidate) +
            (leftCandidate.length > 0 && rightCandidate.length > 0 ? 2 : 0);
          if (used <= inner) {
            chosenLeft = leftCandidate;
            chosenRight = rightCandidate;
            matched = true;
            break;
          }
        }
        if (matched) break;
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
      rows.push(`  ${justify(chosenLeft, chosenRight, inner)}`);

      if (focus === "sidebar") {
        // On the selected child, because a cursor left blinking in a composer
        // that is not taking keys says the wrong thing about where typing goes.
        const heading = 2;
        const selectedRow = Math.min(transcriptHeight, heading + sidebarRow * 3 + 1);
        paint(rows, selectedRow, transcriptWidth + 2);
        return;
      }

      paint(rows, transcriptHeight + 3, 4 + [...visible].length + 1);
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
      const prompt = expandComposerPrompt(composer, pastedContents, pastedImages);
      composer = "";
      pastedContents.clear();
      pastedImages.clear();
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

      // `/resume` lists recent foreign coding-agent sessions or describes one.
      const resumeMatch = /^\/resume(?:\s+(\d+))?\s*$/.exec(prompt.trim());
      if (resumeMatch !== null) {
        const selection = resumeMatch[1] === undefined ? undefined : Number(resumeMatch[1]);
        if (options.resume === undefined) {
          session.notice("This session cannot resume foreign sessions.");
        } else {
          void options
            .resume(selection)
            .then((text) => {
              session.notice(text);
              render();
            })
            .catch(() => {
              session.notice("The foreign session scan failed unexpectedly.");
              render();
            });
        }
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
      else {
        composer = "";
        pastedContents.clear();
        pastedImages.clear();
      }
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
          const pastedText = text.slice(from, to);
          const state = {
            nextTextId: nextPasteId,
            nextImageId,
            pastedText: pastedContents,
            pastedImages,
          };
          composer += handleIncomingPasteChunk(pastedText, state);
          nextPasteId = state.nextTextId;
          nextImageId = state.nextImageId;
          index = to + PASTE_END.length;
          dirty = true;
          continue;
        }

        // The child screen takes the keyboard while it is up, for the same
        // reason the skills screen does: a stray letter must not fall through
        // into a composer the reader cannot see.
        if (screen === "child") {
          if (char === "\x1b") {
            const sequence = matchEscapeSequence(text, index);
            if (sequence === undefined) {
              pendingEscape = text.slice(index);
              break;
            }
            index += sequence.length;

            const page = Math.max(1, childViewport - 1);
            const scroll = (by: number) => {
              const last = Math.max(0, childLines - childViewport);
              const from = childAnchor ?? last;
              const next = Math.min(last, Math.max(0, from + by));
              childAnchor = next >= last ? undefined : next;
            };

            if (sequence === "\x1b" || sequence === "\x1b[D" || sequence === "\x1bOD") {
              // Back to the chat, and back into the column it was opened from,
              // so a reader stepping through children does not have to press
              // right again between each one.
              screen = "chat";
              focus = "sidebar";
              childAnchor = undefined;
              painted = [];
            } else if (sequence === "\x1b[A" || sequence === "\x1bOA") scroll(-1);
            else if (sequence === "\x1b[B" || sequence === "\x1bOB") scroll(1);
            else if (sequence === "\x1b[5~") scroll(-page);
            else if (sequence === "\x1b[6~") scroll(page);
            else continue;

            render();
            continue;
          }

          index += 1;
          if (char === "\x03" || char === "\x04") {
            screen = "chat";
            focus = "composer";
            childAnchor = undefined;
            painted = [];
            render();
          }
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
          // keyboard protocol reports. Shift+tab moves the Coder tier; tab
          // moves how hard the model is asked to think.
          if (sequence === "\x1b[Z" || sequence === "\x1b[9;2u") {
            session.cycleTier();
            dirty = true;
            continue;
          }

          // The column, when there is one. Right hands it the arrow keys;
          // left hands them back. Both are no-ops when no children are
          // running, so the keys never disappear into a column that is not on
          // screen.
          const children = sidebarOrder(session.snapshot().tasks);
          const columnOpen =
            children.length > 0 && (stdout.columns ?? 80) >= SIDEBAR_MINIMUM_TERMINAL;

          if (
            focus === "composer" &&
            columnOpen &&
            (sequence === "\x1b[C" || sequence === "\x1bOC")
          ) {
            focus = "sidebar";
            sidebarRow = Math.min(sidebarRow, children.length - 1);
            render();
            continue;
          }

          if (focus === "sidebar") {
            if (sequence === "\x1b[D" || sequence === "\x1bOD" || sequence === "\x1b") {
              focus = "composer";
            } else if (sequence === "\x1b[A" || sequence === "\x1bOA") {
              sidebarRow = Math.max(0, sidebarRow - 1);
            } else if (sequence === "\x1b[B" || sequence === "\x1bOB") {
              sidebarRow = Math.min(Math.max(0, children.length - 1), sidebarRow + 1);
            } else {
              continue;
            }
            render();
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

        // Typing is always typing. A reader who starts a sentence while the
        // column has the keys means to type it, not to lose it, so the
        // character brings the composer back rather than being swallowed.
        if (focus === "sidebar" && char >= " " && char !== "\x7f") {
          focus = "composer";
        }

        if (char === "\r" || char === "\n") {
          index += 1;
          // Swallow a CRLF pair so a paste does not submit twice.
          if (char === "\r" && text[index] === "\n") index += 1;

          // Enter belongs to whatever holds the arrow keys. In the column it
          // opens the selected child rather than sending the composer, which
          // would send a line the reader was not looking at.
          if (focus === "sidebar") {
            const selected = sidebarOrder(session.snapshot().tasks)[sidebarRow];
            if (selected !== undefined) {
              childId = selected.id;
              childAnchor = undefined;
              screen = "child";
              painted = [];
              render();
            }
            continue;
          }

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
        if (char === "\t" && session.canCycleReasoning) {
          session.cycleReasoning();
          dirty = false;
          index += 1;
          continue;
        }

        if (char === "\x7f" || char === "\b") {
          composer = backspaceComposer(composer);
          dirty = true;
          index += 1;
          continue;
        }

        if (char === "\x15") {
          composer = "";
          pastedContents.clear();
          pastedImages.clear();
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
