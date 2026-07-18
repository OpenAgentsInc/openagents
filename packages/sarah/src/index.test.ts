import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  ROOT_AUTHORITY_REVISION,
  SARAH_CAPABILITIES,
  SARAH_RUNTIME_AUTHORITY_PROFILE,
  SarahBusinessContextSchema,
  buildSarahSystemPrompt,
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

  it("builds a citation-bound system prompt from decoded context", () => {
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
    expect(prompt).toContain("[source.release.fixture]");
    expect(prompt).toContain("typed capability brokers");
    expect(prompt).toContain("self-amplification");
  });
});
