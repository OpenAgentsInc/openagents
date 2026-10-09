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
    web = {
        "name": "web",
        "image": args.web_image,
        "command": ["/bin/sh"],
        "args": ["-c", web_sh],
        "ports": [{"containerPort": 8080, "name": "http1"}],
        "env": [
            plain("PUBLIC_ORIGIN", ORIGIN),
            plain("ALT_HOST", ALT_HOST),
            plain("RUN_HOST", RUN_HOST),
            plain("CHAT_BUCKET", CHAT_BUCKET),
            plain("OPENAGENTS_WEB_CHAT_WORKER", args.worker_pubkey),
            secret("openagents-web-1-staging-github-oauth", "GITHUB_OAUTH_JSON"),
            secret("openagents-web-1-staging-csrf-key", "CSRF_KEY"),
            secret("openagents-web-1-staging-byo-keys", "OPENAGENTS_WEB_CLOUD_BYO_KEYS"),
            secret("openagents-web-1-staging-ask-salt", "OPENAGENTS_WEB_ASK_SALT"),
        ],
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
            secret("openagents-web-1-staging-github-oauth", "GITHUB_OAUTH_JSON"),
            secret("openagents-gateway-staging-admin-token", "INFERENCE_ADMIN_TOKEN"),
            secret("openagents-gateway-staging-smoke-signup-token", "SMOKE_SIGNUP_TOKEN"),
            secret("openagents-gateway-staging-vertex-sa", "VERTEX_SA_JSON"),
            secret("openagents-gateway-staging-openrouter-key", "OPENROUTER_API_KEY"),
            secret("openagents-gateway-staging-ai-gateway-key", "AI_GATEWAY_API_KEY"),
            secret("openagents-gateway-staging-typesafe-key", "TYPESAFE_API_KEY"),
        ],
        "volumeMounts": [stack],
        "resources": {"limits": {"cpu": "1", "memory": "512Mi"}},
        "startupProbe": {
            "httpGet": {"path": "/healthz", "port": 8791},
            "periodSeconds": 2,
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
                        "run.googleapis.com/container-dependencies": json.dumps(
                            {"web": ["gateway"], "worker": ["gateway"]}
                        ),
                    },
                },
                "spec": {
                    "serviceAccountName": RUNTIME,
                    "containerConcurrency": 80,
                    "timeoutSeconds": 3600,
                    "volumes": [
                        {"name": "stack", "emptyDir": {"medium": "Memory", "sizeLimit": "256Mi"}}
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
