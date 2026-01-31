#!/usr/bin/env python3
"""Restore all transcript files from git history to docs/transcripts/."""
import os
import subprocess
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__) or ".", "."))

PATHS = [
    "docs/transcripts/001.md",
    "docs/transcripts/002.md",
    "docs/transcripts/003.md",
    "docs/transcripts/092.md",
    "docs/transcripts/093.md",
    "docs/transcripts/094.md",
    "docs/transcripts/095.md",
    "docs/transcripts/119.md",
    "docs/transcripts/125.md",
    "docs/transcripts/138.md",
    "docs/transcripts/141.md",
    "docs/transcripts/142.md",
    "docs/transcripts/149.md",
    "docs/transcripts/150.md",
    "docs/transcripts/153.md",
    "docs/transcripts/164.md",
    "docs/transcripts/166.md",
    "docs/transcripts/167.md",
    "docs/transcripts/169.md",
    "docs/transcripts/170.md",
    "docs/transcripts/171.md",
    "docs/transcripts/172.md",
    "docs/transcripts/173.md",
    "docs/transcripts/174.md",
    "docs/transcripts/175.md",
    "docs/transcripts/176.md",
    "docs/transcripts/177.md",
    "docs/transcripts/178.md",
    "docs/transcripts/179.md",
    "docs/transcripts/180.md",
    "docs/transcripts/188.md",
    "docs/transcripts/189.md",
    "docs/transcripts/190.md",
    "docs/transcripts/191.md",
    "docs/transcripts/192.md",
    "docs/transcripts/193.md",
    "docs/transcripts/195.md",
    "docs/transcripts/196.md",
    "docs/transcripts/197.md",
    "docs/transcripts/198.md",
    "docs/transcripts/194-the-trillion-dollar-question.md",
    "docs/transcripts/199-introducing-autopilot.md",
    "docs/transcripts/200-the-agent-network.md",
    "docs/transcripts/201-fracking-apple-silicon.md",
    "docs/transcripts/202-recursive-language-models.md",
    "docs/transcripts/20250203-1157-ep-157.md",
    "docs/transcripts/203-pylon-and-nexus.md",
    "docs/transcripts/dont-build-agents-build-skills.md",
    "docs/transcripts/dspy/dspy-is-all-you-need.md",
    "docs/transcripts/dspy/state-of-dspy.md",
    "docs/transcripts/ep150.md",
    "docs/transcripts/ep167.md",
    "docs/transcripts/ep168.md",
    "docs/transcripts/oa-186-actions-per-minute.md",
    "docs/transcripts/oa-194-trillion-dollar-question.md",
    "docs/transcripts/README.md",
]


def get_commits(path):
    """Return list of commits that touched path (newest first)."""
    r = subprocess.run(
        ["git", "log", "--all", "--full-history", "--pretty=format:%H", "--", path],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        return []
    return r.stdout.strip().splitlines() if r.stdout.strip() else []


def get_file_at(rev, path):
    """Return file content at rev or None."""
    r = subprocess.run(
        ["git", "show", f"{rev}:{path}"],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    if r.returncode != 0 or not r.stdout:
        return None
    return r.stdout


def main():
    os.chdir(REPO)
    # Restore to docs/transcripts/ at original paths
    out_root = "docs/transcripts"
    # Also save every transcript under docs/transcripts/openagents/ (flat by basename for root files)
    openagents = "docs/transcripts/openagents"
    restored = 0
    for path in PATHS:
        for rev in get_commits(path):
            content = get_file_at(rev, path)
            if content:
                # Original path under docs/transcripts/
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "w") as f:
                    f.write(content)
                # Copy into openagents: same name under openagents (e.g. 001.md, dspy/state-of-dspy.md)
                rel = path[len(out_root) + 1 :]  # e.g. 001.md or dspy/state-of-dspy.md
                dest = os.path.join(openagents, rel)
                os.makedirs(os.path.dirname(dest), exist_ok=True)
                with open(dest, "w") as f:
                    f.write(content)
                print(f"Restored {path} -> also {dest} (from {rev[:8]})")
                restored += 1
                break
        else:
            print(f"Could not restore {path}", file=sys.stderr)
    print(f"\nDone. Restored {restored}/{len(PATHS)} files.")


if __name__ == "__main__":
    main()
