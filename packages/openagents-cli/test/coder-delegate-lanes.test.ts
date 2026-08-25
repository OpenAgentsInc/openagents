import { describe, expect, it } from "vitest";

import {
  CHILD_MODELS,
  childLaneName,
  resolveChildLane,
  SELF_CHILD_LANE,
  selfChildLane,
} from "../src/coder-delegate.js";
import { delegateTool } from "../src/coder-tools.js";
import { CoderTaskRegistry } from "../src/coder-tasks.js";
import type { CoderDelegation } from "../src/coder-session.js";

describe("naming a lane", () => {
  it("resolves Ox Alpha to the lane this process runs itself", () => {
    // It is the same model whichever lane serves it — the child's grant is
    // routed to OpenRouter's `stealth/ox-alpha` — so asking for Ox Alpha means
    // the lane that costs no second agent and no second credential.
    expect(resolveChildLane("ox-alpha")).toBe(SELF_CHILD_LANE);
    expect(resolveChildLane("openagents")).toBe(SELF_CHILD_LANE);
    expect(selfChildLane(resolveChildLane("ox-alpha") ?? "")).toBe(true);
  });

  it("still resolves opencode's own slug, so nothing that worked stops", () => {
    expect(resolveChildLane("opencode/x-preview-f-free")).toBe("opencode/x-preview-f-free");
    expect(selfChildLane("opencode/x-preview-f-free")).toBe(false);
  });

  it("carries a Devin permission mode through", () => {
    expect(resolveChildLane("devin")).toBe("devin");
    expect(resolveChildLane("devin:auto")).toBe("devin:auto");
  });

  it("refuses a name no lane answers to", () => {
    expect(resolveChildLane("gpt-9")).toBeUndefined();
    expect(resolveChildLane("  ")).toBeUndefined();
  });

  it("reports a lane by the name a reader would recognise", () => {
    // Reached by slug or by alias, it reports the same name, so a caller can
    // tell whether the lane they asked for is the lane that answered.
    expect(childLaneName(SELF_CHILD_LANE)).toBe("ox-alpha");
    expect(childLaneName("opencode/gemini-3.7-flash")).toBe("gemini");
  });

  it("offers the names first, since those are what a call would say", () => {
    expect(CHILD_MODELS.slice(0, 2)).toEqual(["ox-alpha", "openagents"]);
    expect(CHILD_MODELS).toContain("gemini");
    expect(CHILD_MODELS).toContain("devin");
    // One lane, two names for it, offered once each and not duplicated further.
    expect(CHILD_MODELS.filter((name) => name === "ox-alpha")).toHaveLength(1);
  });
});

describe("choosing a lane in the tool call", () => {
  const delegation = (options: { lanes?: boolean } = {}): CoderDelegation => {
    const registry = new CoderTaskRegistry();
    const fleet = { submit: () => Promise.resolve({ taskId: "t", status: "completed" } as never) };
    const base = { registry, fleet, label: "opencode (ox-alpha)" };
    return options.lanes === false
      ? base
      : {
          ...base,
          models: CHILD_MODELS,
          fleetFor: (name: string) =>
            resolveChildLane(name) === undefined
              ? undefined
              : { fleet, label: `opencode (${name})` },
        };
  };

  it("offers the lanes as an enum, so a call names one that exists", () => {
    const { parameters, description } = delegateTool(delegation());
    const model = (parameters["properties"] as Record<string, { enum?: string[] }>)["model"];

    expect(model?.enum).toContain("ox-alpha");
    expect(model?.enum).toContain("devin");
    expect(description).toContain("ox-alpha");
  });

  it("offers no choice when the session runs children one way", () => {
    // A knob that goes nowhere is worse than no knob: one session answered
    // "no, I cannot delegate to ox alpha" while holding exactly that lane.
    const { parameters } = delegateTool(delegation({ lanes: false }));

    expect(parameters["properties"]).not.toHaveProperty("model");
  });

  it("says which lanes exist when a call names one that does not", async () => {
    const output = await delegateTool(delegation()).run(
      { prompt: "go", model: "gpt-9" },
      new AbortController().signal,
    );

    expect(output).toContain("no `gpt-9` lane");
    expect(output).toContain("ox-alpha");
  });
});
