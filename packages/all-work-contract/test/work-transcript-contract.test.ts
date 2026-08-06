import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vite-plus/test";

import { decodeWorkComposerDraft, decodeWorkTranscriptPage } from "../src/generated.ts";

const fixture = (path: string): unknown =>
  JSON.parse(readFileSync(resolve(import.meta.dirname, "../fixtures", path), "utf8")) as unknown;

describe("portable work transcript projection", () => {
  it("decodes semantic rows and preserves five independent acknowledgements", () => {
    const page = decodeWorkTranscriptPage(fixture("valid/work-transcript-page.json"));
    const user = page.rows[0];
    expect(user?.kind).toBe("user");
    if (user?.kind !== "user") throw new Error("Expected the user fixture row.");
    expect(user.acknowledgements).toEqual({
      commandId: "cmd:fixture:1",
      admission: "admitted",
      effect: "completed",
      turn: "completed",
      quiescence: "quiesced",
      verification: "verified",
    });
    expect(page.rows.map((row) => row.kind)).toEqual(["user", "proposed_plan", "approval_request"]);
  });

  it("accepts empty durable drafts but rejects provider-specific row kinds", () => {
    expect(decodeWorkComposerDraft(fixture("valid/work-composer-draft.json")).text).toBe("");
    expect(() =>
      decodeWorkTranscriptPage(fixture("invalid/work-transcript-unknown-kind.json")),
    ).toThrow();
  });

  it("rejects transcript text and resident windows above their bounds", () => {
    const valid = decodeWorkTranscriptPage(fixture("valid/work-transcript-page.json"));
    expect(() =>
      decodeWorkTranscriptPage({
        ...valid,
        rows: [
          {
            ...valid.rows[0],
            text: "x".repeat(8_001),
          },
        ],
        returned: 1,
      }),
    ).toThrow();
    expect(() =>
      decodeWorkTranscriptPage({
        ...valid,
        rows: Array.from({ length: 202 }, () => valid.rows[0]),
        returned: 202,
      }),
    ).toThrow();
  });
});
