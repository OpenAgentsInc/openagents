#!/usr/bin/env python3
"""Lay the fix commit's own test changes over an agent's change (#11211).

    overlay.py TESTS_PATCH   (cwd: the checkout with the change applied)

For each file in the patch: apply its hunks as they are when they apply
cleanly. When they do not (the agent edited the same test module), insert
the lines the fix added instead: before the closing brace of the file's
`#[cfg(test)] mod` when the module is inline, or at the end of a test-only
file. Prints what it did; exits 1 when a file could not be overlaid.
"""
import re
import subprocess
import sys
import tempfile


def files_of(patch: str):
    for block in re.split(r"(?m)^(?=diff --git )", patch):
        if block.startswith("diff --git"):
            path = re.match(r"diff --git a/(\S+) b/(\S+)", block).group(2)
            yield path, block


def added_blocks(block: str) -> list[str]:
    out, cur = [], []
    for line in block.splitlines():
        if line.startswith("+++") or line.startswith("---"):
            continue
        if line.startswith("+"):
            cur.append(line[1:])
        elif cur:
            out.append("\n".join(cur))
            cur = []
    if cur:
        out.append("\n".join(cur))
    return out


def inject(path: str, block: str) -> bool:
    try:
        text = open(path).read()
    except FileNotFoundError:
        text = ""
    added = "\n\n".join(added_blocks(block)).rstrip() + "\n"
    inline = re.search(r"(?m)^\s*#\[cfg\(test\)\]\s*\n\s*mod\s+\w+\s*\{", text)
    if inline and not re.search(r"(^|/)tests(/|\.rs$)", path):
        end = text.rstrip().rfind("}")
        if end < inline.start():
            return False
        text = text[:end] + "\n" + added + text[end:]
    else:
        text = text.rstrip("\n") + "\n\n" + added
    open(path, "w").write(text)
    return True


def main() -> int:
    patch = open(sys.argv[1]).read()
    ok = True
    for path, block in files_of(patch):
        with tempfile.NamedTemporaryFile("w", suffix=".patch", delete=False) as f:
            f.write(block)
        applied = subprocess.run(["git", "apply", "--whitespace=nowarn", f.name], capture_output=True)
        if applied.returncode == 0:
            print(f"{path}: applied")
            continue
        if inject(path, block):
            print(f"{path}: injected the fix's added lines")
        else:
            print(f"{path}: could not overlay")
            ok = False
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
