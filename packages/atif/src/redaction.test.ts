import { readFileSync } from "node:fs";

import { describe, expect, test } from "vite-plus/test";
import { Effect } from "effect";

import {
  ATIF_PINNED_SCHEMA_VERSION,
  AtifStep,
  AtifTrajectory,
  atifTraceTripwire,
  validateAtifTrajectory,
} from "./trace-schema.ts";
import {
  REDACTION_CATEGORY_CLASS,
  REDACTION_SERVICE_REF,
  TraceRedactor,
  atifRedactionRules,
  isCredentialCategory,
  type RedactionCategory,
  TraceRedactorLive,
  redactForExternalInference,
  redactString,
  redactStringForExternalInference,
  redactTraceString,
  redactTraceValue,
  redactValue,
  type RedactionResult,
} from "./redaction.ts";

const red = (s: string): RedactionResult<string> => redactString(s);

/**
 * The planted secrets are a SHARED fixture, not a list local to this file.
 *
 * `packages/openagents-cli` and `crates/openagents-cli` assert against the same
 * file, so a token family added here is asserted in all three redaction paths
 * rather than in whichever one the author happened to be editing. That is the
 * fix for the drift that let `oa_pat_` and `smct_` through.
 */
const SECRET_FIXTURES: ReadonlyArray<{
  label: string;
  raw: string;
  leak: string;
  category: string;
  credential: boolean;
}> = JSON.parse(
  readFileSync(
    new URL("../../../fixtures/redaction/planted-secrets.json", import.meta.url),
    "utf8",
  ),
).secrets;

describe("the shared planted-secret fixture", () => {
  const ruleCategories = new Set(atifRedactionRules.map((rule) => rule.category));
  const fixtureCategories = new Set(SECRET_FIXTURES.map((fx) => fx.category));

  test("classifies every category, so a new one cannot arrive unclassified", () => {
    // `REDACTION_CATEGORY_CLASS` is `Record<RedactionCategory, ...>`, so this is
    // already a compile error; the runtime check catches a category that was
    // added to the RULES list without being added to the union at all.
    for (const category of ruleCategories) {
      expect(
        REDACTION_CATEGORY_CLASS[category],
        `${category} has a rule but no entry in REDACTION_CATEGORY_CLASS`,
      ).toBeDefined();
    }
  });

  test("plants a secret for every category that has a rule", () => {
    const uncovered = [...ruleCategories].filter((c) => !fixtureCategories.has(c));
    expect(
      uncovered,
      `these categories have a rule and no planted secret in ` +
        `fixtures/redaction/planted-secrets.json, so nothing asserts that the ` +
        `openagents-cli redaction paths cover them`,
    ).toEqual([]);
  });

  test("marks credential entries the same way the rule list does", () => {
    for (const fx of SECRET_FIXTURES) {
      expect(fx.credential, `${fx.label} is filed under ${fx.category}`).toBe(
        isCredentialCategory(fx.category as RedactionCategory),
      );
    }
  });

  test("leaks name a fragment of the secret body, never only its prefix", () => {
    // A test that asserts a MARKER appeared passes for a redaction that swapped
    // `sk-liveSECRET` for `[REDACTED]liveSECRET`. Every leak here has to be
    // something whose survival means the secret survived.
    for (const fx of SECRET_FIXTURES) {
      expect(fx.raw, `${fx.label} does not contain its own leak`).toContain(fx.leak);
      expect(fx.leak.length, `${fx.label} has a trivially short leak`).toBeGreaterThan(7);
    }
  });
});

describe("redactString", () => {
  for (const fx of SECRET_FIXTURES) {
    test(`${fx.label} is scrubbed`, () => {
      const r = red(fx.raw);
      expect(r.value).not.toContain(fx.leak);
      expect(r.report.counts[fx.category] ?? 0).toBeGreaterThanOrEqual(1);
      expect(r.report.total).toBeGreaterThanOrEqual(1);
    });
  }

  test("slash-separated prose is not redacted as a long blob", () => {
    const prose =
      "states: candidate/shadow/released/active/rejected/rolled and schema/service/IPC/process/PTY/task/test/output/redaction";
    const r = red(prose);
    expect(r.value).toBe(prose);
    expect(r.report.counts.long_blob ?? 0).toBe(0);
  });

  test("a contiguous base64 blob is still redacted", () => {
    const blob = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVowMTIzNDU2Nzg5YWJjZGVm";
    const r = red(`token=${blob} end`);
    expect(r.value).not.toContain(blob);
    expect(r.report.counts.long_blob ?? 0).toBeGreaterThanOrEqual(1);
  });

  test("prose that only matches the mnemonic SHAPE is not redacted", () => {
    // 12 short lowercase words, so it matches the candidate regex, but the words
    // are not all BIP39 words -> it is ordinary prose and must be preserved.
    const prose = "the team will ship this year and then start over next month";
    const r = red(prose);
    expect(r.value).toBe(prose);
    expect(r.report.counts.mnemonic ?? 0).toBe(0);
  });

  test("a real BIP39 seed phrase inside prose is still redacted", () => {
    const r = red(
      "backup phrase legal winner thank year wave sausage worth useful legal winner thank yellow now",
    );
    expect(r.value).not.toContain("legal winner thank year wave sausage");
    expect(r.report.counts.mnemonic ?? 0).toBeGreaterThanOrEqual(1);
  });

  test("known public false positives are preserved", () => {
    const r = red(
      "See https://openagents.com/trace/abc-123 and https://github.com/OpenAgentsInc/openagents/issues/6219 on openagents/khala for #6219.",
    );
    expect(r.value).toContain("https://openagents.com/trace/abc-123");
    expect(r.value).toContain("https://github.com/OpenAgentsInc/openagents/issues/6219");
    expect(r.value).toContain("openagents/khala");
    expect(r.value).toContain("#6219");
    expect(r.report.total).toBe(0);
  });

  test("is deterministic", () => {
    const input = SECRET_FIXTURES.map((f) => f.raw).join(" | ");
    const a = redactTraceString(input);
    const b = redactTraceString(input);
    expect(a.value).toBe(b.value);
    expect(a.report).toEqual(b.report);
  });
});

describe("redactValue", () => {
  test("walks deeply, preserves numeric metrics, and redacts usernames", () => {
    const value = {
      metrics: { prompt_tokens: 12, completion_tokens: 34, cached_tokens: 7 },
      path: "wrote /Users/alice/work/x.ts",
      listing: "drwxr-xr-x@ 3 alice staff 96 file",
      slug: "/private/tmp/-Users-alice-work/log",
      token: "oa_agent_AbCdEf123456789xyz",
    };
    const r = redactValue(value);
    const json = JSON.stringify(r.value);

    expect(r.value.metrics).toEqual(value.metrics);
    expect(json).not.toContain("/Users/alice");
    expect(json).not.toContain(" alice ");
    expect(json).not.toContain("oa_agent_AbCdEf");
    expect(r.report.counts.home_path).toBeGreaterThanOrEqual(1);
    expect(r.report.counts.username).toBeGreaterThanOrEqual(1);
  });
});

describe("redact-before-tripwire safety bar", () => {
  const leakBlob = SECRET_FIXTURES.map((f) => f.raw).join("\n");

  const buildTrajectory = (userMessage: string): AtifTrajectory =>
    new AtifTrajectory({
      schema_version: ATIF_PINNED_SCHEMA_VERSION,
      trajectory_id: "redaction-test-1",
      session_id: "chatcmpl-redaction-test-1",
      visibility: "owner_only",
      agent: {
        name: "Khala",
        version: "gateway-1",
        model_name: "openagents/khala",
      },
      steps: [
        new AtifStep({ step_id: 1, source: "user", message: userMessage }),
        new AtifStep({
          step_id: 2,
          source: "agent",
          message: "Here is the result, redacted appropriately.",
          model_name: "openagents/khala",
          metrics: { prompt_tokens: 10, completion_tokens: 5 },
        }),
      ],
    });

  test("the leaky control trips the backstop", () => {
    expect(atifTraceTripwire(buildTrajectory(leakBlob)).length).toBeGreaterThan(0);
  });

  test("the scrubbed trajectory passes validation and tripwire", () => {
    const { value } = redactTraceValue(buildTrajectory(leakBlob));
    expect(validateAtifTrajectory(value as AtifTrajectory)).toEqual([]);
    expect(atifTraceTripwire(value as AtifTrajectory)).toEqual([]);
    expect(JSON.stringify(value)).toContain("openagents/khala");
  });
});

describe("redactForExternalInference shared service", () => {
  const adversarialRegulatedDocument = [
    "Legal intake for an opaque workspace ref.",
    "Client email jane.doe@example.com and phone (312) 555-0198.",
    "SSN: 123-45-6789 appears in legacy paperwork.",
    "Health packet says DOB: 04/23/1978 and MRN: HOSP-928374.",
    "Local export path /Users/alice/Clients/private-case.md.",
    "Payment material lnbc2500u1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq.",
  ].join("\n");

  test("regulated corpus ingestion is redacted before external inference", () => {
    const result = redactStringForExternalInference(adversarialRegulatedDocument, {
      regulatedVertical: "health",
      surface: "corpus_ingestion",
    });

    expect(result.policy).toEqual({
      appliedBeforeExternalInference: true,
      regulatedVertical: "health",
      serviceRef: REDACTION_SERVICE_REF,
      surface: "corpus_ingestion",
    });
    expect(result.safeForExternalInference).toBe(true);
    expect(result.value).not.toContain("jane.doe@example.com");
    expect(result.value).not.toContain("312) 555-0198");
    expect(result.value).not.toContain("123-45-6789");
    expect(result.value).not.toContain("04/23/1978");
    expect(result.value).not.toContain("HOSP-928374");
    expect(result.value).not.toContain("/Users/alice");
    expect(result.report.counts.email).toBeGreaterThanOrEqual(1);
    expect(result.report.counts.phone).toBeGreaterThanOrEqual(1);
    expect(result.report.counts.ssn).toBeGreaterThanOrEqual(1);
    expect(result.report.counts.date_of_birth).toBeGreaterThanOrEqual(1);
    expect(result.report.counts.medical_record_id).toBeGreaterThanOrEqual(1);
  });

  test("trace capture uses the same service before the tripwire", () => {
    const trajectory = new AtifTrajectory({
      schema_version: ATIF_PINNED_SCHEMA_VERSION,
      trajectory_id: "regulated-redaction-trace-1",
      session_id: "capture-redaction-test-1",
      visibility: "owner_only",
      agent: {
        name: "Khala",
        version: "gateway-1",
        model_name: "openagents/khala",
      },
      steps: [
        new AtifStep({
          step_id: 1,
          source: "user",
          message: adversarialRegulatedDocument,
        }),
        new AtifStep({
          step_id: 2,
          source: "agent",
          message: "Created a redacted plan for the opaque workspace ref.",
          model_name: "openagents/khala",
          metrics: { prompt_tokens: 32, completion_tokens: 12 },
        }),
      ],
    });

    expect(atifTraceTripwire(trajectory).length).toBeGreaterThan(0);

    const result = redactForExternalInference(trajectory, {
      regulatedVertical: "legal",
      surface: "trace_capture",
    });

    expect(result.policy.serviceRef).toBe(REDACTION_SERVICE_REF);
    expect(result.policy.surface).toBe("trace_capture");
    expect(validateAtifTrajectory(result.value as AtifTrajectory)).toEqual([]);
    expect(atifTraceTripwire(result.value as AtifTrajectory)).toEqual([]);
    expect(JSON.stringify(result.value)).not.toContain("jane.doe@example.com");
    expect(JSON.stringify(result.value)).not.toContain("HOSP-928374");
  });
});

describe("TraceRedactor Effect service", () => {
  test("redacts through the Default layer", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const redactor = yield* TraceRedactor;
        return yield* redactor.redact({
          message: "Bearer abcdef0123456789ABCDEF in /Users/carol/x",
        });
      }).pipe(Effect.provide(TraceRedactor.Default)),
    );

    expect(JSON.stringify(result.value)).not.toContain("abcdef0123456789ABCDEF");
    expect(JSON.stringify(result.value)).not.toContain("/Users/carol");
    expect(result.report.counts.bearer).toBe(1);
    expect(result.report.counts.home_path).toBe(1);
  });

  test("redacts through the legacy live layer alias", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const redactor = yield* TraceRedactor;
        return yield* redactor.redactText("email d@example.com");
      }).pipe(Effect.provide(TraceRedactorLive)),
    );

    expect(result.value).toContain("[REDACTED:email]");
  });

  test("redacts external-inference text through the shared service", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const redactor = yield* TraceRedactor;
        return yield* redactor.redactTextForExternalInference(
          "MRN: HOSP-928374 for jane.doe@example.com",
          { regulatedVertical: "health", surface: "corpus_ingestion" },
        );
      }).pipe(Effect.provide(TraceRedactorLive)),
    );

    expect(result.policy.serviceRef).toBe(REDACTION_SERVICE_REF);
    expect(result.value).not.toContain("HOSP-928374");
    expect(result.value).not.toContain("jane.doe@example.com");
  });
});
