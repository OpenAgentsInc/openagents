import { describe, expect, test } from "vite-plus/test";

import { resolveRepositoryMainRef } from "./repository-main-ref.js";

const SHA = "a".repeat(40);

describe("repository main ref resolution", () => {
  test("prefers the forge remote without requiring that remote name", async () => {
    const called: string[] = [];
    const ref = await resolveRepositoryMainRef(async (...args) => {
      const candidate = args.at(-1) ?? "";
      called.push(candidate);
      if (candidate.startsWith("refs/remotes/openagents/main")) return SHA;
      throw new Error("missing ref");
    });

    expect(ref).toBe("refs/remotes/openagents/main");
    expect(called).toEqual(["refs/remotes/openagents/main^{commit}"]);
  });

  test("accepts origin or a local main when the forge remote has another name", async () => {
    const origin = await resolveRepositoryMainRef(async (...args) => {
      if (args.at(-1)?.startsWith("refs/remotes/origin/main")) return SHA;
      throw new Error("missing ref");
    });
    const local = await resolveRepositoryMainRef(async (...args) => {
      if (args.at(-1)?.startsWith("refs/heads/main")) return SHA;
      throw new Error("missing ref");
    });

    expect(origin).toBe("refs/remotes/origin/main");
    expect(local).toBe("refs/heads/main");
  });

  test("refuses when no main ref exists", async () => {
    await expect(
      resolveRepositoryMainRef(async () => {
        throw new Error("missing ref");
      }),
    ).rejects.toThrow("repository main is unavailable");
  });
});
