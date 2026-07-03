import { describe, expect, test } from "bun:test";

import { type BrainStep } from "./brain";
import { checkStepAllowed, isReadOnly } from "./target";
import {
  decodeTargetAdapterContract,
  targetFromAdapter,
  TARGET_ADAPTER_SCHEMA_VERSION,
  type TargetAdapterContract,
} from "./target-adapter";

const publicTodoAdapter = (): TargetAdapterContract => ({
  schemaVersion: TARGET_ADAPTER_SCHEMA_VERSION,
  id: "fixture-public-todo-prod",
  displayName: "Fixture Public Todo",
  target: {
    name: "public-todo-prod",
    baseUrl: "https://example.com",
    environment: "prod",
    owner: "external",
    capabilities: ["browser"],
  },
  auth: {
    kind: "none",
    freshIdentity: {
      required: true,
      strategy: "fresh anonymous browser context per run; no persisted cookies",
    },
  },
  restart: { kind: "none" },
  prodReadOnly: {
    policy: "read-only",
    allowedStepKinds: ["navigate", "wait-for", "screenshot", "assert"],
    blockedStepKinds: ["click", "type"],
  },
  scenarioSeeds: [
    {
      id: "home-renders",
      title: "Home page renders",
      startPath: "/",
      commitment: "The target home page renders public text.",
    },
  ],
  checklist: [
    "Auth does not reuse a human account.",
    "Every production run is read-only.",
    "Artifacts contain only public-safe refs.",
  ],
});

describe("Target adapter contract (#8069)", () => {
  test("decodes the minimal third-party adapter schema", () => {
    const decoded = decodeTargetAdapterContract(publicTodoAdapter());
    expect(decoded.schemaVersion).toBe("openagents.qa_runner.target_adapter.v1");
    expect(decoded.target.environment).toBe("prod");
    expect(decoded.auth.freshIdentity.required).toBe(true);
    expect(decoded.scenarioSeeds[0]!.id).toBe("home-renders");
  });

  test("external prod adapters are forced read-only even when restriction is omitted", () => {
    const target = targetFromAdapter(publicTodoAdapter());
    expect(isReadOnly(target)).toBe(true);
    expect(checkStepAllowed(target, "navigate").allowed).toBe(true);

    const decision = checkStepAllowed(target, "click");
    expect(decision.allowed).toBe(false);
    if (!decision.allowed) {
      expect(decision.reason).toContain("read-only");
      expect(decision.reason).toContain("click");
    }
  });

  test("blocked external prod adapters fail before a run can start", () => {
    const adapter: TargetAdapterContract = {
      ...publicTodoAdapter(),
      prodReadOnly: {
        ...publicTodoAdapter().prodReadOnly,
        policy: "blocked",
      },
    };
    expect(() => targetFromAdapter(adapter)).toThrow(/blocked by policy/);
  });

  test("worked fixture scenario exposes the allowed read-only step set", () => {
    const target = targetFromAdapter(publicTodoAdapter());
    const readOnlySteps: ReadonlyArray<BrainStep> = [
      { kind: "navigate", url: "/", label: "open public homepage" },
      { kind: "screenshot", label: "capture public homepage" },
      {
        kind: "assert",
        label: "public page has text",
        check: { kind: "text-contains", selector: "body", value: "Example" },
      },
    ];

    for (const step of readOnlySteps) {
      expect(checkStepAllowed(target, step.kind).allowed).toBe(true);
    }
    expect(checkStepAllowed(target, "type").allowed).toBe(false);
  });
});

