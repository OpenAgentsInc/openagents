#!/usr/bin/env python3
"""Check the retained audit inventory without running product code."""

import csv
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit


def main():
    evidence = Path(__file__).resolve().parent
    audit = evidence.parent
    root = audit.parents[2]
    snapshot = json.loads((evidence / "snapshot.json").read_text())
    revision = snapshot["source_revision"]
    failures = []

    def rows(name):
        with (evidence / name).open(newline="") as handle:
            return list(csv.DictReader(handle))

    required = rows("required-reading.csv")
    reviewed = rows("source-review.csv")
    issues = rows("issues.csv")
    inherited = rows("prior-health-findings.csv")
    expected_counts = {
        "required_files": len(required),
        "open_issues": sum(row["state"] == "open" for row in issues),
        "recently_closed_issues": sum(row["state"] == "closed" for row in issues),
        "historical_health_rows": len(inherited),
    }
    for key, actual in expected_counts.items():
        if actual != snapshot[key]:
            failures.append(f"{key}: expected {snapshot[key]}, got {actual}")
    for name, data, key in (
        ("requested files", required, "path"),
        ("issues", issues, "number"),
        ("historical findings", inherited, "id"),
    ):
        if len({row[key] for row in data}) != len(data):
            failures.append(f"Duplicate identities in {name}")
    for row in required:
        if not row["reviewers"] or not row["coverage"]:
            failures.append(f"Missing review coverage: {row['path']}")
    if sum(int(row["bytes"]) for row in required) != snapshot["required_bytes"]:
        failures.append("Requested file byte total differs")
    if sum(int(row["words"]) for row in required) != snapshot["required_words"]:
        failures.append("Requested file word total differs")

    source_hashes = {}
    for row in required + reviewed:
        path = row["path"]
        if path not in source_hashes:
            result = subprocess.run(
                ["git", "show", f"{revision}:{path}"],
                cwd=root,
                capture_output=True,
                check=False,
            )
            if result.returncode:
                failures.append(f"Pinned source unavailable: {path}")
                source_hashes[path] = None
            else:
                source_hashes[path] = hashlib.sha256(result.stdout).hexdigest()
        if source_hashes[path] != row["sha256"]:
            failures.append(f"Pinned source hash differs: {path}")

    local_links = 0
    markdown = sorted(audit.rglob("*.md"))
    for document in markdown:
        for target in re.findall(r"\[[^\]\n]*\]\(([^)\n]+)\)", document.read_text()):
            target = target.strip().strip("<>")
            parsed = urlsplit(target)
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue
            local_links += 1
            path = document.parent / unquote(parsed.path)
            if not path.exists():
                failures.append(f"Missing link in {document.name}: {target}")

    issue_map = (audit / "08-issue-map.md").read_text().split(
        "## Closed work that materially changes the assessment", 1
    )[0]
    mapped = set(re.findall(r"issues/(\d+)\)", issue_map))
    open_numbers = {row["number"] for row in issues if row["state"] == "open"}
    if mapped != open_numbers:
        failures.append(
            f"Open issue map differs: missing={sorted(open_numbers - mapped)}, "
            f"extra={sorted(mapped - open_numbers)}"
        )
    findings = []
    for document in markdown:
        findings += re.findall(
            r"^#{2,3} ((?:PRODUCT|RUN|LEARN|EVAL|OPS|DATA|CODE)-\d+)",
            document.read_text(),
            flags=re.MULTILINE,
        )
    if len(findings) != 58 or len(set(findings)) != 58:
        failures.append("Expected 58 distinct named analytical topics")
    actions = re.findall(
        r"^\| (A\d+)\.",
        (audit / "09-roadmap-and-acceptance.md").read_text(),
        flags=re.MULTILINE,
    )
    if actions != [f"A{number:02}" for number in range(1, 33)]:
        failures.append("Expected action register A01 through A32")
    for document in evidence.glob("*.json"):
        try:
            json.loads(document.read_text())
        except (ValueError, OSError) as error:
            failures.append(f"Invalid JSON {document.name}: {error}")

    result = {
        "result": "pass" if not failures else "fail",
        "source_revision": revision,
        **expected_counts,
        "reviewed_source_paths": len(source_hashes),
        "source_review_entries": len(reviewed),
        "markdown_documents": len(markdown),
        "local_link_targets_checked": local_links,
        "analytical_topics": len(findings),
        "proposed_actions": len(actions),
        "checks": [
            "CSV counts, uniqueness and required-reading coverage",
            "Requested byte/word totals",
            "Pinned source SHA-256 hashes using git show",
            "Local Markdown path targets (not fragment anchors or web URLs)",
            "All open issues represented in the issue map",
            "Named topics, ordered action IDs and JSON syntax",
        ],
        "failures": failures,
    }
    print(json.dumps(result, indent=2))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
