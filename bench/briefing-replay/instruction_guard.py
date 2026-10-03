#!/usr/bin/env python3
"""Freeze and verify byte-identical required instructions for both experiment arms."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import stat
import tempfile
import unittest

SCHEMA = "openagents.briefing.instruction-guard.v1"
MAX_BYTES = 1024 * 1024
MAX_PROMPT = 4 * MAX_BYTES


def digest(data):
    return hashlib.sha256(data).hexdigest()


def bounded(path, limit):
    with Path(path).open("rb") as handle:
        data = handle.read(limit + 1)
    if len(data) > limit:
        raise ValueError("Input exceeds its byte bound")
    return data


def instruction_file(snapshot, relative):
    parts = PurePosixPath(relative)
    if not relative or parts.is_absolute() or ".." in parts.parts or str(parts) != relative:
        raise ValueError("Instruction paths must be normalized repository-relative paths")
    current = Path(snapshot).resolve(strict=True)
    for part in parts.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError("Instruction paths must not traverse symlinks")
    if not stat.S_ISREG(current.stat().st_mode):
        raise ValueError("Instruction input must be a regular file")
    return bounded(current, 256 * 1024)


def freeze(snapshot, paths, source_revision):
    if not 1 <= len(paths) <= 32 or len(set(paths)) != len(paths):
        raise ValueError("Choose 1–32 distinct instruction paths")
    if len(source_revision) not in (40, 64) or any(c not in "0123456789abcdef" for c in source_revision):
        raise ValueError("Supply the full lowercase declared source revision")
    records = []
    pieces = [b"BEGIN FROZEN REQUIRED REPOSITORY INSTRUCTIONS\n"]
    for path in paths:
        content = instruction_file(snapshot, path)
        content.decode("utf-8")
        records.append({"path": path, "sha256": digest(content), "bytes": len(content)})
        pieces.extend([
            ("\nFILE " + path + " SHA256 " + digest(content) + "\n").encode(),
            content,
            b"\nEND FILE\n",
        ])
    pieces.append(b"END FROZEN REQUIRED REPOSITORY INSTRUCTIONS\n")
    block = b"".join(pieces)
    if len(block) > MAX_BYTES:
        raise ValueError("Combined instructions exceed 1 MiB")
    manifest = {
        "schema": SCHEMA,
        "declared_source_revision": source_revision,
        "files": records,
        "block_sha256": digest(block),
        "block_bytes": len(block),
        "scope": "Byte integrity only; instruction applicability requires independent review",
    }
    return manifest, block


def verify(manifest, block, workspace, prompt):
    if set(manifest) != {"schema", "declared_source_revision", "files", "block_sha256", "block_bytes", "scope"}:
        raise ValueError("Unknown or missing instruction manifest fields")
    if manifest["schema"] != SCHEMA or len(block) > MAX_BYTES or len(prompt) > MAX_PROMPT:
        raise ValueError("Invalid instruction manifest or byte bound")
    if digest(block) != manifest["block_sha256"] or len(block) != manifest["block_bytes"]:
        raise ValueError("Required instruction block changed")
    records = manifest["files"]
    if not isinstance(records, list) or not 1 <= len(records) <= 32:
        raise ValueError("Invalid instruction file count")
    for record in records:
        if set(record) != {"path", "sha256", "bytes"}:
            raise ValueError("Invalid instruction file record")
        content = instruction_file(workspace, record["path"])
        if digest(content) != record["sha256"] or len(content) != record["bytes"]:
            raise ValueError("Required instruction file changed: " + record["path"])
    rebuilt, rebuilt_block = freeze(workspace, [r["path"] for r in records], manifest["declared_source_revision"])
    if rebuilt_block != block or rebuilt["files"] != records:
        raise ValueError("The block does not contain the recorded complete instruction files")
    if prompt.count(block) != 1:
        raise ValueError("Prompt must contain the exact required instruction block once")
    return {"instruction_block_sha256": digest(block), "prompt_sha256": digest(prompt), "files_checked": len(records)}


def verify_pair(control, treatment, suffix):
    if max(len(control), len(treatment), len(suffix)) > MAX_PROMPT:
        raise ValueError("Prompt exceeds its byte bound")
    if treatment != control + suffix:
        raise ValueError("Treatment must equal the complete control input plus the frozen suffix")


def create(path, data):
    with Path(path).open("xb") as handle:
        handle.write(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="operation", required=True)
    new = sub.add_parser("freeze")
    new.add_argument("--snapshot", type=Path, required=True)
    new.add_argument("--source-revision", required=True)
    new.add_argument("--path", action="append", required=True)
    new.add_argument("--manifest", type=Path, required=True)
    new.add_argument("--block", type=Path, required=True)
    check = sub.add_parser("verify")
    check.add_argument("--manifest", type=Path, required=True)
    check.add_argument("--block", type=Path, required=True)
    check.add_argument("--workspace", type=Path, required=True)
    check.add_argument("--prompt", type=Path, required=True)
    pair = sub.add_parser("pair")
    pair.add_argument("--control", type=Path, required=True)
    pair.add_argument("--treatment", type=Path, required=True)
    pair.add_argument("--suffix", type=Path, required=True)
    sub.add_parser("self-test")
    args = parser.parse_args()
    if args.operation == "self-test":
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardTests)
        result = unittest.TextTestRunner().run(suite)
        raise SystemExit(0 if result.wasSuccessful() else 1)
    if args.operation == "freeze":
        manifest, block = freeze(args.snapshot, args.path, args.source_revision)
        create(args.block, block)
        create(args.manifest, (json.dumps(manifest, indent=2) + "\n").encode())
        print(json.dumps({"block_sha256": manifest["block_sha256"], "files": len(manifest["files"])}))
    elif args.operation == "verify":
        manifest = json.loads(bounded(args.manifest, MAX_BYTES))
        print(json.dumps(verify(manifest, bounded(args.block, MAX_BYTES), args.workspace, bounded(args.prompt, MAX_PROMPT))))
    else:
        verify_pair(bounded(args.control, MAX_PROMPT), bounded(args.treatment, MAX_PROMPT), bounded(args.suffix, MAX_PROMPT))
        print(json.dumps({"identical_common_input": True}))


class GuardTests(unittest.TestCase):
    def test_complete_block_and_unchanged_files_are_required(self):
        with tempfile.TemporaryDirectory(prefix="instruction-guard-") as temporary:
            root = Path(temporary)
            (root / "AGENTS.md").write_text("Keep required rule one.\nKeep required rule two.\n")
            manifest, block = freeze(root, ["AGENTS.md"], "a" * 40)
            prompt = b"Task and operational overrides.\n" + block
            verify(manifest, block, root, prompt)
            with self.assertRaises(ValueError):
                verify(manifest, block, root, prompt.replace(b"Keep required rule two.", b""))
            (root / "AGENTS.md").write_text("Changed rule.\n")
            with self.assertRaises(ValueError):
                verify(manifest, block, root, prompt)

    def test_common_input_cannot_be_replaced_by_a_shorter_excerpt(self):
        verify_pair(b"task\ncomplete instructions\n", b"task\ncomplete instructions\nbrief", b"brief")
        with self.assertRaises(ValueError):
            verify_pair(b"task\ncomplete instructions\n", b"task\nbrief", b"brief")

    def test_paths_and_symlinks_cannot_select_other_files(self):
        with tempfile.TemporaryDirectory(prefix="instruction-guard-") as temporary:
            root = Path(temporary)
            (root / "AGENTS.md").write_text("Rule.\n")
            (root / "linked.md").symlink_to(root / "AGENTS.md")
            for relative in ["../AGENTS.md", "/AGENTS.md", "linked.md"]:
                with self.assertRaises(ValueError):
                    freeze(root, [relative], "a" * 40)


if __name__ == "__main__":
    main()
