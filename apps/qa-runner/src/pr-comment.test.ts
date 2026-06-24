// PR-comment + gh-attach composition tests (#6185) — unit + dry-run, NO network.
//
// Proves the PR-evidence comment is composed honestly with a fake gh-attach
// runner: the gh-attach'd embed is used when the upload succeeds; the in-eval
// relative video ref is used when gh-attach is unavailable/unauthenticated (no
// broken/fake embed); the /pro link is always present; and a failing variant is
// reported as failing (no fake green).

import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { localBackend } from "./backend";
import { scriptedBrain } from "./brain";
import { type EvalVariant, runEval } from "./evals";
import { makeFakeChromium } from "./fake-chromium";
import {
  ghAttachUpload,
  ghAttachVariantVideos,
  type GhAttachRunner,
} from "./gh-attach";
import {
  composePrComment,
  PR_COMMENT_MARKER,
} from "./pr-comment";
import { loginRegressionSteps, loginRegressionStepsWrong } from "./scenarios";
import { makeTarget } from "./target";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-pr-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const passingChromium = () =>
  makeFakeChromium({
    pages: {
      "/login": {
        text: "Log in to OpenAgents",
        html: "<form>Log in to OpenAgents</form>",
      },
    },
  });

const variants = (): ReadonlyArray<EvalVariant> => [
  {
    id: "mcp-on",
    label: "MCP on",
    brain: () => scriptedBrain(loginRegressionSteps()),
    backend: () => localBackend({ chromium: passingChromium() }),
  },
  {
    id: "mcp-off",
    label: "MCP off",
    brain: () => scriptedBrain(loginRegressionStepsWrong()),
    backend: () => localBackend({ chromium: passingChromium() }),
  },
];

const runSampleEval = () =>
  Effect.runPromise(
    runEval({
      id: "login-mcp-compare",
      title: "Login under MCP on vs off",
      target: makeTarget({ name: "fake", baseUrl: "https://example.test" }),
      scenario: { id: "login-regression", label: "/login renders sign-in" },
      variants: variants(),
      artifactDir: dir,
      now: () => new Date("2026-06-24T00:00:00.000Z"),
    }),
  );

// A fake gh-attach that "uploads" by echoing a deterministic asset URL.
const fakeOkRunner: GhAttachRunner = {
  run: async args => {
    const file = args[args.length - 1] ?? "file";
    const base = file.split("/").pop() ?? "file";
    return {
      ok: true,
      stdout: `![${base}](https://github.com/o/r/assets/1/${base})\n`,
    };
  },
};

const fakeUnavailableRunner: GhAttachRunner = {
  run: async () => ({ ok: false, stdout: "" }),
};

describe("ghAttachUpload (fake runner)", () => {
  test("returns the embeddable markdown on success", async () => {
    const md = await ghAttachUpload(fakeOkRunner, "/tmp/clip.webm", {
      repo: "o/r",
    });
    expect(md).toBe(
      "![clip.webm](https://github.com/o/r/assets/1/clip.webm)",
    );
  });

  test("returns null when gh-attach is unavailable (honest, no fake embed)", async () => {
    const md = await ghAttachUpload(fakeUnavailableRunner, "/tmp/clip.webm");
    expect(md).toBeNull();
  });

  test("returns null when the runner throws (binary missing)", async () => {
    const throwing: GhAttachRunner = {
      run: async () => {
        throw new Error("gh-attach: command not found");
      },
    };
    const md = await ghAttachUpload(throwing, "/tmp/clip.webm");
    expect(md).toBeNull();
  });

  test("ghAttachVariantVideos maps only the successful uploads", async () => {
    const map = await ghAttachVariantVideos(fakeOkRunner, [
      { variantId: "a", filePath: "/tmp/a.webm" },
      { variantId: "b", filePath: "/tmp/b.webm" },
    ]);
    expect(Object.keys(map).sort()).toEqual(["a", "b"]);
    expect(map.a).toContain("a.webm");
  });
});

describe("composePrComment", () => {
  test("uses the gh-attach'd embed when upload succeeds + always includes the /pro link", async () => {
    const outcome = await runSampleEval();
    const onVideo = outcome.result.variants
      .find(v => v.variantId === "mcp-on")!
      .runs[0]!.video!;
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
      variantVideoPaths: [
        { variantId: "mcp-on", filePath: join(dir, onVideo) },
      ],
      ghAttach: fakeOkRunner,
      ghAttachOptions: { repo: "OpenAgentsInc/openagents" },
    });
    expect(body.startsWith(PR_COMMENT_MARKER)).toBe(true);
    expect(body).toContain("github.com/o/r/assets")
    expect(body).toContain(
      "https://openagents.com/pro/evals/login-mcp-compare",
    );
    // honest: a failing variant is reported as failing.
    expect(body).toContain("some variants failed")
  });

  test("falls back to the relative video ref when gh-attach is unavailable", async () => {
    const outcome = await runSampleEval();
    const onVideo = outcome.result.variants
      .find(v => v.variantId === "mcp-on")!
      .runs[0]!.video!;
    // the artifact really exists on disk (dereferenceable)
    expect(existsSync(join(dir, onVideo))).toBe(true);
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
      variantVideoPaths: [
        { variantId: "mcp-on", filePath: join(dir, onVideo) },
      ],
      ghAttach: fakeUnavailableRunner,
    });
    // no broken embed; the relative ref is shown instead.
    expect(body).not.toContain("github.com/o/r/assets");
    expect(body).toContain(onVideo);
    expect(body).toContain(
      "https://openagents.com/pro/evals/login-mcp-compare",
    );
  });

  test("composes without any gh-attach runner (pure dry-run)", async () => {
    const outcome = await runSampleEval();
    const body = await composePrComment({
      result: outcome.result,
      proBaseUrl: "https://openagents.com",
    });
    expect(body).toContain(PR_COMMENT_MARKER);
    expect(body).toContain("Chill-eval");
    expect(body).toContain("/pro/evals/login-mcp-compare");
  });
});
