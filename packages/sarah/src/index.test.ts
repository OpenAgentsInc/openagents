import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  ROOT_AUTHORITY_REVISION,
  SARAH_CAPABILITIES,
  SARAH_RUNTIME_AUTHORITY_PROFILE,
  SarahBusinessContextSchema,
  buildSarahSystemPrompt,
  sanitizeSarahConversationResponse,
} from "./index";

describe("Sarah owner-orchestrator contract", () => {
  it("binds the admitted root authority and keeps self-amplification reserved", () => {
    expect(ROOT_AUTHORITY_REVISION).toBe(3);
    expect(SARAH_RUNTIME_AUTHORITY_PROFILE.authorityMayAmplify).toBe(false);
    expect(SARAH_RUNTIME_AUTHORITY_PROFILE.reservedActions).toContain("increase_own_authority");
    expect(
      SARAH_CAPABILITIES.find(
        (item) => item.capabilityRef === "capability.sarah.financial_custody",
      ),
    ).toMatchObject({ access: "none", mode: "reserved" });
  });

  it("builds a provenance-bound but conversational system prompt", () => {
    const context = S.decodeUnknownSync(SarahBusinessContextSchema)({
      schema: "openagents.sarah.business_context.v1",
      threadRef: "thread.sarah.fixture",
      generatedAt: "2026-07-18T00:00:00.000Z",
      sources: [
        {
          sourceRef: "source.release.fixture",
          kind: "github_release",
          observedAt: "2026-07-18T00:00:00.000Z",
          freshness: "live",
          sensitivity: "public",
          summary: "RC21 is the newest release candidate.",
        },
      ],
    });
    const prompt = buildSarahSystemPrompt(context);
    expect(prompt).not.toContain("[source.release.fixture]");
    expect(prompt).toContain("one or two sentences");
    expect(prompt).toContain("Never print raw source refs");
    expect(prompt).toContain("typed capability brokers");
    expect(prompt).toContain("self-amplification");
  });

  it("removes raw provenance refs from provider prose", () => {
    expect(sanitizeSarahConversationResponse(
      "Hello [source.sarah.message.fixture]. I can help [source.github.issue.9003].",
    )).toBe("Hello. I can help.");
  });
});
