import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  symlinkSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { afterEach, describe, expect, test } from "vite-plus/test";
import { canonicalFailureMatrixPaths } from "./failure-matrix-paths.js";

const roots: string[] = [];

const fixture = () => {
  const root = mkdtempSync(join(tmpdir(), "sarah-failure-paths-"));
  roots.push(root);
  const repository = join(root, "repository");
  const receiptRoot = join(repository, "docs/ops/receipts/livekit");
  const outside = join(root, "outside");
  mkdirSync(receiptRoot, { recursive: true });
  mkdirSync(outside);
  const input = join(outside, "observation.json");
  writeFileSync(input, "{}");
  return { repository, receiptRoot, outside, input };
};

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

describe("failure matrix canonical paths", () => {
  test("accepts an external input and canonical receipt parent", () => {
    const value = fixture();
    expect(
      canonicalFailureMatrixPaths(
        value.repository,
        value.input,
        "docs/ops/receipts/livekit/result.json",
      ),
    ).toEqual({
      inputPath: realpathSync(value.input),
      receiptPath: join(realpathSync(value.receiptRoot), "result.json"),
    });
  });

  test("rejects input symlinks crossing the repository boundary in either direction", () => {
    const value = fixture();
    const repositoryInput = join(value.repository, "private.json");
    writeFileSync(repositoryInput, "{}");
    const outsideLink = join(value.outside, "into-repository.json");
    symlinkSync(repositoryInput, outsideLink);
    expect(() =>
      canonicalFailureMatrixPaths(
        value.repository,
        outsideLink,
        "docs/ops/receipts/livekit/result.json",
      ),
    ).toThrow("must remain outside");

    const repositoryLink = join(value.repository, "out-of-repository.json");
    symlinkSync(value.input, repositoryLink);
    expect(() =>
      canonicalFailureMatrixPaths(
        value.repository,
        repositoryLink,
        "docs/ops/receipts/livekit/result.json",
      ),
    ).toThrow("must remain outside");
  });

  test("rejects receipt parents that escape through a symlink", () => {
    const value = fixture();
    symlinkSync(value.outside, join(value.receiptRoot, "escape"));
    expect(() =>
      canonicalFailureMatrixPaths(
        value.repository,
        value.input,
        "docs/ops/receipts/livekit/escape/result.json",
      ),
    ).toThrow("must be under docs/ops/receipts/livekit");

    const outsideLink = join(value.outside, "into-receipt-root");
    symlinkSync(value.receiptRoot, outsideLink);
    expect(() =>
      canonicalFailureMatrixPaths(
        value.repository,
        value.input,
        join(outsideLink, "result.json"),
      ),
    ).toThrow("must be under docs/ops/receipts/livekit");
  });
});
