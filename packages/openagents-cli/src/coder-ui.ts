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
 * The layout follows that document:
 *
 *     ┌──────────────────────────────┐
 *     │ transcript, scrollable       │
 *     ├──────────────────────────────┤
 *     │ status  repo · branch · model · budget│
 *     ├──────────────────────────────┤
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

import { fleetPhrase, fleetRows } from "./coder-fleet.js";
import { renderMarkdown, visibleWidth, wrapStyled } from "./coder-markdown.js";
import type { CoderEntry, CoderSession, CoderSnapshot, CoderToolCall } from "./coder-session.js";
import type { CoderTaskStatus } from "./coder-tasks.js";

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
const COMPOSER_ROWS = 3;
/**
 * Rows the fleet block may take before it scrolls internally.
 *
 * A fleet is a status display, not the content: a 30-way fan-out must not push
 * the transcript off the screen. Past this many children the block shows the
 * ones that are still working and counts the rest.
 */
const FLEET_ROWS_MAX = 8;
/** Width of the role gutter, so every entry's text starts in one column. */
const GUTTER = 9;
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

/** A key hint. A pinned one is kept even when the row has to give something up. */
interface Hint {
  readonly text: string;
  readonly pinned?: boolean;
}

/**
 * Lay out the key hints against the counter, dropping hints from the end until
 * the row fits.
 *
 * The counter is state the reader is trying to read; the hints are reminders
 * of keys that work whether or not they are printed. So the hints are what
 * gives way. Padding the two apart and dropping the counter instead is how a
 * wide-enough terminal still managed to hide the reply count.
 *
 * A pinned hint outranks the counter, because a key that stops fifteen agents
 * from spending is not a reminder. Ordering the hints so the stop came before
 * the conveniences was not enough: at eighty columns the counter grows as the
 * transcript scrolls, and the row dropped every hint at once.
 */
function hints(keys: ReadonlyArray<Hint>, right: string, width: number): string {
  const shown = [...keys];
  for (;;) {
    const left = `${DIM}${shown.map((key) => key.text).join(" · ")}${RESET}`;
    if (shown.length > 0 && visibleWidth(left) + visibleWidth(right) + 2 <= width) {
      return justify(left, right, width);
    }
    const droppable = shown.reduce<number>(
      (last, key, index) => (key.pinned === true ? last : index),
      -1,
    );
    if (droppable < 0) break;
    shown.splice(droppable, 1);
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
  /** Redraws the status line once a second so the elapsed time advances. */
  let ticker: NodeJS.Timeout | undefined;
  /** Rows as last painted, so only what changed is written. */
  let painted: string[] = [];
  /** Geometry from the last paint, which is what the scroll keys act on. */
  let lineCount = 0;
  let viewport = 1;
  /** Bytes held back because they may be the start of an escape sequence. */
  let pendingEscape = "";
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
      write(CURSOR_SHOW + ALT_SCROLL_OFF + ALT_SCREEN_OFF);
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
        out.push(...renderEntry(entry, body));
      }
      return out;
    };

    const renderEntry = (entry: CoderEntry, width: number): ReadonlyArray<string> => {
      const [label, color] =
        entry.role === "you"
          ? ["you", CYAN]
          : entry.role === "assistant"
            ? ["coder", GREEN]
            : entry.role === "tool"
              ? ["tool", MAGENTA]
              : entry.role === "reasoning"
                ? ["think", DIM]
                : ["note", YELLOW];

      const head = `  ${color}${BOLD}${label}${RESET}${" ".repeat(GUTTER - 2 - label.length)}`;
      const continuation = " ".repeat(GUTTER);
      const rows = entryRows(entry, width);
      const caret = entry.settled ? "" : `${DIM}▌${RESET}`;

      return rows.map((row, index) => {
        const tail = index === rows.length - 1 ? caret : "";
        return `${index === 0 ? head : continuation}${row}${tail}`;
      });
    };

    const entryRows = (entry: CoderEntry, width: number): ReadonlyArray<string> => {
      if (entry.role === "tool" && entry.tool !== undefined) {
        return toolRows(entry.tool, width, expanded.has(entry.tool.callId));
      }
      if (entry.text.length === 0 && !entry.settled) return ["…"];
      // Reasoning is dim italic rather than Markdown. The styling already says
      // what the text is, and emphasis nested inside italic reads worse than
      // the source it came from.
      if (entry.role === "reasoning") return wrapStyled(entry.text, width, `${DIM}${ITALIC}`);
      if (entry.role === "assistant") return renderMarkdown(entry.text, width);
      return wrapStyled(entry.text, width, entry.role === "notice" ? DIM : "");
    };

    const toolRows = (tool: CoderToolCall, width: number, open: boolean): ReadonlyArray<string> => {
      const mark =
        tool.status === "running"
          ? `${YELLOW}◐${RESET}`
          : tool.status === "failed"
            ? `${RED}✗${RESET}`
            : `${GREEN}✓${RESET}`;
      const rows = [`${mark} ${BOLD}${tool.name}${RESET}`];

      if (!open) {
        const args = clip(tool.arguments, Math.max(8, width - 4));
        if (args.length > 0) rows.push(`${DIM}${args}${RESET}`);
        const outcome =
          tool.error !== undefined
            ? `${RED}${clip(tool.error, Math.max(8, width - 4))}${RESET}`
            : tool.output !== undefined
              ? `${DIM}→ ${clip(tool.output, Math.max(8, width - 6))}${RESET}`
              : tool.status === "running"
                ? `${DIM}→ running…${RESET}`
                : "";
        if (outcome.length > 0) rows.push(outcome);
        return rows;
      }

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

    const render = () => {
      if (closed) return;
      const snapshot = session.snapshot();
      const width = stdout.columns ?? 80;
      const height = stdout.rows ?? 24;
      const transcriptHeight = Math.max(1, height - STATUS_ROWS - COMPOSER_ROWS - 1);

      const fleet = fleetLines(snapshot, width);
      // The fleet takes its rows from the transcript, not from the chrome: the
      // status line and composer stay where the reader's hands expect them.
      const transcriptRows = Math.max(1, transcriptHeight - fleet.length);

      const lines = transcriptLines(snapshot, width);
      lineCount = lines.length;
      viewport = transcriptRows;

      const maxStart = Math.max(0, lines.length - transcriptRows);
      const start = anchor === undefined ? maxStart : Math.min(anchor, maxStart);
      const above = start;
      const below = Math.max(0, lines.length - start - transcriptRows);

      const rows: string[] = [];
      for (let row = 0; row < transcriptRows; row += 1) rows.push(lines[start + row] ?? "");
      rows.push(...fleet);

      // Bottom chrome, in the order a reader scans it: what the session is
      // doing now, then where the typing goes, then what the keys do. The
      // composer sits between two rules so it reads as its own region rather
      // than as the last line of the transcript.
      const rule = `${DIM}${"─".repeat(Math.max(0, width))}${RESET}`;
      const inner = Math.max(10, width - 4);

      const phrase = fleetPhrase(snapshot.tasks);
      // The elapsed time and nothing else. This said `streaming` until the
      // reply source became the inference proxy, which builds the whole body
      // and sends it once: a turn that shows one block after four silent
      // seconds was never streaming, and the status line must not say it was.
      const chatActivity = snapshot.running
        ? `${YELLOW}●${RESET} working… ${DIM}(${elapsed(runningSince, Date.now())})${RESET}`
        : `${DIM}○ ready${RESET}`;
      // The fleet is named on the status line even though the block above lists
      // it, because the block is what gives way first on a short terminal and
      // the count is the part the reader is waiting on.
      const activity =
        phrase === undefined ? chatActivity : `${chatActivity} ${DIM}· ${phrase}${RESET}`;
      // Dropped from the left as the terminal narrows, because that is the
      // order of what a reader cannot recover elsewhere: they can see which
      // checkout they are in, they can ask git for the branch, and nothing on
      // screen but this says which model answers or what the thread has left.
      const facts = [snapshot.repository, snapshot.branch, snapshot.model];
      if (snapshot.budget !== undefined) facts.push(snapshot.budget);
      let where = "";
      for (let from = 0; from < facts.length; from += 1) {
        const candidate = `${DIM}${facts.slice(from).join(" · ")}${RESET}`;
        if (visibleWidth(activity) + visibleWidth(candidate) + 2 > inner) continue;
        where = candidate;
        break;
      }
      rows.push(`  ${justify(activity, where, inner)}`);
      rows.push(rule);
      // The composer shows its tail, never more characters than the row holds.
      // A row written past the last column makes the terminal wrap it, which
      // pushes the rule and the hints down a line and leaves the text they used
      // to occupy on screen with nothing to erase it.
      const composerRoom = Math.max(4, width - 4);
      const typed = [...composer];
      const visible =
        typed.length > composerRoom
          ? `…${typed.slice(typed.length - composerRoom + 1).join("")}`
          : composer;
      rows.push(`  › ${visible}`);
      rows.push(rule);

      // Every key named here does something in the state it is named in, and
      // they are listed in the order a reader needs them, because a narrow row
      // drops them from the end. An earlier version offered "esc esc to
      // interrupt" while idle, where there was nothing to interrupt.
      const keys: Hint[] = [];
      if (snapshot.running) {
        keys.push({ text: "esc to interrupt" }, { text: "ctrl+c to stop" });
      } else {
        keys.push({ text: "enter to send" });
        if (composer.length > 0) keys.push({ text: "esc to clear" });
        else keys.push({ text: "ctrl+d to quit" });
      }
      // Stopping the fleet is pinned rather than merely early: the row is
      // clipped from the end and this hint only appears while children are
      // spending, so an unpinned one went exactly when it applied.
      if (snapshot.tasks.some((task) => task.status === "running")) {
        keys.push({ text: "ctrl+x to stop agents", pinned: true });
      }
      // Only when there is another model to switch to, and only while nothing
      // is running: a turn already accepted keeps the backend it named.
      if (session.canCycleBackend && !snapshot.running) keys.push({ text: "tab to switch model" });
      if (lines.length > transcriptRows) keys.push({ text: "pgup/pgdn to scroll" });
      if (focusedTool(snapshot) !== undefined) keys.push({ text: "ctrl+o to expand" });

      // `this run` is not decoration. The count is this process's, and a
      // source that is not the thread — the stand-in behind `--offline` — has
      // no ceiling the number could be read against, so an unlabelled figure
      // would invite the reader to compare it with a budget beside it.
      const replies = `${snapshot.turns} ${snapshot.turns === 1 ? "reply" : "replies"} this run`;
      const counter =
        anchor !== undefined
          ? `${YELLOW}scrolled${RESET}${DIM} · ↑${above} · ↓${below}${RESET}`
          : above > 0
            ? `${DIM}↑${above} above · ${replies}${RESET}`
            : `${DIM}${replies}${RESET}`;
      rows.push(`  ${hints(keys, counter, inner)}`);

      paint(rows, transcriptRows + fleet.length + 3, 4 + [...visible].length + 1);
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

    const submit = () => {
      const prompt = composer;
      composer = "";
      anchor = undefined;
      runningSince = Date.now();
      render();

      // The elapsed time has to advance between chunks, not only when one
      // arrives, or a slow reply looks stalled.
      ticker ??= setInterval(() => {
        session.pruneTasks();
        if (session.running || session.snapshot().tasks.length > 0) render();
      }, 1000);

      // A delegate line is not a turn: it returns as soon as the children are
      // submitted and each one reports later, so nothing here waits on it and
      // the ticker above keeps the fleet rows moving.
      if (prompt.trimStart().startsWith("/delegate")) {
        void session.submit(prompt);
        render();
        return;
      }

      void session.submit(prompt).finally(() => {
        if (ticker !== undefined) {
          clearInterval(ticker);
          ticker = undefined;
        }
        render();
      });
    };

    /** A lone escape: interrupt if there is something to interrupt, else clear. */
    const onEscape = () => {
      if (!session.interrupt()) composer = "";
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
      const text = pendingEscape + (typeof chunk === "string" ? chunk : chunk.toString("utf8"));
      pendingEscape = "";
      let index = 0;
      let dirty = false;

      while (index < text.length) {
        const char = text[index] ?? "";

        if (char === "\x1b") {
          const sequence = matchEscapeSequence(text, index);
          if (sequence === undefined) {
            // The rest of the sequence has not arrived. Hold it: if nothing
            // follows within the window it was a bare escape after all.
            pendingEscape = text.slice(index);
            break;
          }
          index += sequence.length;
          if (sequence === "\x1b") {
            onEscape();
            dirty = false;
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
          if (!session.running) {
            submit();
            dirty = false;
          }
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

    write(ALT_SCREEN_ON + ALT_SCROLL_ON);
    if (stdin.isTTY) stdin.setRawMode(true);
    stdin.resume();
    stdin.setEncoding("utf8");
    stdin.on("data", onData);
    stdout.on("resize", onResize);

    session.notice(
      "openagents coder — development build. Type a message and press enter. " +
        "Ctrl+D quits, Esc interrupts a reply. `/system` shows what the model is told.",
    );
    render();
  });
}
