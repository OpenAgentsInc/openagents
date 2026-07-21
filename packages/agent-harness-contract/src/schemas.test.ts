import { Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { HarnessBootstrap } from "./bootstrap.ts";
import { HarnessCapabilityUnsupported } from "./capability.ts";
import { HarnessHostToolSpec } from "./host-tool.ts";
import { HarnessContinuationState } from "./lifecycle-state.ts";
import { HarnessPermissionModeSchema } from "./permission.ts";
import { decodeHarnessSkill, HarnessSkill } from "./skill.ts";

describe("skill schema", () => {
  test("decodes a minimal skill", () => {
    const skill = decodeHarnessSkill({
      name: "fast-follow",
      description: "study upstream projects",
      content: "# Fast Follow",
    });
    expect(skill.name).toBe("fast-follow");
    expect(skill.files).toBeUndefined();
  });

  test("rejects an empty name", () => {
    expect(() => decodeHarnessSkill({ name: "", description: "x", content: "y" })).toThrow();
  });

  test("carries attached files", () => {
    const decoded = S.decodeUnknownSync(HarnessSkill)({
      name: "s",
      description: "d",
      content: "c",
      files: [{ path: "a.md", content: "hi" }],
    });
    expect(decoded.files?.[0]?.path).toBe("a.md");
  });
});

describe("permission mode schema", () => {
  test("accepts the three modes", () => {
    for (const mode of ["allow-all", "default", "reject-all"] as const) {
      expect(S.decodeUnknownSync(HarnessPermissionModeSchema)(mode)).toBe(mode);
    }
  });

  test("rejects an unknown mode", () => {
    expect(() => S.decodeUnknownSync(HarnessPermissionModeSchema)("yolo")).toThrow();
  });
});

describe("host-tool spec", () => {
  test("carries an opaque JSON input schema", () => {
    const spec = S.decodeUnknownSync(HarnessHostToolSpec)({
      name: "get_user",
      description: "look up a user",
      inputJsonSchema: { type: "object", properties: { id: { type: "string" } } },
    });
    expect(spec.name).toBe("get_user");
  });
});

describe("capability error", () => {
  test("is a tagged, matchable error", () => {
    const err = new HarnessCapabilityUnsupported({
      harnessId: "codex",
      capability: "suspend_turn",
    });
    expect(err._tag).toBe("AgentHarness.CapabilityUnsupported");
    expect(err.capability).toBe("suspend_turn");
  });
});

describe("lifecycle state", () => {
  test("continuation state pins a non-negative integer cursor", () => {
    const state = S.decodeUnknownSync(HarnessContinuationState)({
      harnessId: "codex",
      sessionId: "s1",
      turnId: "t1",
      cursor: 7,
      lossy: false,
      data: { threadId: "abc" },
    });
    expect(state.cursor).toBe(7);
  });

  test("rejects a negative cursor", () => {
    expect(() =>
      S.decodeUnknownSync(HarnessContinuationState)({
        harnessId: "codex",
        sessionId: "s1",
        turnId: "t1",
        cursor: -1,
        lossy: false,
        data: {},
      }),
    ).toThrow();
  });
});

describe("bootstrap recipe", () => {
  test("decodes files and commands under a stable identity", () => {
    const boot = S.decodeUnknownSync(HarnessBootstrap)({
      identity: "claude-bridge@1",
      files: [{ path: "bridge/index.mjs", content: "// bridge", mode: 0o755 }],
      commands: [{ command: "pnpm install --frozen-lockfile" }],
    });
    expect(boot.identity).toBe("claude-bridge@1");
    expect(boot.files?.[0]?.mode).toBe(0o755);
  });
});
