import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, test } from "vite-plus/test";

import { MutationRunnerError, runMutation } from "../src/index.ts";

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

const fixture = (): string => {
  const root = mkdtempSync(join(tmpdir(), "assure-repo-mut-"));
  roots.push(root);
  writeFileSync(join(root, "subject.txt"), "answer = return 2\n");
  return root;
};

// A "strong" oracle reads the subject and requires the exact expected token; it
// fails when the subject is mutated. A "weak" oracle always passes.
const strongOracle = (root: string): ReadonlyArray<string> => [
  "node",
  "-e",
  "const fs=require('fs');process.exit(fs.readFileSync(require('path').join(process.argv[1],'subject.txt'),'utf8').includes('return 2')?0:1)",
  root,
];
const weakOracle = (): ReadonlyArray<string> => ["node", "-e", "process.exit(0)"];

describe("runMutation", () => {
  test("kills a mutant when the oracle is strong", () => {
    const root = fixture();
    const outcome = runMutation(root, {
      subjectPath: "subject.txt",
      target: "return 2",
      replacement: "return 3",
      testCommand: strongOracle(root),
    });
    expect(outcome.result).toBe("killed");
  });

  test("reports a surviving mutant when the oracle is weak (the false-green signal)", () => {
    const root = fixture();
    const outcome = runMutation(root, {
      subjectPath: "subject.txt",
      target: "return 2",
      replacement: "return 3",
      testCommand: weakOracle(),
    });
    expect(outcome.result).toBe("survived");
    expect(outcome.detail).toContain("WEAK ORACLE");
  });

  test("restores the original bytes after running", () => {
    const root = fixture();
    runMutation(root, {
      subjectPath: "subject.txt",
      target: "return 2",
      replacement: "return 3",
      testCommand: strongOracle(root),
    });
    const restored = require("node:fs").readFileSync(join(root, "subject.txt"), "utf8");
    expect(restored).toBe("answer = return 2\n");
  });

  test("rejects a no-op mutation", () => {
    const root = fixture();
    expect(() =>
      runMutation(root, {
        subjectPath: "subject.txt",
        target: "return 2",
        replacement: "return 2",
        testCommand: weakOracle(),
      }),
    ).toThrow(MutationRunnerError);
  });

  test("rejects a target that is not exactly once", () => {
    const root = fixture();
    writeFileSync(join(root, "subject.txt"), "x x\n");
    expect(() =>
      runMutation(root, {
        subjectPath: "subject.txt",
        target: "x",
        replacement: "y",
        testCommand: weakOracle(),
      }),
    ).toThrow(MutationRunnerError);
  });

  test("does not claim a kill when the baseline oracle already fails", () => {
    const root = fixture();
    const failingOracle: ReadonlyArray<string> = ["node", "-e", "process.exit(1)"];
    const outcome = runMutation(root, {
      subjectPath: "subject.txt",
      target: "return 2",
      replacement: "return 3",
      testCommand: failingOracle,
    });
    expect(outcome.result).toBe("error");
  });
});
