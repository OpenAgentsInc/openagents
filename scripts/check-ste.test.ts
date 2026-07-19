import { describe, expect, test } from "vite-plus/test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  applyScreeningReview,
  agentCompactLineNumbers,
  countDiagnostics,
  deriveProfile,
  dictionaryWords,
  extractProse,
  inspectStructure,
  validateGlossary,
  validateAgentCompactTerms,
  type CheckerConfig,
} from "./ste-core";
import { rewriteSteSemicolons } from "./rewrite-ste-structure";
import { rewriteSteHardForms } from "./rewrite-ste-hard-forms";

const config: CheckerConfig = {
  policyRevision: "test",
  agentCompactRevision: "openagents-agent-compact-v1",
  steIssue: 9,
  glossaryRevision: "test-v1",
  governedExtensions: [".md"],
  sourceDataPrefixes: [
    "docs/transcripts/",
    "docs/reference/",
    "apps/openagents.com/apps/start/public/docs/",
  ],
  proceduralPathSignals: ["runbook"],
  controlPaths: ["AGENTS.md"],
};

describe("STE prose extraction", () => {
  test("removes code, links, URLs, and code fences", () => {
    const prose = extractProse(
      "Use `pnpm test`.\n\n```sh\npnpm test; exit 1\n```\nRead [the guide](https://example.com).",
    );
    expect(prose.map((line) => line.text)).toEqual(["Use  .", "Read the guide."]);
  });

  test("limits a compact extension to the agent section of a dual document", () => {
    const lines = agentCompactLineNumbers(
      "## Human changelog\nUse the app.\n\n## Agent changelog\n- lane: release\n\n## Release provenance\nOwner request.",
      "dual",
    );
    expect([...lines]).toEqual([4, 5, 6]);
  });
});

describe("STE structural checks", () => {
  test("reports the rule, location, and action", () => {
    const diagnostics = inspectStructure(
      "docs/runbook.md",
      "The file is generated; it isn't authorised.",
      "procedural",
    );
    expect(diagnostics.map((item) => item.rule)).toEqual(
      expect.arrayContaining(["STE-8.1", "STE-9.1", "STE-1.4", "STE-3.6"]),
    );
    expect(
      diagnostics.every((item) => item.line === 1 && item.column > 0 && item.action.length > 0),
    ).toBe(true);
  });

  test("uses the procedure word limit", () => {
    const text = `${Array.from({ length: 21 }, (_, index) => `word${index}`).join(" ")}.`;
    expect(countDiagnostics(inspectStructure("runbook.md", text, "procedural"))["STE-5.1"]).toBe(1);
  });

  test("does not inspect source data", () => {
    expect(
      inspectStructure("docs/transcripts/a.md", "It isn't changed; it was written.", "source-data"),
    ).toEqual([]);
  });
});

describe("STE profiles and glossary", () => {
  test("classifies controls, procedures, and source data", () => {
    expect(deriveProfile("AGENTS.md", config).risk).toBe("control");
    expect(deriveProfile("docs/release-runbook.md", config).ste_mode).toBe("mixed");
    expect(deriveProfile("docs/transcripts/a.md", config).ste_status).toBe("source-data");
    expect(
      deriveProfile("apps/openagents.com/apps/start/public/docs/index.md", config),
    ).toMatchObject({
      source: "Generated from apps/openagents.com/apps/start/content/docs",
      ste_mode: "source-data",
      ste_status: "source-data",
    });
    expect(deriveProfile("docs/changelog/2026-07-19-desktop-0.1.0-rc.25.md", config)).toMatchObject(
      {
        ste_audience: "dual",
        ste_agent_compact_revision: "openagents-agent-compact-v1",
      },
    );
  });

  test("rejects duplicate forms and long technical nouns", () => {
    const errors = validateGlossary({
      revision: "v1",
      steIssue: 9,
      terms: [
        { id: "OA-STE-0001", term: "one two three four", permittedForms: ["term"] },
        { id: "OA-STE-0002", term: "other", permittedForms: ["TERM"] },
      ],
    });
    expect(errors).toEqual(
      expect.arrayContaining([
        expect.stringContaining("more than three words"),
        expect.stringContaining("duplicate permitted form"),
      ]),
    );
  });

  test("loads permitted forms from an authorized Issue 9 dictionary", () => {
    const directory = mkdtempSync(join(tmpdir(), "openagents-ste-"));
    const path = join(directory, "dictionary.json");
    writeFileSync(
      path,
      JSON.stringify({ steIssue: 9, entries: [{ permittedForms: ["use", "used"] }] }),
    );
    expect(dictionaryWords(path)).toEqual(new Set(["use", "used"]));
  });

  test("rejects duplicate agent compact forms", () => {
    expect(
      validateAgentCompactTerms({
        revision: "test",
        baseGlossaryRevision: "test",
        terms: [
          { term: "lane", permittedForms: ["lane"], meaning: "A work unit." },
          { term: "other", permittedForms: ["LANE"], meaning: "Another work unit." },
        ],
      }),
    ).toContain("duplicate agent compact form: LANE");
  });
});

describe("STE screening review", () => {
  test("accepts only a selected screening rule after an identified review", () => {
    const profile = {
      ...deriveProfile("docs/test.md", config),
      ste_reviewer: "test-reviewer",
      ste_reviewed_at: "2026-07-19T00:00:00Z",
      ste_accepted_screening_rules: ["STE-2.4" as const],
    };
    const diagnostics = inspectStructure(
      "docs/test.md",
      "Routing is active; the route is controlled.",
      "descriptive",
    );
    expect(applyScreeningReview(diagnostics, profile).map((item) => item.rule)).toEqual([
      "STE-8.1",
    ]);
  });

  test("accepts density only for an identified agent compact review", () => {
    const profile = {
      ...deriveProfile("AGENTS.md", config),
      ste_reviewer: "test-reviewer",
      ste_reviewed_at: "2026-07-19T00:00:00Z",
      ste_audience: "agent" as const,
      ste_agent_compact_revision: "openagents-agent-compact-v1" as const,
      ste_accepted_screening_rules: ["STE-5.1" as const, "STE-8.2" as const],
    };
    const sentence = "Use " + Array.from({ length: 30 }, () => "one").join(" ") + ".";
    const paragraph = Array.from({ length: 7 }, () => "Use one.").join(" ");
    expect(
      applyScreeningReview(
        inspectStructure("AGENTS.md", `${sentence}\n${paragraph}`, "descriptive"),
        profile,
      ),
    ).toEqual([]);
  });
});

describe("STE structural rewrite", () => {
  test("replaces prose semicolons and preserves source data and words", () => {
    const input = [
      "---",
      'summary: "Keep a;b in metadata."',
      "---",
      "Use the route; then inspect it.",
      "Keep `printf 'a;b'` and https://example.com/a;b unchanged.",
      "Keep `multi;line",
      "code;span` unchanged.",
      "```sh",
      "printf 'c;d'",
      "```",
    ].join("\n");
    expect(rewriteSteSemicolons(input)).toBe(
      [
        "---",
        'summary: "Keep a;b in metadata."',
        "---",
        "Use the route. Then inspect it.",
        "Keep `printf 'a;b'` and https://example.com/a;b unchanged.",
        "Keep `multi;line",
        "code;span` unchanged.",
        "```sh",
        "printf 'c;d'",
        "```",
      ].join("\n"),
    );
  });

  test("replaces hard prose forms and keeps inline code", () => {
    expect(
      rewriteSteHardForms("It isn't authorised; use `it isn't authorised;`.\n", "descriptive"),
    ).toBe("It is not authorized. Use `it isn't authorised;`.\n");
  });
});
