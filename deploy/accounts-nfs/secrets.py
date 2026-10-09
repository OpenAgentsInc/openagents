"""Make the gateway's two at-rest keys for one environment in Secret Manager
and let the service's runtime account read them
(docs/deployment/account-storage.md). Nothing is printed but the names.

    python3 deploy/accounts-nfs/secrets.py staging
    python3 deploy/accounts-nfs/secrets.py production --runtime ACCOUNT_EMAIL \
        --web ~/work/.secrets/github-oauth-production.json

- openagents-gateway-<env>-byok-keyring: the oa-seal keyring that seals
  workspaces' own provider keys (gateway `inference.byok.keyring`).
- openagents-gateway-<env>-store-key: base64 of 32 bytes, the key that
  seals stored responses and compaction items (INFERENCE_STORE_KEY).

With --web GITHUB_OAUTH_FILE, also the web stack's own secrets (staging
has the same set under openagents-web-1-staging-*):

- openagents-web-<env>-github-oauth: the OAuth App's private file, as given.
- openagents-web-<env>-csrf-key: 64 hex characters, the web's CSRF key.
- openagents-web-<env>-byo-keys: the oa-seal keyring that seals saved
  own-Claude credentials (OPENAGENTS_WEB_CLOUD_BYO_KEYS).
- openagents-gateway-<env>-admin-token: the /admin/inference bearer.

A secret that already exists is left as it is: replacing either key makes
everything sealed under it unreadable. Run with the automation account
(CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config).
"""

import argparse
import base64
import json
import os
import subprocess

PROJECT = "openagentsgemini"


def gcloud(*args, data=None):
    return subprocess.run(["gcloud", *args, "--project", PROJECT], input=data,
                          capture_output=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("env", choices=["staging", "production"])
    parser.add_argument("--runtime",
                        default="oa-vertex-inference@openagentsgemini.iam.gserviceaccount.com")
    parser.add_argument("--web", metavar="GITHUB_OAUTH_FILE")
    args = parser.parse_args()
    key_id = f"{args.env}-1"
    values = {
        f"openagents-gateway-{args.env}-byok-keyring": json.dumps({
            "schema": "openagents.seal.keyring.v1",
            "current": key_id,
            "keys": {key_id: base64.b64encode(os.urandom(32)).decode()},
        }),
        f"openagents-gateway-{args.env}-store-key": base64.b64encode(os.urandom(32)).decode(),
    }
    if args.web:
        with open(os.path.expanduser(args.web)) as f:
            oauth = f.read()
        if sorted(json.loads(oauth)) != ["client_id", "client_secret", "token_encryption_key"]:
            raise SystemExit("--web: not an OAuth App private file")
        values.update({
            f"openagents-web-{args.env}-github-oauth": oauth,
            f"openagents-web-{args.env}-csrf-key": os.urandom(32).hex(),
            f"openagents-web-{args.env}-byo-keys": json.dumps({
                "schema": "openagents.seal.keyring.v1",
                "current": key_id,
                "keys": {key_id: base64.b64encode(os.urandom(32)).decode()},
            }),
            f"openagents-gateway-{args.env}-admin-token":
                base64.urlsafe_b64encode(os.urandom(32)).decode().rstrip("="),
        })
    for name, value in values.items():
        exists = gcloud("secrets", "describe", name).returncode == 0
        if not exists:
            made = gcloud("secrets", "create", name, "--replication-policy", "automatic",
                          "--labels", f"env={args.env},issue=11127", "--data-file=-",
                          data=value.encode())
            if made.returncode:
                raise SystemExit(f"{name}: {made.stderr.decode().strip().splitlines()[-1]}")
        granted = gcloud("secrets", "add-iam-policy-binding", name,
                         "--member", f"serviceAccount:{args.runtime}",
                         "--role", "roles/secretmanager.secretAccessor")
        if granted.returncode:
            raise SystemExit(f"{name}: {granted.stderr.decode().strip().splitlines()[-1]}")
        print(f"{name}: {'kept' if exists else 'made'}, readable by {args.runtime}")


if __name__ == "__main__":
    main()
