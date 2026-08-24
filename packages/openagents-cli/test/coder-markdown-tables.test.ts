import { describe, expect, it } from "vitest";

import { renderMarkdown } from "../src/coder-markdown.js";

const ESCAPE = String.fromCharCode(27);

const plain = (rows: ReadonlyArray<string>): ReadonlyArray<string> =>
  rows.map((row) => row.split(new RegExp(`${ESCAPE}\\[[0-9;]*m`)).join(""));

describe("rendering a Markdown table", () => {
  const table = [
    "| Metric | Value |",
    "| --- | ---: |",
    "| Bytes | 43 |",
    "| Longest word | `jumps` |",
    "| Most frequent word | `the` (×2) |",
  ].join("\n");

  it("lays the cells out in columns instead of printing the source", () => {
    const rows = plain(renderMarkdown(table, 78));

    // It used to render as its own source: rows of pipes and a line of dashes,
    // which is the one shape of Markdown harder to read unrendered than prose.
    expect(rows[0]).toBe("Metric                 Value");
    expect(rows.some((row) => row.includes("|"))).toBe(false);
    expect(rows.some((row) => row.includes("---"))).toBe(false);
  });

  it("honours the alignment the rule asks for", () => {
    const rows = plain(renderMarkdown(table, 78));

    // `---:` is a right-aligned column, and the values line up on their right.
    expect(rows[2]).toBe("Bytes                     43");
    expect(rows[4]).toBe("Most frequent word  the (×2)");
  });

  it("sizes a column by what a reader sees, not by the markup", () => {
    const rows = plain(renderMarkdown(table, 78));

    // A cell written with backticks is eight columns and ten characters.
    // Sizing by the source cut it; sizing by the styled string over-widened it.
    expect(rows[4]).toContain("the (×2)");
    expect(rows[3]).toContain("jumps");
  });

  it("keeps a code span in a cell styled", () => {
    expect(renderMarkdown(table, 78)[3]).toContain(`${ESCAPE}[36m`);
  });

  it("shrinks the widest column first when the table will not fit", () => {
    const wide = [
      "| Lane | Repository identified | Observation |",
      "|---|---|---|",
      "| ox-alpha | OpenAgentsInc/openagents.com | Phoenix app with a forge |",
    ].join("\n");

    const rows = plain(renderMarkdown(wide, 28));

    // A table that overflows wraps into something worse than its source.
    for (const row of rows) expect(row.length).toBeLessThanOrEqual(28);
    expect(rows[0]).toContain("Lane");
  });

  it("fills a row that is short of cells rather than dropping the row", () => {
    const ragged = ["| A | B | C |", "| :-: | --- | ---: |", "| x |", "| 1 | 2 | 3 |"].join("\n");

    const rows = plain(renderMarkdown(ragged, 40));

    expect(rows).toContain("x");
    expect(rows).toContain("1  2  3");
  });

  it("leaves a line of pipes that is not a table alone", () => {
    // A table needs its rule; without one this is a sentence with a pipe in it.
    expect(plain(renderMarkdown("a | b not a table", 40))).toEqual(["a | b not a table"]);
  });

  it("leaves a table inside a code fence as source", () => {
    const fenced = ["```", "| a | b |", "| --- | --- |", "```"].join("\n");
    const rows = plain(renderMarkdown(fenced, 40));

    expect(rows.some((row) => row.includes("| a | b |"))).toBe(true);
  });
});
