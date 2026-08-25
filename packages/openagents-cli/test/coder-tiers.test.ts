import { describe, expect, it } from "vitest";

import { CODER_BACKENDS } from "../src/coder-backends.js";
import { CoderSession, type ReplySource } from "../src/coder-session.js";
import {
  coderTierLabel,
  nextTier,
  TIER_MODELS,
  tierForModel,
  tierLabel,
  tierUnavailable,
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

  it("opens a tier whose lane the server says is available", () => {
    const served = [
      { id: TIER_MODELS.flash, available: true },
      { id: TIER_MODELS.pro, available: true },
    ];
    expect(tierUnavailable(served, "auto")).toBeUndefined();
    expect(tierUnavailable(served, "flash")).toBeUndefined();
    expect(tierUnavailable(served, "pro")).toBeUndefined();
  });

  it("refuses a tier whose lane the server serves but cannot answer on", () => {
    const served = [
      { id: TIER_MODELS.flash, available: false },
      { id: TIER_MODELS.pro, available: true },
    ];
    // The dead lane is refused; the live one and Auto still open. This is the
    // case the tier rename could have buried: `Coder Flash` is a friendlier
    // name to fail under than the vendor id a reader could have searched for.
    // The refusal points at the tiers that can answer.
    expect(tierUnavailable(served, "flash")).toContain("Coder Pro");
    expect(tierUnavailable(served, "flash")).not.toContain("Coder Flash");
    expect(tierUnavailable(served, "pro")).toBeUndefined();
    expect(tierUnavailable(served, "auto")).toBeUndefined();
  });

  it("refuses a tier this deployment does not serve at all", () => {
    const served = [{ id: TIER_MODELS.pro, available: true }];
    expect(tierUnavailable(served, "flash")).toContain("not serving that lane");
  });

  it("says Auto cannot select when no lane on the deployment can answer", () => {
    const served = [
      { id: TIER_MODELS.flash, available: false },
      { id: TIER_MODELS.pro, available: false },
    ];
    for (const tier of ["auto", "flash", "pro"] as const) {
      expect(tierUnavailable(served, tier)).toContain("no lane on this deployment");
    }
  });

  it("allows every tier when the catalog could not be read", () => {
    // Not the same as "serves nothing": an unreachable or older server is a
    // reason to let the turn report the truth, not to refuse up front.
    for (const tier of ["auto", "flash", "pro", "local"] as const) {
      expect(tierUnavailable(undefined, tier)).toBeUndefined();
    }
  });

  it("never refuses Coder Local, which answers from this machine", () => {
    expect(tierUnavailable([], "local")).toBeUndefined();
    expect(tierUnavailable([{ id: TIER_MODELS.pro, available: false }], "local")).toBeUndefined();
  });

  it("names no vendor model and quotes no price when it refuses", () => {
    const served = [
      { id: TIER_MODELS.flash, available: false },
      { id: TIER_MODELS.pro, available: false },
    ];
    for (const tier of ["auto", "flash", "pro"] as const) {
      const refusal = tierUnavailable(served, tier);
      expect(refusal).toBeDefined();
      // The invariant does not lapse when the news is bad.
      for (const id of [TIER_MODELS.flash, TIER_MODELS.pro, "ox-alpha", "gemini", "gpt", "ollama"])
        expect(refusal).not.toContain(id);
      // Availability is the only claim. A tier carries no price comparison,
      // because the catalog declares a rate for some lanes and none for
      // others.
      for (const priced of ["cheap", "cheaper", "free", "$", "per million", "cost"])
        expect(refusal?.toLowerCase()).not.toContain(priced);
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

  // What `tierUnavailable` is for, end to end: the build throws for a lane the
  // deployment cannot answer, and the session stays on the tier that was
  // answering rather than stranding the reader on a dead one.
  it("stays on the answering tier when the next tier cannot be built", async () => {
    const refusal = tierUnavailable(
      [
        { id: TIER_MODELS.flash, available: false },
        { id: TIER_MODELS.pro, available: true },
      ],
      "flash",
    );
    expect(refusal).toBeDefined();

    const session = new CoderSession(silent, "repo", "main", undefined, undefined, undefined, {
      initial: "auto",
      build: (tier) =>
        tier === "flash" ? Promise.reject(new Error(refusal)) : Promise.resolve(silent),
    });

    session.cycleTier();
    // The label flips optimistically; the build decides whether it holds.
    expect(session.snapshot().model).toBe("Coder Flash");

    await session.submit("hello");

    // The refused tier did not take, and the reader was told which tier it
    // was and where to go instead — without a vendor name.
    expect(session.snapshot().model).toBe("Coder Auto");
    const notices = session
      .snapshot()
      .entries.filter((entry) => entry.role === "notice")
      .map((entry) => entry.text)
      .join("\n");
    expect(notices).toContain("Coder Flash is not available");
    expect(notices).toContain("Coder Pro");
    expect(notices).not.toContain(TIER_MODELS.flash);
  });
});
