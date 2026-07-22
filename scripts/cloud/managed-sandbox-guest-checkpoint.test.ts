import { execFileSync, spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { afterEach, describe, expect, test } from "vitest";

const script = resolve(import.meta.dirname, "managed-sandbox-guest-checkpoint.py");
const roots: string[] = [];

const python = String.raw`
import importlib.util
import json
import pathlib
import sys

spec = importlib.util.spec_from_file_location("managed_sandbox_guest_checkpoint", sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
action = sys.argv[2]
workspace = pathlib.Path(sys.argv[3])
archive = pathlib.Path(sys.argv[4])
if action == "create":
    result = module.create_checkpoint(workspace, archive)
elif action == "inspect":
    result = module.inspect_checkpoint(archive, sys.argv[5])
elif action == "restore":
    result = module.restore_checkpoint(workspace, archive, sys.argv[5])
elif action == "recover":
    result = {"recovered": module.recover_restore(workspace)}
else:
    raise RuntimeError("unknown test action")
print(json.dumps(result, separators=(",", ":"), sort_keys=True))
`;

type CheckpointResult = Readonly<{
  contentBytes: number;
  contentDigest: string;
  entryCount: number;
  formatRef: string;
  repositoryPostImageDigest: string;
  unpackedBytes: number;
}>;

const makeRoot = () => {
  const root = mkdtempSync(join(tmpdir(), "oa-msb-checkpoint-test-"));
  roots.push(root);
  return root;
};

const invoke = (
  action: "create" | "inspect" | "restore" | "recover",
  workspace: string,
  archive: string,
  digest?: string,
) =>
  JSON.parse(
    execFileSync(
      "python3",
      ["-c", python, script, action, workspace, archive, ...(digest ? [digest] : [])],
      { encoding: "utf8" },
    ),
  ) as CheckpointResult & Readonly<{ recovered?: boolean }>;

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { force: true, recursive: true });
});

describe("managed-sandbox guest content checkpoint", () => {
  test("creates deterministic content-only bytes and excludes credential paths", () => {
    const root = makeRoot();
    const workspace = join(root, "workspace");
    const firstArchive = join(root, "first.tar");
    const secondArchive = join(root, "second.tar");
    mkdirSync(join(workspace, "src"), { recursive: true });
    mkdirSync(join(workspace, ".git"));
    mkdirSync(join(workspace, "nested", ".config", "gcloud"), { recursive: true });
    writeFileSync(join(workspace, "src", "main.ts"), "export const answer = 42;\n");
    writeFileSync(join(workspace, "run.sh"), "#!/bin/sh\necho ready\n");
    chmodSync(join(workspace, "run.sh"), 0o755);
    writeFileSync(join(workspace, ".env"), "TOKEN=must-not-move\n");
    writeFileSync(join(workspace, ".git", "config"), "credential=must-not-move\n");
    writeFileSync(
      join(workspace, "nested", ".config", "gcloud", "credentials.json"),
      "must-not-move\n",
    );

    const first = invoke("create", workspace, firstArchive);
    utimesSync(join(workspace, "src", "main.ts"), new Date(0), new Date());
    const second = invoke("create", workspace, secondArchive);
    const names = execFileSync("tar", ["-tf", firstArchive], { encoding: "utf8" });
    const bytes = readFileSync(firstArchive);

    expect(first).toEqual(second);
    expect(readFileSync(secondArchive)).toEqual(bytes);
    expect(first).toMatchObject({
      entryCount: 4,
      formatRef: "format.sbx.content-tar.v1",
      unpackedBytes: 47,
    });
    expect(first.contentDigest).toMatch(/^sha256:[0-9a-f]{64}$/u);
    expect(first.repositoryPostImageDigest).toMatch(/^sha256:[0-9a-f]{64}$/u);
    expect(names).toContain("src/main.ts");
    expect(names).toContain("run.sh");
    expect(names).not.toContain(".env");
    expect(names).not.toContain(".git");
    expect(bytes.toString("utf8")).not.toContain("must-not-move");
  });

  test("refuses symlinks and special archive members", () => {
    const root = makeRoot();
    const workspace = join(root, "workspace");
    const archive = join(root, "checkpoint.tar");
    mkdirSync(workspace);
    writeFileSync(join(root, "outside"), "outside\n");
    symlinkSync(join(root, "outside"), join(workspace, "escape"));

    const create = spawnSync("python3", ["-c", python, script, "create", workspace, archive], {
      encoding: "utf8",
    });
    expect(create.status).not.toBe(0);
    expect(create.stderr).toContain("checkpoint_symlink_refused");

    rmSync(join(workspace, "escape"));
    execFileSync("python3", [
      "-c",
      String.raw`
import io
import tarfile
import sys
with tarfile.open(sys.argv[1], "w") as archive:
    member = tarfile.TarInfo("escape")
    member.type = tarfile.SYMTYPE
    member.linkname = "/etc/passwd"
    archive.addfile(member)
`,
      archive,
    ]);
    const digest = execFileSync("shasum", ["-a", "256", archive], {
      encoding: "utf8",
    })
      .trim()
      .split(/\s+/u)[0];
    const inspect = spawnSync(
      "python3",
      ["-c", python, script, "inspect", workspace, archive, `sha256:${digest}`],
      { encoding: "utf8" },
    );
    expect(inspect.status).not.toBe(0);
    expect(inspect.stderr).toContain("checkpoint_special_file_refused");
  });

  test("verifies the archive before a crash-recoverable staged restore", () => {
    const root = makeRoot();
    const source = join(root, "source");
    const destination = join(root, "destination");
    const archive = join(root, "checkpoint.tar");
    mkdirSync(join(source, "src"), { recursive: true });
    mkdirSync(destination);
    writeFileSync(join(source, "src", "main.ts"), "restored\n");
    writeFileSync(join(source, "run.sh"), "#!/bin/sh\nexit 0\n");
    chmodSync(join(source, "run.sh"), 0o755);
    writeFileSync(join(destination, "old.txt"), "old\n");

    const created = invoke("create", source, archive);
    const inspected = invoke("inspect", destination, archive, created.contentDigest);
    const restored = invoke("restore", destination, archive, created.contentDigest);

    expect(inspected).toEqual(created);
    expect(restored.repositoryPostImageDigest).toBe(created.repositoryPostImageDigest);
    expect(readFileSync(join(destination, "src", "main.ts"), "utf8")).toBe("restored\n");
    expect(statSync(join(destination, "run.sh")).mode & 0o111).not.toBe(0);
    expect(existsSync(join(destination, "old.txt"))).toBe(false);
    expect(existsSync(join(destination, ".openagents-checkpoint-restore-state.json"))).toBe(false);
  });

  test("rolls back an interrupted replace before the next restore", () => {
    const root = makeRoot();
    const workspace = join(root, "workspace");
    const archive = join(root, "unused.tar");
    mkdirSync(join(workspace, ".openagents-checkpoint-restore-backup"), {
      recursive: true,
    });
    mkdirSync(join(workspace, ".openagents-checkpoint-restore-stage"));
    writeFileSync(
      join(workspace, ".openagents-checkpoint-restore-backup", "original.txt"),
      "original\n",
    );
    writeFileSync(join(workspace, "partial.txt"), "partial\n");
    writeFileSync(
      join(workspace, ".openagents-checkpoint-restore-state.json"),
      '{"state":"installing"}',
    );

    const recovered = invoke("recover", workspace, archive);

    expect(recovered.recovered).toBe(true);
    expect(readFileSync(join(workspace, "original.txt"), "utf8")).toBe("original\n");
    expect(existsSync(join(workspace, "partial.txt"))).toBe(false);
    expect(existsSync(join(workspace, ".openagents-checkpoint-restore-backup"))).toBe(false);
  });

  test("recovers both backup phases without deleting valid content", () => {
    const root = makeRoot();
    const backingUp = join(root, "backing-up");
    const committed = join(root, "committed");
    const archive = join(root, "unused.tar");
    for (const workspace of [backingUp, committed]) {
      mkdirSync(join(workspace, ".openagents-checkpoint-restore-backup"), {
        recursive: true,
      });
      mkdirSync(join(workspace, ".openagents-checkpoint-restore-stage"));
    }
    writeFileSync(join(backingUp, "still-original.txt"), "still original\n");
    writeFileSync(
      join(backingUp, ".openagents-checkpoint-restore-backup", "moved-original.txt"),
      "moved original\n",
    );
    writeFileSync(
      join(backingUp, ".openagents-checkpoint-restore-state.json"),
      '{"state":"backing_up"}',
    );
    writeFileSync(join(committed, "new.txt"), "new\n");
    writeFileSync(join(committed, ".openagents-checkpoint-restore-backup", "old.txt"), "old\n");
    writeFileSync(
      join(committed, ".openagents-checkpoint-restore-state.json"),
      '{"state":"committed"}',
    );

    expect(invoke("recover", backingUp, archive).recovered).toBe(true);
    expect(invoke("recover", committed, archive).recovered).toBe(true);

    expect(readFileSync(join(backingUp, "still-original.txt"), "utf8")).toBe("still original\n");
    expect(readFileSync(join(backingUp, "moved-original.txt"), "utf8")).toBe("moved original\n");
    expect(readFileSync(join(committed, "new.txt"), "utf8")).toBe("new\n");
    expect(existsSync(join(committed, "old.txt"))).toBe(false);
  });

  test("fails closed when checkpoint bytes change", () => {
    const root = makeRoot();
    const workspace = join(root, "workspace");
    const archive = join(root, "checkpoint.tar");
    mkdirSync(workspace);
    writeFileSync(join(workspace, "main.txt"), "content\n");
    const created = invoke("create", workspace, archive);
    writeFileSync(archive, Buffer.concat([readFileSync(archive), Buffer.from("tamper")]));

    const inspect = spawnSync(
      "python3",
      ["-c", python, script, "inspect", workspace, archive, created.contentDigest],
      { encoding: "utf8" },
    );

    expect(inspect.status).not.toBe(0);
    expect(inspect.stderr).toContain("checkpoint_digest_mismatch");
  });
});
