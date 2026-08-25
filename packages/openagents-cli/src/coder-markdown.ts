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
export function renderMarkdown(
  text: string,
  width: number,
  /**
   * A style every row sits in, restored after each reset the markup emits.
   *
   * Without this a caller cannot render Markdown inside a style of its own:
   * the first bold or code span ends with a reset, and everything after it on
   * that row loses the dim or the italic the caller asked for.
   */
  base = "",
): ReadonlyArray<string> {
  const rows: string[] = [];
  /** The fence marker that opened the current code block, if one is open. */
  let fence: string | undefined;

  const source = text.split("\n");
  for (let at = 0; at < source.length; at += 1) {
    const line = source[at] ?? "";
    const fenced = /^\s*(```+|~~~+)/.exec(line);

    // A table is a block, not a run of lines, so it is taken whole before the
    // line-at-a-time path sees it as seven paragraphs of pipes.
    if (
      fence === undefined &&
      fenced === null &&
      isTableRow(line) &&
      isTableRule(source[at + 1] ?? "")
    ) {
      const block: string[] = [line, source[at + 1] ?? ""];
      let end = at + 2;
      while (end < source.length && isTableRow(source[end] ?? "")) {
        block.push(source[end] ?? "");
        end += 1;
      }
      rows.push(...tableRows(block, width));
      at = end - 1;
      continue;
    }

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

  if (base.length === 0) return rows;
  // Reapplied after every reset the markup wrote, and opened again on each row,
  // because a row is painted on its own and carries no style from the last.
  return rows.map((row) => `${base}${row.replaceAll(RESET, `${RESET}${base}`)}${RESET}`);
}

/** One non-fenced source line as one or more rendered rows. */

/** A row of cells, as written between pipes. */
const tableCells = (line: string): ReadonlyArray<string> =>
  line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => cell.trim());

/** True for the `| --- | :--: |` line that makes the row above a header. */
const isTableRule = (line: string): boolean => {
  const cells = tableCells(line);
  return cells.length > 0 && cells.every((cell) => /^:?-{1,}:?$/.test(cell));
};

/** True for anything that could be a row of a table. */
const isTableRow = (line: string): boolean => line.trim().startsWith("|");

type Alignment = "left" | "right" | "center";

const alignments = (rule: string): ReadonlyArray<Alignment> =>
  tableCells(rule).map((cell) => {
    const left = cell.startsWith(":");
    const right = cell.endsWith(":");
    if (left && right) return "center";
    return right ? "right" : "left";
  });

/** Pad a styled cell to a column, by its visible width rather than its bytes. */
const padCell = (cell: string, room: number, how: Alignment): string => {
  const slack = Math.max(0, room - visibleWidth(cell));
  if (how === "right") return `${" ".repeat(slack)}${cell}`;
  if (how === "center") {
    const left = Math.floor(slack / 2);
    return `${" ".repeat(left)}${cell}${" ".repeat(slack - left)}`;
  }
  return `${cell}${" ".repeat(slack)}`;
};

/**
 * Render a Markdown table as aligned columns.
 *
 * It was rendered as its source: seven lines of pipes and a row of dashes,
 * which is the one shape of Markdown that is harder to read unrendered than
 * any prose. Cells keep their inline markup, so a code span in a cell is still
 * a code span.
 *
 * Columns are sized to their widest cell and then shrunk together if the whole
 * is too wide, because a table that overflows the terminal wraps into
 * something worse than the source it came from.
 */
function tableRows(lines: ReadonlyArray<string>, width: number): ReadonlyArray<string> {
  const [header, rule, ...body] = lines;
  if (header === undefined || rule === undefined) return lines.map((line) => line);

  const how = alignments(rule);
  const headings = tableCells(header);
  const cells = body.map((line) => tableCells(line));
  const columns = Math.max(headings.length, ...cells.map((row) => row.length));

  /**
   * Style a cell, and cut it to a column if it does not fit.
   *
   * Cut on the spans rather than on either the source or the styled string.
   * The source counts markup a reader never sees — a cell written `` `the` ``
   * is eight columns and ten characters — and the styled string counts escape
   * bytes. Both make a cell that fits look like one that does not.
   */
  const styled = (text: string, style: string, room = Number.POSITIVE_INFINITY) => {
    const spans = scan(text, style);
    const total = spans.reduce((sum, span) => sum + [...span.text].length, 0);
    const trim = total > room;
    let left = trim ? Math.max(1, room - 1) : room;
    const out: string[] = [];

    for (const span of spans) {
      if (left <= 0) break;
      const glyphs = [...span.text];
      const take = glyphs.slice(0, left).join("");
      left -= [...take].length;
      out.push(`${span.style}${take}${span.style === "" ? "" : RESET}`);
    }

    return `${out.join("")}${trim ? "…" : ""}`;
  };

  // Measured on the text a reader sees, not on the styled string: markup adds
  // escape bytes that occupy no columns, and sizing by them makes every column
  // that contains a code span too wide.
  const plain = (text: string) => visibleWidth(styled(text, ""));

  const widths: number[] = [];
  for (let column = 0; column < columns; column += 1) {
    const widest = cells.reduce(
      (most, row) => Math.max(most, plain(row[column] ?? "")),
      plain(headings[column] ?? ""),
    );
    widths.push(widest);
  }

  // Two spaces between columns. If that does not fit, the widest column gives
  // way first, and keeps giving way until it does.
  const gap = 2;
  let total = () => widths.reduce((sum, room) => sum + room, 0) + gap * (columns - 1);
  while (total() > width && widths.some((room) => room > 4)) {
    const widest = widths.indexOf(Math.max(...widths));
    widths[widest] = Math.max(4, (widths[widest] ?? 4) - 1);
  }

  const line = (values: ReadonlyArray<string>, style: string) =>
    Array.from({ length: columns }, (_unused, column) =>
      padCell(
        styled(values[column] ?? "", style, widths[column] ?? 0),
        widths[column] ?? 0,
        how[column] ?? "left",
      ),
    )
      .join(" ".repeat(gap))
      .trimEnd();

  const out = [line(headings, BOLD)];
  out.push(`${DIM}${widths.map((room) => "─".repeat(room)).join("─".repeat(gap))}${RESET}`);
  for (const row of cells) out.push(line(row, ""));
  return out;
}

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
