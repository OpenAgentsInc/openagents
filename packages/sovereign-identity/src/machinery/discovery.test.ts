import { chmodSync, lstatSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Effect } from "effect";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { PUBLIC_TEST_MNEMONIC } from "../contract/index.ts";
import {
  type CandidateStatProbe,
  inspectCandidatePath,
  nodeFsCandidateSource,
  summarizeDiagnostics,
} from "./discovery.ts";

/**
 * Existence-only discovery: `lstat` metadata only, symbolic-link refusal,
 * weak-permission custody blocker, and a proof that discovery reads no secret
 * bytes and mutates no file.
 */
describe("existence-only candidate discovery", () => {
  let dir: string;
  const paths = {
    goodFile: "",
    weakFile: "",
    symlink: "",
    directory: "",
    absent: "",
  };

  beforeAll(() => {
    dir = mkdtempSync(path.join(tmpdir(), "idr02-discovery-"));
    paths.goodFile = path.join(dir, "identity.mnemonic");
    paths.weakFile = path.join(dir, "weak.mnemonic");
    paths.symlink = path.join(dir, "identity.symlink");
    paths.directory = path.join(dir, "a-directory");
    paths.absent = path.join(dir, "missing.mnemonic");

    // The file content is the PUBLIC TEST mnemonic, never a real secret. It lets
    // the "no secret bytes" proof assert the phrase never reaches diagnostics.
    writeFileSync(paths.goodFile, `${PUBLIC_TEST_MNEMONIC}\n`, { mode: 0o600 });
    chmodSync(paths.goodFile, 0o600);
    writeFileSync(paths.weakFile, `${PUBLIC_TEST_MNEMONIC}\n`, { mode: 0o644 });
    chmodSync(paths.weakFile, 0o644);
    symlinkSync(paths.goodFile, paths.symlink);
    writeFileSync(path.join(dir, "a-directory-marker"), "x");
    // Reuse a real directory: the temp dir itself is a directory candidate.
    paths.directory = dir;
  });

  afterAll(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  test("a 0600 regular file is an admissible candidate with the exact mode", () => {
    const diagnostic = inspectCandidatePath({
      sourceLabel: "primary_pylon_identity",
      absolutePath: paths.goodFile,
    });
    expect(diagnostic.fsType).toBe("regular_file");
    expect(diagnostic.present).toBe(true);
    expect(diagnostic.admissible).toBe(true);
    expect(diagnostic.permissionMode).toBe("600");
    expect(diagnostic.blocker).toBeNull();
    // The source label is public; the raw private path never appears.
    expect(JSON.stringify(diagnostic)).not.toContain(paths.goodFile);
  });

  test("a weak-permission file reports a custody blocker and is not admissible", () => {
    const diagnostic = inspectCandidatePath({
      sourceLabel: "weak_perms_candidate",
      absolutePath: paths.weakFile,
    });
    expect(diagnostic.fsType).toBe("regular_file");
    expect(diagnostic.present).toBe(true);
    expect(diagnostic.admissible).toBe(false);
    expect(diagnostic.blocker).toBe("weak_permissions");
    expect(diagnostic.permissionMode).toBe("644");
  });

  test("a symbolic-link candidate is refused by default", () => {
    const diagnostic = inspectCandidatePath({
      sourceLabel: "symlink_candidate",
      absolutePath: paths.symlink,
    });
    expect(diagnostic.fsType).toBe("symbolic_link");
    expect(diagnostic.present).toBe(true);
    expect(diagnostic.admissible).toBe(false);
    expect(diagnostic.blocker).toBe("link_refused");
  });

  test("a missing candidate is absent, never an error, and never admissible", () => {
    const diagnostic = inspectCandidatePath({
      sourceLabel: "missing_candidate",
      absolutePath: paths.absent,
    });
    expect(diagnostic.fsType).toBe("absent");
    expect(diagnostic.present).toBe(false);
    expect(diagnostic.admissible).toBe(false);
    expect(diagnostic.blocker).toBeNull();
  });

  test("a directory candidate is present but not admissible", () => {
    const diagnostic = inspectCandidatePath({
      sourceLabel: "directory_candidate",
      absolutePath: paths.directory,
    });
    expect(diagnostic.fsType).toBe("directory");
    expect(diagnostic.admissible).toBe(false);
  });

  test("Windows platform mode skips the POSIX permission blocker", () => {
    const diagnostic = inspectCandidatePath(
      { sourceLabel: "weak_on_windows", absolutePath: paths.weakFile },
      { platform: "win32" },
    );
    expect(diagnostic.fsType).toBe("regular_file");
    expect(diagnostic.admissible).toBe(true);
    expect(diagnostic.permissionMode).toBeNull();
    expect(diagnostic.blocker).toBeNull();
  });

  test("discovery reads NO secret bytes: only lstat is used and no phrase leaks", async () => {
    const lstatCalls: string[] = [];
    const readAttempts: string[] = [];
    // A probe with `lstat` only. The extra throwing methods are never on the
    // typed surface; they exist only to catch an accidental read at runtime.
    const guardedProbe: CandidateStatProbe & {
      readFileSync: () => never;
      readSync: () => never;
      openSync: () => never;
    } = {
      lstatSync: (candidate: string) => {
        lstatCalls.push(candidate);
        return lstatSync(candidate);
      },
      readFileSync: () => {
        readAttempts.push("readFileSync");
        throw new Error("discovery must not read secret bytes");
      },
      readSync: () => {
        readAttempts.push("readSync");
        throw new Error("discovery must not read secret bytes");
      },
      openSync: () => {
        readAttempts.push("openSync");
        throw new Error("discovery must not read secret bytes");
      },
    };

    const source = nodeFsCandidateSource(
      [
        { sourceLabel: "primary_pylon_identity", absolutePath: paths.goodFile },
        { sourceLabel: "weak_perms_candidate", absolutePath: paths.weakFile },
        { sourceLabel: "missing_candidate", absolutePath: paths.absent },
      ],
      { probe: guardedProbe },
    );

    const diagnostics = await Effect.runPromise(source.discover());

    expect(readAttempts).toEqual([]);
    expect(lstatCalls.length).toBe(3);
    // The known-good file holds the PUBLIC TEST mnemonic; it must not leak into
    // any diagnostic, proving discovery never read the file content.
    expect(JSON.stringify(diagnostics)).not.toContain(PUBLIC_TEST_MNEMONIC);

    const { admissible, blockers } = summarizeDiagnostics(diagnostics);
    expect(admissible.map((entry) => entry.sourceLabel)).toEqual(["primary_pylon_identity"]);
    expect(blockers).toContain("weak_permissions");
  });
});
