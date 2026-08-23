/**
 * Markdown to ANSI for the coder transcript.
 *
 * Replies arrive as Markdown and used to be printed as source, so the reader
 * saw `**ox-alpha**` rather than a bold name. This turns the source into
 * styled, width-wrapped rows.
 *
 * Two properties of the caller shape every decision here:
 *
 * - The text arrives in chunks. Every frame re-renders the whole entry from
 *   whatever has arrived, so a half-arrived `**` or an unclosed fence is the
 *   normal case rather than an error. Unterminated markup renders as the
 *   literal characters that arrived, which means the row never loses a
 *   character and never flickers between two different readings of the same
 *   prefix.
 * - Rows are laid out by the interface, which owns the gutter. Wrapping
 *   therefore works on visible width with ANSI ignored, and a wrapped list
 *   item continues under its text rather than under its bullet.
 *
 * There is no Markdown dependency. The subset below is what a reply uses, and
 * a parser small enough to test directly is worth more here than a general one
 * that would still need a streaming and a wrapping layer on top.
 */

const RESET = "\x1b[0m";
const BOLD = "\x1b[1m";
const DIM = "\x1b[2m";
const ITALIC = "\x1b[3m";
const CODE = "\x1b[36m";
const HEADING = "\x1b[1m\x1b[36m";

/** A run of text that shares one ANSI prefix. */
export interface StyledSpan {
  readonly text: string;
  readonly style: string;
}

/** Visible width, ignoring ANSI styling. */
export function visibleWidth(text: string): number {
  return [...text.replace(/\x1b\[[0-9;]*m/g, "")].length;
}

/** Render one text block with a single style, wrapped like a paragraph. */
export function wrapStyled(text: string, width: number, style: string): ReadonlyArray<string> {
  const rows: string[] = [];
  for (const paragraph of text.split("\n")) {
    if (paragraph.length === 0) {
      rows.push("");
      continue;
    }
    rows.push(...wrapSpans([{ text: paragraph, style }], width, "", ""));
  }
  return rows;
}

/** Render Markdown source as styled rows no wider than `width`. */
export function renderMarkdown(text: string, width: number): ReadonlyArray<string> {
  const rows: string[] = [];
  /** The fence marker that opened the current code block, if one is open. */
  let fence: string | undefined;

  for (const line of text.split("\n")) {
    const fenced = /^\s*(```+|~~~+)/.exec(line);

    if (fence !== undefined) {
      // A fence closes only on its own marker, so a ``` inside a ~~~ block is
      // content rather than a terminator.
      if (fenced !== null && line.trim().startsWith(fence)) {
        fence = undefined;
        continue;
      }
      rows.push(...codeRows(line, width));
      continue;
    }

    if (fenced !== null) {
      fence = fenced[1] ?? "```";
      continue;
    }

    rows.push(...blockRows(line, width));
  }

  return rows;
}

/** One non-fenced source line as one or more rendered rows. */
function blockRows(line: string, width: number): ReadonlyArray<string> {
  if (line.trim().length === 0) return [""];

  if (/^\s*([-*_])\s*(\1\s*){2,}$/.test(line)) {
    return [`${DIM}${"─".repeat(Math.max(1, Math.min(width, 24)))}${RESET}`];
  }

  const heading = /^\s*(#{1,6})\s+(.*)$/.exec(line);
  if (heading !== null) {
    return wrapSpans(scan(heading[2] ?? "", HEADING), width, "", "");
  }

  const quote = /^\s*>\s?(.*)$/.exec(line);
  if (quote !== null) {
    const bar = `${DIM}│${RESET} `;
    return wrapSpans(scan(quote[1] ?? "", DIM), width, bar, bar);
  }

  const bullet = /^(\s*)([-*+])\s+(.*)$/.exec(line);
  if (bullet !== null) {
    const indent = " ".repeat(bullet[1]?.length ?? 0);
    return wrapSpans(scan(bullet[3] ?? "", ""), width, `${indent}${DIM}•${RESET} `, `${indent}  `);
  }

  const numbered = /^(\s*)(\d{1,9}[.)])\s+(.*)$/.exec(line);
  if (numbered !== null) {
    const indent = " ".repeat(numbered[1]?.length ?? 0);
    const marker = numbered[2] ?? "1.";
    return wrapSpans(
      scan(numbered[3] ?? "", ""),
      width,
      `${indent}${DIM}${marker}${RESET} `,
      `${indent}${" ".repeat(marker.length + 1)}`,
    );
  }

  return wrapSpans(scan(line, ""), width, "", "");
}

/** A line inside a fenced block: never wrapped by word, only hard-split. */
function codeRows(line: string, width: number): ReadonlyArray<string> {
  const body = Math.max(4, width - 2);
  const expanded = line.replace(/\t/g, "  ");
  const rows: string[] = [];
  let rest = [...expanded];
  do {
    const piece = rest.slice(0, body).join("");
    rows.push(`${DIM}│${RESET} ${CODE}${piece}${RESET}`);
    rest = rest.slice(body);
  } while (rest.length > 0);
  return rows;
}

/**
 * Split inline Markdown into styled spans.
 *
 * Every construct is matched by finding its terminator first. When the
 * terminator has not arrived the opening characters are kept as literal text,
 * which is what makes a half-streamed `**bold` read as `**bold` rather than
 * swallowing the rest of the reply.
 */
function scan(text: string, style: string): ReadonlyArray<StyledSpan> {
  const spans: StyledSpan[] = [];
  let buffer = "";

  const flush = () => {
    if (buffer.length === 0) return;
    spans.push({ text: buffer, style });
    buffer = "";
  };

  let index = 0;
  while (index < text.length) {
    const char = text[index] ?? "";
    const next = text[index + 1];

    if (char === "\\" && next !== undefined && /[\\`*_~[\]()#+\-.!>]/.test(next)) {
      buffer += next;
      index += 2;
      continue;
    }

    if (char === "`") {
      const end = text.indexOf("`", index + 1);
      if (end > index + 1) {
        flush();
        spans.push({ text: text.slice(index + 1, end), style: `${style}${CODE}` });
        index = end + 1;
        continue;
      }
      buffer += char;
      index += 1;
      continue;
    }

    const strong = text.startsWith("**", index)
      ? "**"
      : text.startsWith("__", index)
        ? "__"
        : undefined;
    if (strong !== undefined) {
      const end = text.indexOf(strong, index + 2);
      if (end > index + 2) {
        flush();
        spans.push(...scan(text.slice(index + 2, end), `${style}${BOLD}`));
        index = end + 2;
        continue;
      }
      buffer += strong;
      index += 2;
      continue;
    }

    if ((char === "*" || char === "_") && opensEmphasis(text, index, char)) {
      const end = closesEmphasis(text, index + 1, char);
      if (end !== undefined) {
        flush();
        spans.push(...scan(text.slice(index + 1, end), `${style}${ITALIC}`));
        index = end + 1;
        continue;
      }
    }

    buffer += char;
    index += 1;
  }

  flush();
  return spans;
}

const WORD = /[\p{L}\p{N}]/u;

/** An opener needs text after it, and `_` also needs a boundary before it. */
function opensEmphasis(text: string, index: number, marker: string): boolean {
  const after = text[index + 1];
  if (after === undefined || /\s/.test(after)) return false;
  if (marker !== "_") return true;
  const before = text[index - 1];
  return before === undefined || !WORD.test(before);
}

/** The matching terminator, or undefined when it has not arrived yet. */
function closesEmphasis(text: string, from: number, marker: string): number | undefined {
  for (let index = from; index < text.length; index += 1) {
    if (text[index] !== marker) continue;
    const before = text[index - 1];
    if (before === undefined || /\s/.test(before)) continue;
    if (marker === "_") {
      const after = text[index + 1];
      if (after !== undefined && WORD.test(after)) continue;
    }
    return index;
  }
  return undefined;
}

/**
 * Greedy word wrap over styled spans.
 *
 * Wrapping happens on visible width so styling never shifts the right edge,
 * and the continuation prefix is separate from the first one so a wrapped list
 * item lines up under its own text.
 */
export function wrapSpans(
  spans: ReadonlyArray<StyledSpan>,
  width: number,
  first: string,
  continuation: string,
): ReadonlyArray<string> {
  const rows: string[] = [];
  let prefix = first;
  let line: StyledSpan[] = [];
  let used = 0;
  /** A space seen between two words, carrying the style of the span it came from. */
  let pendingSpace: string | undefined;

  const room = () => Math.max(4, width - visibleWidth(prefix));

  const emit = () => {
    rows.push(prefix + merge(line).map(paint).join(""));
    prefix = continuation;
    line = [];
    used = 0;
    pendingSpace = undefined;
  };

  for (const span of spans) {
    for (const piece of span.text.split(/(\s+)/)) {
      if (piece.length === 0) continue;
      if (/^\s+$/.test(piece)) {
        if (used > 0) pendingSpace = span.style;
        continue;
      }

      let word = [...piece];
      while (word.length > 0) {
        const gap = pendingSpace !== undefined && used > 0 ? 1 : 0;
        const available = room() - used - gap;

        if (word.length > available && used > 0) {
          emit();
          continue;
        }

        // A word wider than a whole row is split rather than dropped.
        const take = word.length > room() ? room() : word.length;
        if (pendingSpace !== undefined && used > 0) {
          // The space keeps the style of the span it came from, so a styled
          // run stays one escape sequence and a boundary space stays plain.
          line.push({ text: " ", style: pendingSpace });
          used += 1;
        }
        pendingSpace = undefined;
        line.push({ text: word.slice(0, take).join(""), style: span.style });
        used += take;
        word = word.slice(take);
        if (word.length > 0) emit();
      }
    }
  }

  emit();
  return rows;
}

/** Join neighbours that share a style, so a styled run is one escape sequence. */
function merge(spans: ReadonlyArray<StyledSpan>): ReadonlyArray<StyledSpan> {
  const out: StyledSpan[] = [];
  for (const span of spans) {
    const last = out.at(-1);
    if (last !== undefined && last.style === span.style)
      out[out.length - 1] = { text: last.text + span.text, style: last.style };
    else out.push(span);
  }
  return out;
}

function paint(span: StyledSpan): string {
  return span.style.length === 0 ? span.text : `${span.style}${span.text}${RESET}`;
}
