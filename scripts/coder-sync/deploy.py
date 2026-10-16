#!/usr/bin/env python3
"""Provision and deploy coder-sync using the automation Google Cloud config."""
import argparse
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request

PROJECT = "openagentsgemini"
REGION = "us-central1"
INSTANCE = "coder-sync-pg"
ACCOUNT = f"coder-sync@{PROJECT}.iam.gserviceaccount.com"
BUILD_ACCOUNT = f"projects/{PROJECT}/serviceAccounts/oa-mvp-automation@{PROJECT}.iam.gserviceaccount.com"
ROOT = Path(__file__).resolve().parents[2]
os.environ.setdefault("CLOUDSDK_CONFIG", "/Users/christopherdavid/work/.secrets/gcloud-sa-config")
os.environ.update(BROWSER="/usr/bin/true", NO_OPEN_BROWSER="1")


def g(*args, optional=False):
    result = subprocess.run(["gcloud", *args, "--project", PROJECT, "--quiet"], text=True, capture_output=True)
    if result.returncode:
        if optional and ("NOT_FOUND" in result.stderr or "does not exist" in result.stderr or "not found" in result.stderr.lower() or "404" in result.stderr):
            return None
        # Arguments never include database passwords or account tokens.
        sys.stderr.write(result.stderr)
        raise RuntimeError("Google Cloud command failed: " + " ".join(args[:3]))
    return result.stdout.strip()


def sql_user(name, password):
    # Send the password in the HTTPS body, never argv, a log, or a source file.
    token = g("auth", "print-access-token")
    data = json.dumps({"name": name, "password": password}).encode()
    request = urllib.request.Request(
        f"https://sqladmin.googleapis.com/sql/v1beta4/projects/{PROJECT}/instances/{INSTANCE}/users",
        data=data, headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(request, timeout=60) as response:
        operation = json.load(response)["name"]
    g("sql", "operations", "wait", operation, "--timeout=300")


def lifecycle():
    for suffix in ("-staging", ""):
        g("storage", "buckets", "update", f"gs://{PROJECT}-coder-sync{suffix}",
          f"--lifecycle-file={ROOT / 'deploy/coder-sync/lifecycle.json'}")
        print(f"Lifecycle ready: coder-sync{suffix}")


def provision():
    if g("iam", "service-accounts", "describe", ACCOUNT, "--format=value(email)", optional=True) is None:
        g("iam", "service-accounts", "create", "coder-sync", "--display-name=Coder sync")
    try:
        g("projects", "add-iam-policy-binding", PROJECT, f"--member=serviceAccount:{ACCOUNT}", "--role=roles/cloudsql.client", "--condition=None")
    except RuntimeError:
        print(f"Owner step: gcloud projects add-iam-policy-binding {PROJECT} --member=serviceAccount:{ACCOUNT} --role=roles/cloudsql.client --condition=None", flush=True)

    instance = g("sql", "instances", "describe", INSTANCE, "--format=value(state)", optional=True)
    if instance is None:
        g("sql", "instances", "create", INSTANCE, "--database-version=POSTGRES_17", "--edition=ENTERPRISE", "--tier=db-f1-micro",
          f"--region={REGION}", "--storage-size=10GB", "--storage-type=SSD", "--availability-type=ZONAL", "--async")
        print("Cloud SQL creation started. Run provision again after it becomes RUNNABLE.")
        return
    if instance != "RUNNABLE":
        raise RuntimeError("Cloud SQL is still starting; run provision again when it is RUNNABLE.")
    for staging in (True, False):
        suffix = "-staging" if staging else ""
        database = "coder_sync_staging" if staging else "coder_sync"
        bucket = f"{PROJECT}-coder-sync{suffix}"
        secret = f"coder-sync{suffix}-database-url"
        if g("sql", "databases", "describe", database, f"--instance={INSTANCE}", optional=True) is None:
            g("sql", "databases", "create", database, f"--instance={INSTANCE}")
        if g("storage", "buckets", "describe", f"gs://{bucket}", optional=True) is None:
            g("storage", "buckets", "create", f"gs://{bucket}", f"--location={REGION}", "--uniform-bucket-level-access", "--public-access-prevention")
        g("storage", "buckets", "add-iam-policy-binding", f"gs://{bucket}", f"--member=serviceAccount:{ACCOUNT}", "--role=roles/storage.objectAdmin")
        if g("secrets", "describe", secret, optional=True) is None:
            g("secrets", "create", secret, "--replication-policy=automatic")
        versions = json.loads(g("secrets", "versions", "list", secret, "--filter=state:ENABLED", "--format=json(name)"))
        if not versions:
            password = secrets.token_urlsafe(40)
            sql_user(database, password)
            dsn = f"host=/cloudsql/{PROJECT}:{REGION}:{INSTANCE} user={database} dbname={database} password={password}"
            subprocess.run(["gcloud", "secrets", "versions", "add", secret, "--data-file=-", "--project", PROJECT, "--quiet"],
                           input=dsn, text=True, stdout=subprocess.DEVNULL, check=True)
        g("secrets", "add-iam-policy-binding", secret, f"--member=serviceAccount:{ACCOUNT}", "--role=roles/secretmanager.secretAccessor")
        print(f"Ready: {database}, gs://{bucket}, {secret}")
    lifecycle()


def revision():
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise RuntimeError("Commit changes before building or deploying.")
    subprocess.run(["git", "fetch", "origin", "main"], cwd=ROOT, check=True)
    if subprocess.run(["git", "merge-base", "--is-ancestor", "origin/main", "HEAD"], cwd=ROOT).returncode:
        raise RuntimeError("Rebase on origin/main before building or deploying.")
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()


def build():
    commit = revision()
    image = f"us-central1-docker.pkg.dev/{PROJECT}/openagents/coder-sync"
    published = g("artifacts", "docker", "images", "describe", f"{image}:{commit}", "--format=value(image_summary.digest)", optional=True)
    if published:
        print(f"Already published: {image}@{published}")
        return
    versions = json.loads(g("artifacts", "docker", "images", "list", image, "--include-tags", "--format=json"))
    builders = [v for v in versions if any(t == "builder" or t.startswith("builder-") for t in v.get("tags", []))]
    cache = "rust:1.98.1-slim-trixie"
    if builders:
        cache = f"{image}@{max(builders, key=lambda v: v['createTime'])['version']}"
    scratch = os.environ.get("OPENAGENTS_SCRATCH") or subprocess.check_output(["openagents", "scratch"], text=True).strip()
    context = Path(tempfile.mkdtemp(prefix="coder-sync-build-", dir=scratch))
    try:
        archive = subprocess.Popen(["git", "archive", commit, "crates/coder-sync", "Cargo.lock", "deploy/coder-sync"], cwd=ROOT, stdout=subprocess.PIPE)
        subprocess.run(["tar", "-x", "-C", str(context)], stdin=archive.stdout, check=True)
        archive.stdout.close()
        if archive.wait():
            raise RuntimeError("Could not export the committed build.")
        print(g("builds", "submit", str(context), "--config", str(context / "deploy/coder-sync/cloudbuild.yaml"),
                f"--service-account={BUILD_ACCOUNT}", f"--substitutions=_TAG={commit},_CACHE_IMAGE={cache}", "--async", "--format=value(id)"))
    finally:
        shutil.rmtree(context)


def deploy(staging):
    commit = revision()
    suffix = "-staging" if staging else ""
    service = f"coder-sync{suffix}"
    args = ["run", "deploy", service, f"--region={REGION}", f"--image=us-central1-docker.pkg.dev/{PROJECT}/openagents/coder-sync:{commit}",
            f"--service-account={ACCOUNT}", "--max=1", "--min=0", "--max-instances=1", "--min-instances=0", "--timeout=3600", "--concurrency=80", "--cpu=1", "--memory=512Mi", "--allow-unauthenticated",
            f"--add-cloudsql-instances={PROJECT}:{REGION}:{INSTANCE}",
            f"--set-secrets=CODER_SYNC_DATABASE_URL=coder-sync{suffix}-database-url:latest",
            f"--set-env-vars=CODER_SYNC_ENV={'staging' if staging else 'production'},CODER_SYNC_AUTH=openagents,CODER_SYNC_BUCKET={PROJECT}-coder-sync{suffix},CODER_SYNC_REVISION={commit}",
            "--args="]
    g(*args)
    print(g("run", "services", "describe", service, f"--region={REGION}", "--format=value(status.url)"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["provision", "build", "deploy", "domain", "lifecycle"])
    parser.add_argument("--staging", action="store_true")
    args = parser.parse_args()
    if args.command == "lifecycle":
        lifecycle()
    elif args.command == "provision": provision()
    elif args.command == "build": build()
    elif args.command == "deploy": deploy(args.staging)
    else:
        print(g("beta", "run", "domain-mappings", "create", "--service=coder-sync", "--domain=sync.openagents.com", f"--region={REGION}"))


if __name__ == "__main__":
    try: main()
    except RuntimeError as error:
        print(f"Deployment stopped: {error}", file=sys.stderr)
        sys.exit(1)
    except Exception as error:
        # SQL and HTTP exceptions can contain response bodies; keep credentials out.
        print(f"Deployment stopped ({type(error).__name__}). Check the preceding command result.", file=sys.stderr)
        sys.exit(1)
