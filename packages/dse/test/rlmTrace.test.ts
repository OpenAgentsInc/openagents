import { expect, test } from "bun:test";

import { decodeRlmTraceDocV1CompatibleSync } from "../src/traceMining/rlmTrace.js";

test("rlmTrace decoder keeps canonical v1 events typed", () => {
  const doc = decodeRlmTraceDocV1CompatibleSync({
    format: "openagents.dse.rlm_trace",
    formatVersion: 1,
    signatureId: "@openagents/test/Sig.v1",
    events: [
      { _tag: "Input", input: { question: "q?" } },
      {
        iteration: 1,
        promptHash: "sha256:p",
        action: {
          _tag: "Search",
          target: { _tag: "Var", name: "Input.blobs" },
          query: "q"
        }
      },
      {
        iteration: 1,
        observation: {
          _tag: "SearchResult",
          trust: "untrusted",
          origin: "tool",
          target: { _tag: "Var", name: "Input.blobs" },
          totalChars: 10,
          query: "q",
          totalMatches: 1,
          truncated: false,
          matches: [
            {
              index: 0,
              snippet: "quote",
              span: {
                _tag: "SpanRef",
                source: { _tag: "VarJson", name: "Input.blobs" },
                startChar: 0,
                endChar: 5,
                totalChars: 10
              }
            }
          ]
        }
      }
    ]
  });

  expect(doc.events.length).toBe(3);
  const first = doc.events[0];
  const second = doc.events[1];
  const third = doc.events[2];

  expect(first && "_tag" in first ? first._tag : "n/a").toBe("Input");
  expect(second && "action" in second ? second.action._tag : "n/a").toBe("Search");
  expect(third && "observation" in third ? third.observation._tag : "n/a").toBe(
    "SearchResult"
  );
});

test("rlmTrace decoder normalizes legacy v1 final event shape", () => {
  const doc = decodeRlmTraceDocV1CompatibleSync({
    format: "openagents.dse.rlm_trace",
    formatVersion: 1,
    events: [
      { _tag: "Input", input: { question: "legacy" } },
      { _tag: "LegacyNoise", payload: 123 },
      { _tag: "Final", output: { answer: "a" } }
    ]
  });

  expect(doc.events.length).toBe(2);
  const final = doc.events[1];
  expect(final && "_tag" in final ? final._tag : "n/a").toBe("Final");
  if (final && "_tag" in final && final._tag === "Final") {
    expect(final.output).toEqual({ answer: "a" });
  }
});

test("rlmTrace decoder rejects unsupported format versions explicitly", () => {
  expect(() =>
    decodeRlmTraceDocV1CompatibleSync({
      format: "openagents.dse.rlm_trace",
      formatVersion: 2,
      events: []
    })
  ).toThrow("Unsupported rlm trace formatVersion=2 (expected 1)");
});
