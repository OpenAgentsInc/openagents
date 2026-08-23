import { describe, expect, it } from "vitest";

import { renderMarkdown, visibleWidth, wrapStyled } from "../src/coder-markdown.js";

const SGR = new RegExp("\\u001b\\[[0-9;]*m", "g");
const plain = (rows: ReadonlyArray<string>) => rows.map((row) => row.replace(SGR, ""));

const BOLD = "\x1b[1m";
const ITALIC = "\x1b[3m";
const DIM = "\x1b[2m";
const CODE = "\x1b[36m";

describe("renderMarkdown", () => {
  it("renders bold as ANSI rather than as asterisks", () => {
    const rows = renderMarkdown("hello **ox-alpha** there", 60);
    expect(rows[0]).toContain(`${BOLD}ox-alpha\x1b[0m`);
    expect(plain(rows)[0]).toBe("hello ox-alpha there");
  });

  it("renders italic and inline code", () => {
    const rows = renderMarkdown("a *slanted* word and `some_code()`", 60);
    expect(rows[0]).toContain(`${ITALIC}slanted\x1b[0m`);
    expect(rows[0]).toContain(`${CODE}some_code()\x1b[0m`);
  });

  it("renders a fenced block as styled rows without the fence markers", () => {
    const rows = renderMarkdown(
      ["before", "```elixir", 'def hello, do: "world"', "```", "after"].join("\n"),
      60,
    );
    expect(plain(rows)).toEqual(["before", '│ def hello, do: "world"', "after"]);
    expect(rows[1]).toContain(CODE);
  });

  it("keeps a fenced block styled while its closing fence has not arrived", () => {
    const rows = renderMarkdown(["```", "line one", "line two"].join("\n"), 60);
    expect(plain(rows)).toEqual(["│ line one", "│ line two"]);
  });

  it("renders headings, bullets, numbers, and quotes", () => {
    const rows = renderMarkdown(
      ["# Title", "- first", "- second", "1. one", "> quoted"].join("\n"),
      60,
    );
    expect(plain(rows)).toEqual(["Title", "• first", "• second", "1. one", "│ quoted"]);
    expect(rows[0]).toContain(BOLD);
  });

  it("wraps a list item under its text rather than under its bullet", () => {
    const rows = plain(renderMarkdown("- alpha beta gamma delta epsilon zeta", 16));
    expect(rows[0]).toBe("• alpha beta");
    for (const row of rows.slice(1)) expect(row.startsWith("  ")).toBe(true);
    expect(rows.join(" ").replace(/\s+/g, " ")).toContain("alpha beta gamma delta epsilon zeta");
  });

  it("never renders a row wider than the width it was given", () => {
    const source = "a-very-long-unbroken-token-that-cannot-be-split-on-spaces and some words";
    for (const row of renderMarkdown(source, 20)) expect(visibleWidth(row)).toBeLessThanOrEqual(20);
  });

  it("renders unterminated markup as the characters that arrived", () => {
    expect(plain(renderMarkdown("half a **bold run", 60))[0]).toBe("half a **bold run");
    expect(plain(renderMarkdown("half a `code run", 60))[0]).toBe("half a `code run");
  });

  it("loses no characters at any point in a streamed reply", () => {
    const source = [
      "## Connected **repositories**",
      "",
      "- 📓 `openagents.com` — *the forge*",
      "",
      "```",
      "git push openagents HEAD:main",
      "```",
    ].join("\n");

    for (let length = 1; length <= source.length; length += 1) {
      const arrived = source.slice(0, length);
      const rendered = plain(renderMarkdown(arrived, 60)).join("\n");
      // Every non-markup character that arrived is still on screen.
      const wanted = arrived.replace(/[`*#>\-\s]/gu, "");
      const shown = rendered.replace(/[`*#>\-\s│•]/gu, "");
      expect(shown).toContain(wanted);
    }
  });

  it("drops only the fence info string, which names a language rather than saying anything", () => {
    expect(plain(renderMarkdown("```elixir\nx\n```", 60))).toEqual(["│ x"]);
  });

  it("keeps a mid-token split from mangling a run that later completes", () => {
    // `**ox` then `-alpha**` is how a bold name arrives from the server.
    expect(plain(renderMarkdown("**ox", 60))[0]).toBe("**ox");
    expect(plain(renderMarkdown("**ox-alpha**", 60))[0]).toBe("ox-alpha");
    expect(renderMarkdown("**ox-alpha**", 60)[0]).toContain(BOLD);
  });
});

describe("wrapStyled", () => {
  it("applies one style to the whole block and preserves blank lines", () => {
    const rows = wrapStyled("one\n\ntwo", 40, DIM);
    expect(plain(rows)).toEqual(["one", "", "two"]);
    expect(rows[0]).toBe(`${DIM}one\x1b[0m`);
  });
});
