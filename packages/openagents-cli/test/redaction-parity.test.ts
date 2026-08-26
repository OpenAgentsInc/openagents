/**
 * The guard that keeps the CLI redaction path from drifting away from ATIF.
 *
 * There used to be two hand-written rule lists -- one in
 * `packages/atif/src/redaction.ts`, one in `src/trace-store.ts` -- and they
 * drifted twice. `oa_pat_`, `oa_token`, `oa_agent_` and `oa-x-` were in ATIF
 * and missing from the CLI, so `openagents trace redact` printed "Nothing
 * matched the redaction rules" over a file full of live OpenAgents tokens.
 * `smct_` was missing from both. Neither gap produced an error; both produced a
 * redaction that reported success.
 *
 * There is one list now, and this file is what holds it to that. Each test
 * fails at a different link in the chain, so adding a token family walks the
 * author to the place that still needs it:
 *
 *   1. Add a rule to ATIF, forget to classify it   -> compile error.
 *   2. Classify it, forget the planted secret      -> the ATIF fixture test.
 *   3. Plant it, forget the CLI                    -> the tests below.
 *
 * Assertions here are ALWAYS that the secret BODY is absent. Asserting that a
 * marker appeared would pass for a redaction that swapped a prefix and left the
 * key in place, which is the original bug.
 */
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import {
  REDACTION_CATEGORY_CLASS,
  atifCredentialRules,
  atifRedactionRules,
  isCredentialCategory,
} from "../src/memory/redaction.js";
import { redactText, redactionRules } from "../src/trace-store.js";

interface PlantedSecret {
  readonly label: string;
  readonly category: string;
  readonly credential: boolean;
  readonly raw: string;
  readonly leak: string;
}

const planted: ReadonlyArray<PlantedSecret> = (
  JSON.parse(
    readFileSync(
      new URL("../../../fixtures/redaction/planted-secrets.json", import.meta.url),
      "utf8",
    ),
  ) as { secrets: ReadonlyArray<PlantedSecret> }
).secrets;

const credentials = planted.filter((entry) => entry.credential);

const home = "/Users/octavia";
const rules = redactionRules(home);

describe("the CLI redaction path against the shared planted secrets", () => {
  it("has at least one planted credential to check", () => {
    // A fixture that failed to load would make every loop below vacuous, and a
    // suite that asserts nothing is the same failure this file exists to stop.
    expect(credentials.length).toBeGreaterThanOrEqual(16);
  });

  it.each(credentials)("removes the body of the planted $label", ({ leak, raw }) => {
    const result = redactText(raw, rules);
    expect(result.text).not.toContain(leak);
    expect(result.total).toBeGreaterThanOrEqual(1);
  });

  it("removes every planted credential from one document at once", () => {
    // Rules run in order over one growing string, so a rule can consume the
    // text a later rule was going to match. Per-line checks miss that; this
    // plants the whole set in a single document and checks the same floor.
    const document = credentials.map((entry) => entry.raw).join("\n");
    const result = redactText(document, rules);
    const survivors = credentials
      .filter((entry) => result.text.includes(entry.leak))
      .map((entry) => entry.label);
    expect(survivors).toEqual([]);
  });

  it("never echoes a secret back in the report it prints", () => {
    // The report is what the command shows the operator. It carries counts.
    const document = credentials.map((entry) => entry.raw).join("\n");
    const { counts, total } = redactText(document, rules);
    const report = JSON.stringify({ counts, total });
    for (const entry of credentials) {
      expect(report, `${entry.label} appeared in the report`).not.toContain(entry.leak);
    }
  });
});

describe("the CLI rule list against the ATIF rule list", () => {
  const cliCategories = new Set(rules.map((rule) => rule.category));

  it("carries every ATIF credential rule, by pattern, not by retyping it", () => {
    // Identity, not equality: the CLI list holds the SAME RegExp objects the
    // ATIF list holds. A rule that was copied rather than imported fails here
    // even when the copy is currently byte-identical, because a copy is what
    // drifts on the next edit.
    const cliPatterns = new Set(rules.map((rule) => rule.pattern));
    const restated = atifCredentialRules
      .filter((rule) => !cliPatterns.has(rule.pattern))
      .map((rule) => rule.category);
    expect(restated).toEqual([]);
  });

  it("covers every ATIF credential category", () => {
    const uncovered = atifCredentialRules
      .map((rule) => rule.category)
      .filter((category) => !cliCategories.has(category));
    expect(
      uncovered,
      "these credential categories exist in packages/atif/src/redaction.ts and " +
        "have no coverage in the openagents trace redact path",
    ).toEqual([]);
  });

  it("adds only trace-specific rules of its own", () => {
    // The rules the CLI still owns are the ones ATIF does not have a category
    // for: a JSON field named like a secret, a broad `NAME=value` line, and the
    // home-path rewrite to `~` that a trace wants and an export does not. If
    // this list grows, the new rule probably belongs in ATIF instead, where all
    // three redaction paths would get it.
    const own = rules
      .filter((rule) => !atifRedactionRules.some((atif) => atif.pattern === rule.pattern))
      .map((rule) => rule.category);
    expect(own).toEqual(["secret_field", "env_value", "env_value", "home_path", "home_path"]);
  });

  it("keeps the ATIF categories the trace path deliberately declines", () => {
    // Not every ATIF category belongs in a trace redaction. `long_blob` would
    // eat a public `npub`, and `home_path` is handled here as a `~` rewrite
    // rather than a tag. Naming them keeps the omission a decision rather than
    // an oversight -- and the compile-time `Record<RedactionCategory, ...>` in
    // ATIF means a NEW category cannot land in this set by default.
    const declined = atifRedactionRules
      .map((rule) => rule.category)
      .filter((category) => !isCredentialCategory(category));
    // Compared as a set, so this asserts membership without depending on the
    // order the rules happen to be written in.
    expect(new Set(declined)).toEqual(
      new Set([
        "date_of_birth",
        "email",
        "file_url",
        "home_path",
        "ip",
        "long_blob",
        "medical_record_id",
        "owner_id",
        "phone",
        "ssn",
      ]),
    );
    for (const category of declined) {
      expect(REDACTION_CATEGORY_CLASS[category]).toBe("other");
    }
  });
});
