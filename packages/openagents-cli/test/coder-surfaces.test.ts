/**
 * The staged text surfaces, and the announcement that names them.
 *
 * `surfaces/coder/` owns the coder's optimizable text
 * (OpenAgentsInc/openagents#122) and `coder-surfaces.generated.ts` is the copy
 * this package compiles. `pnpm run check:coder-surfaces` is the gate that
 * refuses a stale build; this is the second net, inside the ordinary test
 * sweep, so a surface edited without the rebuild fails here too rather than
 * shipping the previous sentence in silence.
 */

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vite-plus/test";

import {
  CODER_SURFACE_DIGESTS,
  SYSTEM_PROMPT_SURFACE,
  TOOL_DESCRIPTION_SURFACE,
} from "../src/coder-surfaces.generated.js";
import { surfaceAnnouncement, systemPrompt, THREAD_LANE } from "../src/coder-system.js";

const artifact = (name: string): string =>
  readFileSync(fileURLToPath(new URL(`../../../surfaces/coder/${name}`, import.meta.url)), "utf8");

const index = JSON.parse(artifact("index.json")) as {
  surfaces: Record<string, { file: string; digest: string }>;
};

describe("the embedded surfaces", () => {
  test("carry the digest of the artifact they were built from", () => {
    for (const [id, digest] of Object.entries(CODER_SURFACE_DIGESTS)) {
      const entry = index.surfaces[id];
      expect(entry, `${id} is not in surfaces/coder/index.json`).toBeDefined();
      const found = `sha256:${createHash("sha256").update(artifact(entry!.file)).digest("hex")}`;
      expect(found, `${entry!.file} does not digest to what index.json pins`).toBe(entry!.digest);
      expect(digest, `the embedded ${id} digest is stale`).toBe(entry!.digest);
    }
  });

  test("hold text rather than placeholders", () => {
    for (const [key, value] of Object.entries({
      ...SYSTEM_PROMPT_SURFACE,
      ...TOOL_DESCRIPTION_SURFACE,
    })) {
      expect(value.length, `${key} is empty`).toBeGreaterThan(0);
    }
  });
});

describe("the staged-text announcement", () => {
  // The shape `packages/coder-effectiveness/src/harbor-job.ts` parses. A change
  // here without a change there loses the pin on every later bench row.
  test("is one line naming every surface by digest", () => {
    const line = surfaceAnnouncement();
    expect(line).toMatch(/^\[oa:surfaces .+\]$/u);
    const inner = /\[oa:surfaces ([^\]]+)\]/u.exec(line)?.[1];
    expect(inner).toBeDefined();
    const parsed = Object.fromEntries(
      inner!.split(",").map((pair) => {
        const at = pair.indexOf("=");
        return [pair.slice(0, at), pair.slice(at + 1)];
      }),
    );
    expect(parsed).toEqual(CODER_SURFACE_DIGESTS);
  });
});

describe("the system prompt", () => {
  test("is composed from the staged surface", () => {
    const prompt = systemPrompt([], THREAD_LANE);
    expect(prompt).toContain(SYSTEM_PROMPT_SURFACE["coder.concision"]);
    expect(prompt).toContain(SYSTEM_PROMPT_SURFACE["coder.no_tools"]);
    expect(prompt).toContain(THREAD_LANE);
    expect(prompt).not.toContain("{lane}");
  });

  test("singularizes a one-tool session and does not leave the placeholder", () => {
    const one = systemPrompt([{ name: "shell" } as never], THREAD_LANE);
    expect(one).toContain("You have 1 tool, and no others:");
    expect(one).not.toContain("{count}");
    expect(one).not.toContain("{plural}");
  });
});
