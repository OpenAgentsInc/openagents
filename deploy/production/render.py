"""Render the production service `coder` (openagents.com) with the account
service: the live serving revision's spec plus the `gateway` sidecar, the
account-store NFS volumes, and the web container started by
deploy/production/web.sh. The result is a new revision with no traffic at
the tag `new`, for `gcloud run services replace`. Nothing secret is
written: secrets are Secret Manager references.

    python3 deploy/production/render.py --service service.json \\
        --revision-spec revision.json --stack-image IMAGE@sha256:... \\
        [--web-image IMAGE@sha256:...] --name REVISION > spec.json

Running it on a spec that already has the gateway refreshes the launchers,
the environment and the images; the web container's own arguments stay as
they are. See docs/deployment/account-storage.md (Production).
"""

import argparse
import json
import pathlib

ORIGIN = "https://openagents.com"
# deploy/accounts-nfs/provision.sh production.
NFS_SERVER = "10.42.27.2"
EGRESS_SUBNET = "openagents-web-production"
# The account database (#11154, docs/data/schema.md): Cloud SQL through the
# connector's socket, its connection string in Secret Manager. --files
# leaves it out (the stores stay on the NFS share, the rollback).
DATABASE = "openagentsgemini:us-central1:openagents-production-pg"
DATABASE_SECRET = "openagents-production-pg-dsn"
# GitHub sign-in is open to anyone with a GitHub account (since 2026-10-15);
# the list only marks site admins: the owner (GitHub AtlantisPleb, id
# 14167547) (oa_auth::invite, docs/auth/github.md).
INVITE_ONLY = json.dumps(
    {"open": True, "github": [{"id": 14167547, "login": "AtlantisPleb", "admin": True}]},
    separators=(",", ":"),
)
# The retired coder-serve sidecar's own database
# (docs/deployment/coder-serve-retirement.md).
RETIRED_DATABASE = "openagentsgemini:us-central1:coder-pg"
SYSTEM = ("serving.knative.dev/", "client.knative.dev/", "run.googleapis.com/operation-id",
          "run.googleapis.com/ingress-status", "run.googleapis.com/urls",
          "run.googleapis.com/creator", "run.googleapis.com/lastModifier")


def secret(name, var):
    return {"name": var, "valueFrom": {"secretKeyRef": {"name": name, "key": "latest"}}}


def plain(var, value):
    return {"name": var, "value": value}


def keep(entries, drop):
    return {k: v for k, v in (entries or {}).items() if not k.startswith(drop)}


def set_env(container, entries):
    names = {e["name"] for e in entries}
    container["env"] = [e for e in container.get("env", []) if e["name"] not in names] + entries


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--service", required=True, help="gcloud run services describe coder --format=json")
    parser.add_argument("--revision-spec", required=True,
                        help="gcloud run revisions describe SERVING --format=json")
    parser.add_argument("--stack-image", required=True)
    parser.add_argument("--web-image")
    parser.add_argument("--name", required=True)
    parser.add_argument("--files", action="store_true",
                        help="keep the account stores on the NFS share, not the database")
    args = parser.parse_args()
    here = pathlib.Path(__file__).resolve().parent
    service = json.load(open(args.service))
    revision = json.load(open(args.revision_spec))
    spec = revision["spec"]
    containers = spec["containers"]

    web = next(c for c in containers if c["name"] == "web")
    if args.web_image:
        web["image"] = args.web_image
    if web.get("command") == ["/bin/sh"]:
        site_args = web["args"][3:]  # after "-c", the launcher, "$0"
    else:
        site_args = web.get("args", [])
    web["command"] = ["/bin/sh"]
    web["args"] = ["-c", (here / "web.sh").read_text(), "openagents-web", *site_args]
    set_env(web, [
        plain("PUBLIC_ORIGIN", ORIGIN),
        plain("WEB_STATE", "/state"),
        plain("INVITE_ONLY_JSON", INVITE_ONLY),
        secret("openagents-web-production-github-oauth", "GITHUB_OAUTH_JSON"),
        secret("openagents-web-production-csrf-key", "CSRF_KEY"),
        secret("openagents-web-production-byo-keys", "OPENAGENTS_WEB_CLOUD_BYO_KEYS"),
        # First-party analytics (#11153, docs/deployment/analytics.md).
        plain("OPENAGENTS_WEB_ANALYTICS_BUCKET", "openagentsgemini-web-analytics-prod"),
        secret("openagents-web-analytics-key", "OPENAGENTS_WEB_ANALYTICS_KEY"),
        # Agent work (#11162, docs/deployment/agent-work.md): oa-boat machines (#11256),
        # and the setup agent's model through the gateway on the house key.
        plain("STACK_STATE", "/stack"),
        plain("ENVIRONMENTS_MODEL", "google/gemini-3.8-flash"),
        # Our own Boat-compatible service on GCE (#11256), not boat.dev.
        secret("oa-boat-api-key", "BOAT_API_KEY"),
        plain("BOAT_API_BASE", "https://oa-boat-157437760789.us-central1.run.app/api/v1"),
        # Gemini on Vertex for the chat's images and PDFs (#11221).
        secret("openagents-vertex-sa-key", "VERTEX_SA_JSON"),
        # Work hosts that take "work on this issue" runs (#11258).
        secret("openagents-work-host-token", "OPENAGENTS_WORK_HOST_TOKEN"),
        # Admit the dedicated production test account, preserving the general gate.
        plain("OPENAGENTS_WEB_AGENT_ACCOUNTS", "acct_83d16a35955ef908"),
        # Chats on their own computers through oa-computers (#11624).
        plain("OPENAGENTS_WEB_COMPUTERS_URL", "https://oa-computers-157437760789.us-central1.run.app"),
        secret("oa-computers-token", "OPENAGENTS_WEB_COMPUTERS_TOKEN"),
        # The production test account (#11601): the site forwards the
        # operator's sign-up so checks never run as the owner.
        plain("OPENAGENTS_WEB_API_OPERATOR_SIGNUP", "1"),
        # CAD Lab is part of Pro (#11693): the test account passes, and
        # checks the Free side with X-OpenAgents-Test-Plan: free.
        plain("OPENAGENTS_WEB_PRO_TEST_ACCOUNTS", "acct_83d16a35955ef908"),
    ] + ([] if args.files else [
        # The actor runtime (#11253, docs/deployment/actors.md) in the
        # account database. Mac jobs go through it only while the live
        # spec's OPENAGENTS_WEB_MAC_JOBS_ACTORS is 1 (set_env keeps it).
        secret(DATABASE_SECRET, "OPENAGENTS_WEB_ACTORS_DATABASE_URL"),
    ]))
    # The gateway's store read-only: the web reads only its house key.
    web["volumeMounts"] = [
        {"name": "webstate", "mountPath": "/state"},
        {"name": "stack", "mountPath": "/stack", "readOnly": True},
    ]
    # No coder-serve sidecar (docs/deployment/coder-serve-retirement.md):
    # the site answers /v1 from the gateway, the keyed /mcp itself, and
    # every other path the old server had with 404.
    web["env"] = [e for e in web.get("env", []) if e["name"] != "OPENAGENTS_WEB_UPSTREAM"]
    containers = [c for c in containers if c["name"] != "coder-serve"]

    gateway = {
        "name": "gateway",
        "image": args.stack_image,
        "command": ["/bin/sh"],
        # The staging launcher, in GATEWAY_HOLD=serving mode: a no-traffic
        # candidate never takes the account store from the serving revision.
        "args": ["-c", (here.parent / "staging" / "gateway.sh").read_text()],
        "env": [
            plain("STACK_STATE", "/stack"),
            plain("PUBLIC_ORIGIN", ORIGIN),
            plain("GATEWAY_HOLD", "serving"),
            plain("INVITE_ONLY_JSON", INVITE_ONLY),
            secret("openagents-web-production-github-oauth", "GITHUB_OAUTH_JSON"),
            secret("openagents-gateway-production-admin-token", "INFERENCE_ADMIN_TOKEN"),
            secret("openagents-vertex-sa-key", "VERTEX_SA_JSON"),
            secret("openagents-openrouter-api-key", "OPENROUTER_API_KEY"),
            secret("openagents-vercel-gateway-api-key", "AI_GATEWAY_API_KEY"),
            secret("openagents-gateway-production-byok-keyring", "BYOK_KEYRING_JSON"),
            secret("openagents-gateway-production-store-key", "INFERENCE_STORE_KEY"),
            # The production test account's operator sign-up (#11601).
            secret("openagents-gateway-production-smoke-signup-token", "SMOKE_SIGNUP_TOKEN"),
        ] + ([] if args.files else [secret(DATABASE_SECRET, "OPENAGENTS_ACCOUNTS_DATABASE_URL")]),
        "volumeMounts": [{"name": "stack", "mountPath": "/stack"}],
        "resources": {"limits": {"cpu": "1", "memory": "512Mi"}},
        # No startup probe and no dependency: the gateway waits for its
        # revision to serve traffic, and the site answers meanwhile.
    }
    spec["containers"] = [c for c in containers if c["name"] != "gateway"] + [gateway]
    spec["volumes"] = [
        {"name": "stack", "nfs": {"server": NFS_SERVER, "path": "/srv/accounts/stack"}},
        {"name": "webstate", "nfs": {"server": NFS_SERVER, "path": "/srv/accounts/web"}},
    ]

    annotations = keep(revision["metadata"].get("annotations"), SYSTEM)
    annotations.update({
        # One writer on the account store (docs/deployment/account-storage.md).
        "autoscaling.knative.dev/minScale": "1",
        "autoscaling.knative.dev/maxScale": "1",
        "run.googleapis.com/execution-environment": "gen2",
        "run.googleapis.com/network-interfaces": json.dumps(
            [{"network": "default", "subnetwork": EGRESS_SUBNET}]),
        "run.googleapis.com/vpc-access-egress": "private-ranges-only",
    })
    annotations.pop("run.googleapis.com/container-dependencies", None)
    # Add the account database to the list, never replace it; the retired
    # sidecar's coder-pg goes.
    instances = [
        name for name in annotations.get("run.googleapis.com/cloudsql-instances", "").split(",")
        if name.strip() and name.strip() not in (DATABASE, RETIRED_DATABASE)
    ]
    if not args.files:
        instances.append(DATABASE)
    if instances:
        annotations["run.googleapis.com/cloudsql-instances"] = ",".join(instances)
    else:
        annotations.pop("run.googleapis.com/cloudsql-instances", None)

    traffic = []
    for entry in service["spec"].get("traffic", []):
        if entry.get("latestRevision"):
            raise SystemExit("refusing: a traffic entry follows the latest revision")
        entry = dict(entry)
        if entry.get("tag") == "new":
            del entry["tag"]
            if not entry.get("percent"):
                continue
        traffic.append(entry)
    traffic.append({"revisionName": args.name, "tag": "new"})
    meta = service["metadata"]
    out = {
        "apiVersion": "serving.knative.dev/v1",
        "kind": "Service",
        "metadata": {
            "name": meta["name"],
            "namespace": meta["namespace"],
            "labels": keep(meta.get("labels"), SYSTEM),
            "annotations": keep(meta.get("annotations"), SYSTEM),
        },
        "spec": {
            "template": {
                "metadata": {
                    "name": args.name,
                    "labels": keep(revision["metadata"].get("labels"), SYSTEM + ("cloud.googleapis.com/",)),
                    "annotations": annotations,
                },
                "spec": spec,
            },
            "traffic": traffic,
        },
    }
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
