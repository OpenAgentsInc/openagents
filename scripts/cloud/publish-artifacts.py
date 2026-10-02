#!/usr/bin/env python3
"""Upload an explicitly selected cloud run bundle and comment expiring links."""
import argparse
import json
import pathlib
import re
import subprocess


def run(*args):
    return subprocess.check_output(args, text=True, timeout=120)


def publish(directory, bucket, issue, repository):
    if not re.fullmatch(r"gs://[a-z0-9][a-z0-9._-]+", bucket):
        raise ValueError("Use a GCS bucket without an object path.")
    directory = pathlib.Path(directory)
    if not re.fullmatch(r"[a-zA-Z0-9_-]+", directory.name):
        raise ValueError("Use a run ID with letters, numbers, underscores, or hyphens.")
    links = []
    for name in ("stdout.log", "stderr.log", "change.patch", "evidence.json"):
        source = directory / name
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"Missing regular artifact: {name}")
        target = f"{bucket}/{directory.name}/{name}"
        run("gcloud", "storage", "cp", str(source), target)
        signed = json.loads(run("gcloud", "storage", "sign-url", target,
                                "--duration=1d", "--format=json"))
        url = signed[0]["signed_url"]
        if not url.startswith("https://") or any(c in url for c in "\r\n<> "):
            raise ValueError("The storage signer returned an invalid URL.")
        links.append(f"- [{name}](<{url}>)")
    comment = "Cloud run artifacts (links expire after 24 hours):\n\n" + "\n".join(links)
    # Send bearer links through stdin, not process arguments or console output.
    subprocess.run(["gh", "issue", "comment", str(issue), "--repo", repository,
                    "--body-file", "-"], input=comment, text=True, check=True, timeout=120)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory")
    parser.add_argument("--bucket", required=True)
    parser.add_argument("--issue", required=True, type=int)
    parser.add_argument("--repo", default="OpenAgentsInc/openagents")
    args = parser.parse_args()
    publish(args.directory, args.bucket, args.issue, args.repo)
