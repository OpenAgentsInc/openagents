"""Render the staging Cloud Run service (openagents-web-1-staging) as JSON
for `gcloud run services replace`. Nothing secret is written: secrets are
Secret Manager references, resolved by Cloud Run at start.

    python3 deploy/staging/render.py --web-image IMAGE@sha256:... \
        --stack-image IMAGE@sha256:... --worker-pubkey HEX > service.json

See deploy/staging/README.md.
"""

import argparse
import json
import pathlib

SERVICE = "openagents-web-1-staging"
PROJECT_NUMBER = "157437760789"
# staging.openagents.com is a Cloud Run domain mapping to this service; the
# two run.app addresses keep answering too.
ORIGIN = "https://staging.openagents.com"
RUN_HOST = f"{SERVICE}-{PROJECT_NUMBER}.us-central1.run.app"
ALT_HOST = f"{SERVICE}-ezxz4mgdsq-uc.a.run.app"
# The automation account may not act as the default compute account (see
# docs/deployment/openagents-web.md), so staging runs as the inference account.
RUNTIME = "oa-vertex-inference@openagentsgemini.iam.gserviceaccount.com"
CHAT_BUCKET = "openagentsgemini-web-chats-stage"
ANALYTICS_BUCKET = "openagentsgemini-web-analytics-staging"
# The account-store NFS server (deploy/accounts-nfs,
# docs/deployment/account-storage.md) and the subnet the service reaches it
# through (Direct VPC egress, private ranges only).
NFS_SERVER = "10.42.26.2"
EGRESS_SUBNET = "openagents-web-staging"
# Invite-only sign-in (oa_auth::invite, docs/auth/github.md): only the
# owner's GitHub account (AtlantisPleb, id 14167547) may sign in, as a site
# admin. Both the gateway and the web server read it.
INVITE_ONLY = json.dumps(
    {"github": [{"id": 14167547, "login": "AtlantisPleb", "admin": True}]},
    separators=(",", ":"),
)


def secret(name, var):
    return {"name": var, "valueFrom": {"secretKeyRef": {"name": name, "key": "latest"}}}


def plain(var, value):
    return {"name": var, "value": value}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--web-image", required=True)
    parser.add_argument("--stack-image", required=True)
    parser.add_argument("--worker-pubkey", required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    here = pathlib.Path(__file__).resolve().parent
    web_sh = (here / "web.sh").read_text()
    stack = {"name": "stack", "mountPath": "/stack"}
    webstate = {"name": "webstate", "mountPath": "/state"}
    web = {
        "name": "web",
        "image": args.web_image,
        "command": ["/bin/sh"],
        "args": ["-c", web_sh],
        "ports": [{"containerPort": 8080, "name": "http1"}],
        "env": [
            plain("PUBLIC_ORIGIN", ORIGIN),
            plain("WEB_STATE", "/state"),
            plain("ALT_HOST", ALT_HOST),
            plain("RUN_HOST", RUN_HOST),
            plain("CHAT_BUCKET", CHAT_BUCKET),
            plain("OPENAGENTS_WEB_CHAT_WORKER", args.worker_pubkey),
            plain("INVITE_ONLY_JSON", INVITE_ONLY),
            # The smoke suite's operator test account (#11155): staging only.
            plain("OPENAGENTS_WEB_API_OPERATOR_SIGNUP", "1"),
            secret("openagents-web-1-staging-github-oauth", "GITHUB_OAUTH_JSON"),
            secret("openagents-web-1-staging-csrf-key", "CSRF_KEY"),
            secret("openagents-web-1-staging-byo-keys", "OPENAGENTS_WEB_CLOUD_BYO_KEYS"),
            secret("openagents-web-1-staging-ask-salt", "OPENAGENTS_WEB_ASK_SALT"),
            # First-party analytics (#11153, docs/deployment/analytics.md).
            plain("OPENAGENTS_WEB_ANALYTICS_BUCKET", ANALYTICS_BUCKET),
            secret("openagents-web-analytics-key-staging", "OPENAGENTS_WEB_ANALYTICS_KEY"),
        ],
        "volumeMounts": [webstate],
        "resources": {"limits": {"cpu": "1", "memory": "1Gi"}},
        "startupProbe": {
            # A TCP check: the site answers only its public hosts, so an
            # HTTP probe on the instance address is refused.
            "tcpSocket": {"port": 8080},
            "periodSeconds": 2,
            "failureThreshold": 60,
        },
    }
    gateway = {
        "name": "gateway",
        "image": args.stack_image,
        "command": ["/bin/sh"],
        "args": ["-c", (here / "gateway.sh").read_text()],
        "env": [
            plain("STACK_STATE", "/stack"),
            plain("PUBLIC_ORIGIN", ORIGIN),
            plain("INVITE_ONLY_JSON", INVITE_ONLY),
            secret("openagents-web-1-staging-github-oauth", "GITHUB_OAUTH_JSON"),
            secret("openagents-gateway-staging-admin-token", "INFERENCE_ADMIN_TOKEN"),
            secret("openagents-gateway-staging-smoke-signup-token", "SMOKE_SIGNUP_TOKEN"),
            secret("openagents-gateway-staging-vertex-sa", "VERTEX_SA_JSON"),
            secret("openagents-gateway-staging-openrouter-key", "OPENROUTER_API_KEY"),
            secret("openagents-gateway-staging-ai-gateway-key", "AI_GATEWAY_API_KEY"),
            secret("openagents-gateway-staging-typesafe-key", "TYPESAFE_API_KEY"),
            secret("openagents-gateway-staging-byok-keyring", "BYOK_KEYRING_JSON"),
            secret("openagents-gateway-staging-store-key", "INFERENCE_STORE_KEY"),
        ],
        "volumeMounts": [stack],
        "resources": {"limits": {"cpu": "1", "memory": "512Mi"}},
        # Up to 240 s: a new instance waits (at most 150 s) for the old
        # one to hand over the store before its gateway starts.
        "startupProbe": {
            "httpGet": {"path": "/healthz", "port": 8791},
            "periodSeconds": 4,
            "failureThreshold": 60,
        },
    }
    worker = {
        "name": "worker",
        "image": args.stack_image,
        "command": ["/bin/sh"],
        "args": ["-c", (here / "worker.sh").read_text()],
        "env": [
            plain("STACK_STATE", "/stack"),
            plain("OPENAGENTS_PRODUCT_KB_EMBEDDINGS", "gateway"),
            secret("openagents-chat-worker-staging-secret", "CODER_WORKER_SECRET"),
            secret("openagents-gateway-staging-ai-gateway-key", "CODER_AI_GATEWAY_KEY"),
            secret("openagents-gateway-staging-typesafe-key", "TYPESAFE_API_KEY"),
        ],
        "volumeMounts": [stack],
        "resources": {"limits": {"cpu": "1", "memory": "512Mi"}},
    }
    service = {
        "apiVersion": "serving.knative.dev/v1",
        "kind": "Service",
        "metadata": {
            "name": SERVICE,
            "labels": {"env": "staging", "issue": "11094"},
            "annotations": {"run.googleapis.com/ingress": "all"},
        },
        "spec": {
            "template": {
                "metadata": {
                    "name": args.revision,
                    "annotations": {
                        "autoscaling.knative.dev/minScale": "1",
                        "autoscaling.knative.dev/maxScale": "1",
                        "run.googleapis.com/cpu-throttling": "false",
                        "run.googleapis.com/startup-cpu-boost": "true",
                        "run.googleapis.com/execution-environment": "gen2",
                        "run.googleapis.com/network-interfaces": json.dumps(
                            [{"network": "default", "subnetwork": EGRESS_SUBNET}]
                        ),
                        "run.googleapis.com/vpc-access-egress": "private-ranges-only",
                        "run.googleapis.com/container-dependencies": json.dumps(
                            {"web": ["gateway"], "worker": ["gateway"]}
                        ),
                    },
                },
                "spec": {
                    "serviceAccountName": RUNTIME,
                    "containerConcurrency": 80,
                    "timeoutSeconds": 3600,
                    # Durable: accounts, sessions, API keys, sealed GitHub
                    # tokens, saved provider keys, saved own-Claude keys.
                    "volumes": [
                        {"name": "stack", "nfs": {"server": NFS_SERVER, "path": "/srv/accounts/stack"}},
                        {"name": "webstate", "nfs": {"server": NFS_SERVER, "path": "/srv/accounts/web"}},
                    ],
                    "containers": [web, gateway, worker],
                },
            },
            "traffic": [{"latestRevision": True, "percent": 100}],
        },
    }
    print(json.dumps(service, indent=2))


if __name__ == "__main__":
    main()
