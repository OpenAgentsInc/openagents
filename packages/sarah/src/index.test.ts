import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  ROOT_AUTHORITY_REVISION,
  DEFAULT_SARAH_HARNESS_POLICY,
  SARAH_AUTHORITY_REVISION,
  SARAH_CAPABILITIES,
  SARAH_RUNTIME_AUTHORITY_PROFILE,
  SarahBusinessContextSchema,
  SarahHarnessPolicySchema,
  buildSarahSystemPrompt,
  sanitizeSarahConversationResponse,
} from "./index";

describe("Sarah owner-orchestrator contract", () => {
  it("binds the admitted root authority and keeps self-amplification reserved", () => {
    expect(ROOT_AUTHORITY_REVISION).toBe(6);
    expect(SARAH_AUTHORITY_REVISION).toBe(4);
    expect(SARAH_RUNTIME_AUTHORITY_PROFILE.authorityMayAmplify).toBe(false);
    expect(SARAH_RUNTIME_AUTHORITY_PROFILE.reservedActions).toContain("increase_own_authority");
    expect(
      SARAH_CAPABILITIES.find(
        (item) => item.capabilityRef === "capability.sarah.financial_custody",
      ),
    ).toMatchObject({ access: "none", mode: "reserved" });
    expect(
      SARAH_CAPABILITIES.find((item) => item.capabilityRef === "capability.sarah.managed_sandbox"),
    ).toMatchObject({ access: "act", mode: "brokered" });
    expect(
      SARAH_RUNTIME_AUTHORITY_PROFILE.grants.find(
        ({ grantRef }) => grantRef === "grant.sarah.managed_sandbox",
      ),
    ).toMatchObject({
      actions: expect.arrayContaining(["create_managed_sandbox", "delete_managed_sandbox"]),
      programs: ["program.managed_agent_sandboxes"],
    });
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
    const prompt = buildSarahSystemPrompt(context, {
      laneRef: "hosted_khala",
      modelRef: "gemma-4-31b-it",
      providerLabel: "Google AI Studio",
      runtimeLabel: "OpenAgents hosted runtime",
    });
    expect(prompt).not.toContain("[source.release.fixture]");
    expect(prompt).toContain("one or two sentences");
    expect(prompt).toContain("Never print raw source refs");
    expect(prompt).toContain("typed capability brokers");
    expect(prompt).toContain("self-amplification");
    expect(prompt).toContain("model gemma-4-31b-it");
    expect(prompt).toContain("provider Google AI Studio");
    expect(prompt).toContain("Never infer the current model");
  });

  it("removes raw provenance refs from provider prose", () => {
    expect(
      sanitizeSarahConversationResponse(
        "Hello [source.sarah.message.fixture]. I can help [source.github.issue.9003].",
      ),
    ).toBe("Hello. I can help.");
  });

  it("freezes a released six-dimension conversational harness into the prompt", () => {
    const context = S.decodeUnknownSync(SarahBusinessContextSchema)({
      schema: "openagents.sarah.business_context.v1",
      threadRef: "thread.sarah.fixture",
      generatedAt: "2026-07-18T00:00:00.000Z",
      sources: [],
    });
    const policy = S.decodeUnknownSync(SarahHarnessPolicySchema)({
      ...DEFAULT_SARAH_HARNESS_POLICY,
      conversationInstructions: [
        "Say what is happening while a delegated action is still running.",
      ],
      maxReplyWords: 80,
    });
    const prompt = buildSarahSystemPrompt(
      context,
      {
        laneRef: "hosted_khala",
        modelRef: "gemma-4-31b-it",
        providerLabel: "Google AI Studio",
        runtimeLabel: "OpenAgents hosted runtime",
      },
      policy,
    );
    expect(Object.keys(policy.dimensions)).toHaveLength(6);
    expect(prompt).toContain("Default to under 80 words");
    expect(prompt).toContain("Say what is happening while a delegated action is still running.");
    expect(prompt).toContain("frozen until the turn terminates");
  });
});
