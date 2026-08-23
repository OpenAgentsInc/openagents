/**
 * The full-screen interface for `openagents coder`.
 *
 * This draws with ANSI escapes and Node's own standard streams, with no
 * rendering dependency. The design document proposes OpenTUI for this layer,
 * and OpenTUI cannot render here yet: its FFI is Bun-only (the package ships
 * `index.bun.js` beside a `.dylib` and no N-API entry point), so
 * `createCliRenderer` fails on Node with "OpenTUI native FFI is not available
 * for this runtime yet". The CLI installs as an npm package that people run on
 * Node, so the interface cannot require Bun. See `docs/2026-08-23-openagents-
 * coder-cli-spec.md` section 3 and its open questions.
 *
 * The layout follows that document:
 *
 *     ┌──────────────────────────────┐
 *     │ transcript, scrollable       │
 *     ├──────────────────────────────┤
 *     │ status  repo · branch · model│
 *     ├──────────────────────────────┤
 *     │ composer                     │
 *     └──────────────────────────────┘
 */

import type { CoderEntry, CoderSession, CoderSnapshot } from "./coder-session.js";

const ALT_SCREEN_ON = "\x1b[?1049h";
const ALT_SCREEN_OFF = "\x1b[?1049l";
const CURSOR_HIDE = "\x1b[?25l";
const CURSOR_SHOW = "\x1b[?25h";
const CLEAR = "\x1b[2J";

const DIM = "\x1b[2m";
const BOLD = "\x1b[1m";
const RESET = "\x1b[0m";
const CYAN = "\x1b[36m";
const GREEN = "\x1b[32m";
const YELLOW = "\x1b[33m";

const STATUS_ROWS = 1;
const COMPOSER_ROWS = 3;
/** Escape arms interruption for this long, so an arrow key is not an escape. */
const INTERRUPT_WINDOW_MS = 5000;

export interface CoderUiOptions {
  readonly stdin: NodeJS.ReadStream;
  readonly stdout: NodeJS.WriteStream;
}

/** Wrap one paragraph to the available width, preserving blank lines. */
function wrap(text: string, width: number): ReadonlyArray<string> {
  const lines: string[] = [];
  for (const paragraph of text.split("\n")) {
    if (paragraph.length === 0) {
      lines.push("");
      continue;
    }
    let current = "";
    for (const word of paragraph.split(" ")) {
      if (current.length === 0) {
        current = word;
      } else if (current.length + 1 + word.length <= width) {
        current += ` ${word}`;
      } else {
        lines.push(current);
        current = word;
      }
    }
    lines.push(current);
  }
  return lines;
}

/**
 * Match a complete escape sequence at `index`, or return undefined for a bare
 * escape. Covers CSI (`\x1b[…final`), SS3 (`\x1bO…`), and the `\x1b[…~` forms
 * that carry PageUp and PageDown.
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

  return undefined;
}

/**
 * Run the interface until the user exits. Resolves with the process exit code.
 */
export function runCoderUi(session: CoderSession, options: CoderUiOptions): Promise<number> {
  const { stdin, stdout } = options;

  let composer = "";
  let scrollOffset = 0;
  let escapeArmedAt = 0;
  let armedNotice = false;
  let exitCode = 0;
  let closed = false;

  const write = (text: string) => {
    stdout.write(text);
  };

  return new Promise<number>((resolve) => {
    const finish = (code: number) => {
      if (closed) return;
      closed = true;
      exitCode = code;
      unsubscribe();
      stdin.off("data", onData);
      stdout.off("resize", render);
      if (stdin.isTTY) stdin.setRawMode(false);
      stdin.pause();
      write(CURSOR_SHOW + ALT_SCREEN_OFF);
      resolve(exitCode);
    };

    /** Turn the transcript into printable lines, newest last. */
    const transcriptLines = (snapshot: CoderSnapshot, width: number): ReadonlyArray<string> => {
      const out: string[] = [];
      const body = Math.max(20, width - 12);

      for (const entry of snapshot.entries) {
        if (out.length > 0) out.push("");
        out.push(...renderEntry(entry, body));
      }
      return out;
    };

    const renderEntry = (entry: CoderEntry, width: number): ReadonlyArray<string> => {
      const label =
        entry.role === "you"
          ? `${CYAN}${BOLD}you${RESET}`
          : entry.role === "assistant"
            ? `${GREEN}${BOLD}coder${RESET}`
            : `${YELLOW}${BOLD}note${RESET}`;

      const text = entry.text.length === 0 && !entry.settled ? "…" : entry.text;
      const wrapped = wrap(text, width);
      const caret = entry.settled ? "" : `${DIM}▌${RESET}`;

      return wrapped.map((line, index) => {
        const gutter = index === 0 ? label : "     ";
        const tail = index === wrapped.length - 1 ? caret : "";
        const pad = index === 0 ? "  " : "";
        return `  ${gutter}${pad} ${line}${tail}`;
      });
    };

    const render = () => {
      if (closed) return;
      const snapshot = session.snapshot();
      const width = stdout.columns ?? 80;
      const height = stdout.rows ?? 24;
      const transcriptHeight = Math.max(1, height - STATUS_ROWS - COMPOSER_ROWS - 1);

      const lines = transcriptLines(snapshot, width);
      const maxOffset = Math.max(0, lines.length - transcriptHeight);
      if (scrollOffset > maxOffset) scrollOffset = maxOffset;
      const start = Math.max(0, maxOffset - scrollOffset);
      const visible = lines.slice(start, start + transcriptHeight);

      const frame: string[] = [];
      frame.push(CURSOR_HIDE, CLEAR, "\x1b[H");

      for (let row = 0; row < transcriptHeight; row += 1) {
        frame.push(`\x1b[${row + 1};1H`, visible[row] ?? "");
      }

      // Separator, status line, separator.
      const rule = "─".repeat(Math.max(0, width));
      frame.push(`\x1b[${transcriptHeight + 1};1H`, `${DIM}${rule}${RESET}`);

      const scrolled = scrollOffset > 0 ? `  ·  scrolled ${scrollOffset}` : "";
      const state = snapshot.running ? `${YELLOW}working${RESET}` : `${DIM}ready${RESET}`;
      const status =
        `  ${snapshot.repository} ${DIM}·${RESET} ${snapshot.branch} ` +
        `${DIM}·${RESET} ${snapshot.model} ${DIM}·${RESET} ${snapshot.turns} replies ` +
        `${DIM}·${RESET} ${state}${DIM}${scrolled}${RESET}`;
      frame.push(`\x1b[${transcriptHeight + 2};1H`, status);
      frame.push(`\x1b[${transcriptHeight + 3};1H`, `${DIM}${rule}${RESET}`);

      const hint = snapshot.running
        ? `${DIM}esc esc interrupt${RESET}`
        : `${DIM}enter send  ·  ctrl+d quit${RESET}`;
      const armed = armedNotice ? `  ${YELLOW}again to interrupt${RESET}` : "";
      const promptPrefix = "  › ";
      frame.push(`\x1b[${transcriptHeight + 4};1H`, `${promptPrefix}${composer}`);
      frame.push(`\x1b[${transcriptHeight + 5};1H`, `  ${hint}${armed}`);

      // Park the cursor at the composer so typing looks right.
      frame.push(`\x1b[${transcriptHeight + 4};${promptPrefix.length + composer.length + 1}H`);
      frame.push(CURSOR_SHOW);
      write(frame.join(""));
    };

    const submit = () => {
      const prompt = composer;
      composer = "";
      scrollOffset = 0;
      render();
      void session.submit(prompt);
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
      const text = typeof chunk === "string" ? chunk : chunk.toString("utf8");
      let index = 0;
      let dirty = false;

      const disarm = () => {
        if (armedNotice) {
          armedNotice = false;
          dirty = true;
        }
      };

      while (index < text.length) {
        const char = text[index] ?? "";

        if (char === "\x1b") {
          const sequence = matchEscapeSequence(text, index);
          if (sequence !== undefined) {
            disarm();
            index += sequence.length;
            if (sequence === "\x1b[5~") {
              scrollOffset += 5;
              dirty = true;
            } else if (sequence === "\x1b[6~") {
              scrollOffset = Math.max(0, scrollOffset - 5);
              dirty = true;
            }
            // Every other sequence (arrows, home, end) is ignored rather than
            // typed into the composer.
            continue;
          }

          // A bare escape. The first arms interruption, the second inside the
          // window performs it. Without the window an arrow key's leading byte
          // is indistinguishable from a deliberate escape.
          const now = Date.now();
          if (armedNotice && now - escapeArmedAt <= INTERRUPT_WINDOW_MS) {
            armedNotice = false;
            if (!session.interrupt()) composer = "";
          } else {
            escapeArmedAt = now;
            armedNotice = true;
          }
          dirty = true;
          index += 1;
          continue;
        }

        disarm();

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
        dirty = true;
        index = end;
      }

      if (dirty) render();
    };

    const unsubscribe = session.onChange(render);

    write(ALT_SCREEN_ON);
    if (stdin.isTTY) stdin.setRawMode(true);
    stdin.resume();
    stdin.setEncoding("utf8");
    stdin.on("data", onData);
    stdout.on("resize", render);

    session.notice(
      "openagents coder — development build. Type a message and press enter. " +
        "Ctrl+D quits, Esc Esc interrupts a reply.",
    );
    render();
  });
}
