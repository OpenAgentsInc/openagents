import { describe, expect, it } from "vitest";

import { CODER_BACKENDS } from "../src/coder-backends.js";
import { CoderSession, type ReplySource } from "../src/coder-session.js";
import {
  coderTierLabel,
  nextTier,
  TIER_MODELS,
  tierForModel,
  tierLabel,
} from "../src/coder-tiers.js";

// The invariant (INVARIANTS.md "Coder Model Naming"): a vendor model name
// never renders. Every display path resolves through these functions, so the
// suite that holds the invariant is the suite that pins their answers.
describe("coder tiers", () => {
  it("labels every tier as OpenAgents Coder, never a vendor name", () => {
    expect(tierLabel("auto")).toBe("Coder Auto");
    expect(tierLabel("flash")).toBe("Coder Flash");
    expect(tierLabel("pro")).toBe("Coder Pro");
    expect(tierLabel("local")).toBe("Coder Local");
  });

  it("maps the pinned tier models and the local lane", () => {
    expect(coderTierLabel(TIER_MODELS.flash)).toBe("Coder Flash");
    expect(coderTierLabel(TIER_MODELS.pro)).toBe("Coder Pro");
    expect(coderTierLabel("ollama:qwen3.8:27b-mtp-q8_0")).toBe("Coder Local");
    expect(coderTierLabel(undefined)).toBe("Coder Auto");
  });

  it("shows the bare product name for a model no tier pins", () => {
    const label = coderTierLabel("some-experimental-model");
    expect(label).toBe("Coder");
    expect(label).not.toContain("experimental");
  });

  it("cycles Auto to Flash to Pro to Local and around", () => {
    expect(nextTier("auto")).toBe("flash");
    expect(nextTier("flash")).toBe("pro");
    expect(nextTier("pro")).toBe("local");
    expect(nextTier("local")).toBe("auto");
  });

  it("keeps the tier map and the backend catalog in agreement", () => {
    for (const backend of CODER_BACKENDS) {
      const tier = tierForModel(backend.id);
      // A backend outside the tier map must carry the bare product label; one
      // inside it must carry its tier's label. Either way, no vendor name.
      expect(backend.label).toBe(tier === undefined ? "Coder" : tierLabel(tier));
      expect(backend.label.startsWith("Coder")).toBe(true);
    }
  });

  it("never leaks a vendor id through any label", () => {
    for (const id of [
      TIER_MODELS.flash,
      TIER_MODELS.pro,
      "ox-alpha",
      "ollama:llama3",
      "anything-else",
    ]) {
      const label = coderTierLabel(id);
      expect(label).not.toContain(id);
      expect(label.startsWith("Coder")).toBe(true);
    }
  });
});

// The acceptance from OpenAgentsInc/openagents#40, held at the session level:
// a fresh session is Coder Auto, and shift+tab's cycle walks Flash, Pro,
// Local. The interface renders snapshot().model verbatim, so pinning the
// snapshot pins the bottom bar.
const silent: ReplySource = {
  model: "Coder Auto",
  async *reply() {
    yield { type: "text", value: "ok" } as const;
  },
};

describe("session tier cycling", () => {
  it("starts as Coder Auto and flips through Flash, Pro, Local on cycleTier", () => {
    const session = new CoderSession(silent, "repo", "main", undefined, undefined, undefined, {
      initial: "auto",
      build: () => Promise.resolve(silent),
    });

    expect(session.snapshot().model).toBe("Coder Auto");
    expect(session.cycleTier().label).toBe("Coder Flash");
    expect(session.snapshot().model).toBe("Coder Flash");
    expect(session.cycleTier().label).toBe("Coder Pro");
    expect(session.cycleTier().label).toBe("Coder Local");
    expect(session.cycleTier().label).toBe("Coder Auto");
  });
});
