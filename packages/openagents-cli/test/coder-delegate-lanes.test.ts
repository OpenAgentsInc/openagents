import { describe, expect, it } from "vitest";

import {
  CHILD_LANE_ALIASES,
  CHILD_MODELS,
  childLaneName,
  resolveChildLane,
} from "../src/coder-delegate.js";
import { delegateTool } from "../src/coder-tools.js";
import { CoderTaskRegistry } from "../src/coder-tasks.js";
import type { CoderDelegation } from "../src/coder-session.js";

describe("naming a lane", () => {
  it("resolves the name a person uses to the slug the harness knows", () => {
    // Ox Alpha's slug says neither `ox` nor `alpha`, so a session offered only
    // the slug is one nobody can ask for Ox Alpha by name.
    expect(resolveChildLane("ox-alpha")).toBe("opencode/x-preview-f-free");
    expect(CHILD_LANE_ALIASES["ox-alpha"]).toBe("opencode/x-preview-f-free");
  });

  it("resolves a slug to itself, so nothing that worked stops", () => {
    expect(resolveChildLane("opencode/x-preview-f-free")).toBe("opencode/x-preview-f-free");
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
    expect(childLaneName("opencode/x-preview-f-free")).toBe("ox-alpha");
    expect(childLaneName("opencode/gemini-3.7-flash")).toBe("gemini");
  });

  it("offers the names first, since those are what a call would say", () => {
    expect(CHILD_MODELS.slice(0, 2)).toEqual(["ox-alpha", "gemini"]);
    expect(CHILD_MODELS).toContain("devin");
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
