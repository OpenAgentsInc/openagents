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
# Invite-only sign-in: only the owner (GitHub AtlantisPleb, id 14167547),
# as a site admin (docs/auth/github.md).
INVITE_ONLY = json.dumps(
    {"github": [{"id": 14167547, "login": "AtlantisPleb", "admin": True}]},
    separators=(",", ":"),
)
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
    ])
    web["volumeMounts"] = [{"name": "webstate", "mountPath": "/state"}]

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
        ],
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
        "run.googleapis.com/container-dependencies": json.dumps({"web": ["coder-serve"]}),
    })

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
