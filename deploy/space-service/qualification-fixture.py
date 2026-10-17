#!/usr/bin/env python3
"""Prepare credentials for the isolated native service qualification account."""
import json
import secrets
from broker import cloud, PROJECT, PREFIX, LABELS


def main():
    name = PREFIX + "-qualification"
    existing = cloud("secrets", "describe", name, missing=True)
    if existing is None:
        value = {"schema": "openagents.space.qualification.v1", "org_id": "00000000-0000-4000-8000-000000011814",
            "user_id": "00000000-0000-4000-8000-000000021814", "key_id": "00000000-0000-4000-8000-000000031814",
            "session_id": "00000000-0000-4000-8000-000000041814", "api_key": secrets.token_urlsafe(48),
            "admin_session": secrets.token_hex(32), "computers_token": secrets.token_urlsafe(48)}
        cloud("secrets", "create", name, "--replication-policy=automatic", f"--labels={LABELS}", "--data-file=-", data=json.dumps(value).encode())
    elif existing.get("labels", {}).get("openagents-issue") != "11814":
        raise RuntimeError("The qualification credential belongs to another deployment.")
    cloud("secrets", "add-iam-policy-binding", name, f"--member=serviceAccount:{PREFIX}-control@{PROJECT}.iam.gserviceaccount.com", "--role=roles/secretmanager.secretAccessor")
    print(json.dumps({"secret": name, "version": 1, "owner_account_used": False}))


if __name__ == "__main__":
    main()
