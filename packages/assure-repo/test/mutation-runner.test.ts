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

// Passes on the original subject (exit 0) but, when the subject is mutated,
// terminates itself with a signal instead of returning an exit code — the way a
// timeout SIGTERM, an out-of-memory SIGKILL, or a crash would end the process.
const signalOnMutantOracle = (root: string): ReadonlyArray<string> => [
  "node",
  "-e",
  "const fs=require('fs');const p=require('path');const s=fs.readFileSync(p.join(process.argv[1],'subject.txt'),'utf8');if(s.includes('return 2')){process.exit(0)}else{process.kill(process.pid,'SIGKILL')}",
  root,
];

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

  // Regression for FREERANGE-02: a mutant whose test is terminated by a signal
  // (a timeout SIGTERM, an OOM SIGKILL, or a crash) returns no verdict and must
  // NOT be counted as killed, which would inflate the kill rate and make a weak
  // oracle read as sound. The "kills a mutant when the oracle is strong" test
  // above is the positive control through the same path: a real non-zero exit
  // code IS still a kill.
  test("reports inconclusive (not killed) when the mutant test is terminated by a signal", () => {
    const root = fixture();
    const outcome = runMutation(root, {
      subjectPath: "subject.txt",
      target: "return 2",
      replacement: "return 3",
      testCommand: signalOnMutantOracle(root),
    });
    expect(outcome.result).toBe("inconclusive");
    expect(outcome.result).not.toBe("killed");
    expect(outcome.detail).toContain("INCONCLUSIVE");
  });
});
