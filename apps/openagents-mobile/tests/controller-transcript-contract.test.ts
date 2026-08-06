import { describe, expect, it } from "vite-plus/test";

import { decodeWorkComposerDraft, decodeWorkTranscriptPage } from "../src/controller/contracts.ts";

describe("mobile semantic transcript contract", () => {
  it("decodes portable rows without a provider payload escape hatch", () => {
    const page = decodeWorkTranscriptPage({
      schemaVersion: "openagents.work_transcript.v1",
      aggregateType: "thread",
      aggregateId: "thread:mobile:test",
      generation: 2,
      status: "working",
      limit: 200,
      returned: 1,
      hasOlder: false,
      oldestRecordedAtMs: 100,
      newestRecordedAtMs: 100,
      rows: [
        {
          kind: "assistant",
          schemaVersion: "openagents.work_transcript.v1",
          rowId: "row:assistant:1",
          state: "active",
          recordedAtMs: 100,
          updatedAtMs: 100,
          acknowledgements: null,
          text: "Portable semantic output",
        },
      ],
    });
    expect(page.rows[0]?.kind).toBe("assistant");
    expect(() =>
      decodeWorkTranscriptPage({
        ...page,
        rows: [{ ...page.rows[0], payload: { rawProviderMessage: true } }],
      }),
    ).toThrow();
  });

  it("round-trips a bounded typed composer draft", () => {
    const draft = decodeWorkComposerDraft({
      schemaVersion: "openagents.composer_draft.v1",
      aggregateType: "thread",
      aggregateId: "thread:mobile:test",
      text: "Use the selected review",
      context: [
        {
          kind: "review",
          sourceRef: "review:pr:41",
          label: "PR 41",
        },
      ],
      updatedAtMs: 100,
    });
    expect(draft.context[0]).toMatchObject({ kind: "review", label: "PR 41" });
  });
});
