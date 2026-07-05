import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

import { describe, expect, test } from "vitest";

import {
  buildD1TableSweep,
  extractTableDeclarations,
  formatMarkdownReport,
  stripJavaScriptComments,
} from "./d1-zero-reference-sweep.mjs";

const fixtureRoot = () => {
  const root = mkdtempSync(join(tmpdir(), "d1-sweep-"));
  mkdirSync(join(root, "workers/api/migrations"), { recursive: true });
  mkdirSync(join(root, "workers/api/src/test"), { recursive: true });
  mkdirSync(join(root, "workers/api/src"), { recursive: true });

  return root;
};

const write = (root: string, path: string, text: string) => {
  const target = join(root, path);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, text);
};

describe("D1 zero-reference sweep", () => {
  test("extracts CREATE TABLE declarations with line evidence", () => {
    const declarations = extractTableDeclarations(
      [
        "-- migration",
        "CREATE TABLE IF NOT EXISTS live_table (id TEXT PRIMARY KEY);",
        "CREATE TABLE rebuilt_table AS SELECT * FROM live_table;",
      ].join("\n"),
      "workers/api/migrations/0001_fixture.sql",
    );

    expect(declarations).toEqual([
      {
        line: 2,
        sourcePath: "workers/api/migrations/0001_fixture.sql",
        table: "live_table",
      },
      {
        line: 3,
        sourcePath: "workers/api/migrations/0001_fixture.sql",
        table: "rebuilt_table",
      },
    ]);
  });

  test("strips JavaScript comments without stripping SQL strings", () => {
    const stripped = stripJavaScriptComments(
      [
        "const query = `SELECT * FROM live_table`;",
        "// comment_only_table",
        "/* block_only_table */",
      ].join("\n"),
    );

    expect(stripped).toContain("live_table");
    expect(stripped).not.toContain("comment_only_table");
    expect(stripped).not.toContain("block_only_table");
  });

  test("classifies production, test-only, migration-only, retained, and confirmed-zero tables", () => {
    const root = fixtureRoot();
    write(
      root,
      "workers/api/migrations/0001_fixture.sql",
      [
        "CREATE TABLE referenced_table (id TEXT PRIMARY KEY);",
        "CREATE TABLE test_only_table (id TEXT PRIMARY KEY);",
        "CREATE TABLE comment_only_table (id TEXT PRIMARY KEY);",
        "CREATE TABLE migration_only_table (id TEXT PRIMARY KEY);",
        "CREATE TABLE retained_table (id TEXT PRIMARY KEY);",
        "CREATE TABLE confirmed_table (id TEXT PRIMARY KEY);",
      ].join("\n"),
    );
    write(
      root,
      "workers/api/src/live.ts",
      [
        "export const sql = `SELECT * FROM referenced_table`;",
        "// comment_only_table should not count as live",
      ].join("\n"),
    );
    write(
      root,
      "workers/api/src/live.test.ts",
      [
        "test('fixture', () => {",
        "  const sql = `SELECT * FROM test_only_table`;",
        "  const confirmed = `SELECT * FROM confirmed_table`;",
        "});",
      ].join("\n"),
    );

    const sweep = buildD1TableSweep({
      confirmedZeroReferences: new Map([
        [
          "confirmed_table",
          {
            issueRef: "#fixture",
            reason: "Fixture-confirmed zero reference.",
          },
        ],
      ]),
      manualRetentions: new Map([
        [
          "retained_table",
          {
            issueRef: "#fixture",
            reason: "Fixture manual retention.",
          },
        ],
      ]),
      root,
      scanRoots: ["workers/api/src"],
    });
    const statuses = Object.fromEntries(
      sweep.tables.map((table) => [table.table, table.status]),
    );

    expect(sweep.summary).toMatchObject({
      createTableStatements: 6,
      uniqueTableNames: 6,
    });
    expect(statuses).toMatchObject({
      comment_only_table: "migration_only",
      confirmed_table: "confirmed_zero_reference",
      migration_only_table: "migration_only",
      referenced_table: "referenced",
      retained_table: "manually_retained",
      test_only_table: "test_only",
    });
  });

  test("formats a deterministic markdown report with all classified tables", () => {
    const root = fixtureRoot();
    write(
      root,
      "workers/api/migrations/0001_fixture.sql",
      [
        "CREATE TABLE alpha_table (id TEXT PRIMARY KEY);",
        "CREATE TABLE beta_table (id TEXT PRIMARY KEY);",
      ].join("\n"),
    );

    const markdown = formatMarkdownReport(
      buildD1TableSweep({
        root,
        scanRoots: ["workers/api/src"],
      }),
    );

    expect(markdown).toContain("CREATE TABLE statements scanned: 2");
    expect(markdown).toContain("| alpha_table | migration_only |");
    expect(markdown).toContain("| beta_table | migration_only |");
    expect(markdown).not.toContain("Generated at");
  });
});
