#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${OPENAGENTS_BASE_URL:-https://staging.openagents.com}"
ADMIN_EMAIL="${OA_SMOKE_ADMIN_EMAIL:-chris@openagents.com}"
ALLOW_NON_STAGING="${OA_SMOKE_ALLOW_NON_STAGING:-0}"

if [[ "${ALLOW_NON_STAGING}" != "1" ]]; then
  if [[ "${BASE_URL}" != *"staging"* ]]; then
    echo "refusing to run paywall smoke against non-staging base URL: ${BASE_URL}" >&2
    echo "set OA_SMOKE_ALLOW_NON_STAGING=1 to override" >&2
    exit 2
  fi
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required" >&2
  exit 2
fi

python3 - "${BASE_URL}" "${ADMIN_EMAIL}" <<'PY'
import json
import sys
from datetime import datetime, timezone

try:
    import requests
except Exception as exc:
    print(f"python dependency missing: {exc}", file=sys.stderr)
    sys.exit(2)

base = sys.argv[1].rstrip('/')
admin_email = sys.argv[2].strip().lower()
run = datetime.now(timezone.utc).strftime("%Y%m%dt%H%M%Sz").lower()

session = requests.Session()
result = {"run": run, "base": base, "adminEmail": admin_email, "ok": False, "steps": {}, "users": {}}


def ensure(resp, context, ok_status=(200, 201)):
    if resp.status_code not in ok_status:
        raise RuntimeError(f"{context} failed status={resp.status_code} body={resp.text[:500]}")


def register(email, name, token_name, create_autopilot=False):
    resp = session.post(
        f"{base}/api/auth/register",
        json={
            "email": email,
            "name": name,
            "tokenName": token_name,
            "createAutopilot": create_autopilot,
        },
        timeout=60,
    )
    ensure(resp, f"register {email}")
    data = resp.json()["data"]
    return data["token"], data["user"]


def headers(token):
    return {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}


try:
    admin_token, admin_user = register(admin_email, "Smoke Admin", f"smoke-paywall-admin-{run}", False)
    consumer_email = f"smoke-paywall-consumer-{run}@openagents.com"
    consumer_token, consumer_user = register(consumer_email, "Smoke Paywall Consumer", f"smoke-paywall-consumer-{run}", True)

    result["users"]["admin"] = {
        "id": admin_user.get("id"),
        "email": admin_user.get("email"),
        "handle": admin_user.get("handle"),
    }
    result["users"]["consumer"] = {
        "id": consumer_user.get("id"),
        "email": consumer_user.get("email"),
        "handle": consumer_user.get("handle"),
    }

    # Consumer can read but not mutate.
    consumer_read = session.get(f"{base}/api/l402/paywalls", headers={"Authorization": f"Bearer {consumer_token}"}, timeout=30)
    ensure(consumer_read, "consumer read paywalls")
    result["steps"]["consumerReadPaywalls"] = consumer_read.status_code

    payload = {
        "name": f"Smoke Paywall {run}",
        "hostRegexp": r"^l402\\.openagents\\.com$",
        "pathRegexp": f"^/smoke-paywall-{run}(?:/.*)?$",
        "priceMsats": 21000,
        "upstream": "https://example.com",
        "enabled": True,
        "metadata": {"run": run, "suite": "smoke-paywall-e2e"},
    }

    forbidden_create = session.post(f"{base}/api/l402/paywalls", headers=headers(consumer_token), json=payload, timeout=60)
    if forbidden_create.status_code != 403:
        raise RuntimeError(f"consumer create expected 403, got {forbidden_create.status_code} {forbidden_create.text[:300]}")
    result["steps"]["consumerCreateForbidden"] = True

    create = session.post(f"{base}/api/l402/paywalls", headers=headers(admin_token), json=payload, timeout=120)
    ensure(create, "admin create paywall", (201,))
    created = create.json()["data"]
    paywall = created["paywall"]
    paywall_id = paywall["id"]

    result["steps"]["adminCreate"] = {
        "status": create.status_code,
        "paywallId": paywall_id,
        "name": paywall.get("name"),
        "pathRegexp": paywall.get("pathRegexp"),
        "deploymentMode": (created.get("deployment") or {}).get("mode"),
        "mutationEventId": created.get("mutationEventId"),
    }

    forbidden_update = session.patch(f"{base}/api/l402/paywalls/{paywall_id}", headers=headers(consumer_token), json={"priceMsats": 22000}, timeout=60)
    if forbidden_update.status_code != 403:
        raise RuntimeError(f"consumer update expected 403, got {forbidden_update.status_code} {forbidden_update.text[:300]}")
    result["steps"]["consumerUpdateForbidden"] = True

    update = session.patch(
        f"{base}/api/l402/paywalls/{paywall_id}",
        headers=headers(admin_token),
        json={"priceMsats": 25000, "enabled": False, "metadata": {"run": run, "updated": True}},
        timeout=120,
    )
    ensure(update, "admin update paywall")
    updated = update.json()["data"]

    result["steps"]["adminUpdate"] = {
        "status": update.status_code,
        "priceMsats": (updated.get("paywall") or {}).get("priceMsats"),
        "enabled": (updated.get("paywall") or {}).get("enabled"),
        "deploymentMode": (updated.get("deployment") or {}).get("mode"),
        "mutationEventId": updated.get("mutationEventId"),
    }

    deployments = session.get(f"{base}/api/l402/deployments", headers={"Authorization": f"Bearer {admin_token}"}, timeout=60)
    ensure(deployments, "admin deployments")
    events = ((deployments.json().get("data") or {}).get("deployments") or [])
    event_types = [e.get("type") for e in events]

    result["steps"]["deploymentEvents"] = {
        "count": len(events),
        "hasCreated": "l402_paywall_created" in event_types,
        "hasUpdated": "l402_paywall_updated" in event_types,
    }

    forbidden_delete = session.delete(f"{base}/api/l402/paywalls/{paywall_id}", headers={"Authorization": f"Bearer {consumer_token}"}, timeout=60)
    if forbidden_delete.status_code != 403:
        raise RuntimeError(f"consumer delete expected 403, got {forbidden_delete.status_code} {forbidden_delete.text[:300]}")
    result["steps"]["consumerDeleteForbidden"] = True

    delete = session.delete(f"{base}/api/l402/paywalls/{paywall_id}", headers={"Authorization": f"Bearer {admin_token}"}, timeout=120)
    ensure(delete, "admin delete paywall")
    deleted = delete.json()["data"]

    result["steps"]["adminDelete"] = {
        "status": delete.status_code,
        "deleted": deleted.get("deleted"),
        "mutationEventId": deleted.get("mutationEventId"),
        "deletedAt": (deleted.get("paywall") or {}).get("deletedAt"),
    }

    delete_again = session.delete(f"{base}/api/l402/paywalls/{paywall_id}", headers={"Authorization": f"Bearer {admin_token}"}, timeout=60)
    if delete_again.status_code != 404:
        raise RuntimeError(f"second delete expected 404, got {delete_again.status_code} {delete_again.text[:300]}")
    result["steps"]["adminDeleteAgain404"] = True

    consumer_read_after = session.get(f"{base}/api/l402/paywalls", headers={"Authorization": f"Bearer {consumer_token}"}, timeout=30)
    ensure(consumer_read_after, "consumer read paywalls after")
    result["steps"]["consumerReadPaywallsAfter"] = consumer_read_after.status_code

    result["ok"] = True
    print(json.dumps(result, indent=2))
except Exception as exc:
    result["error"] = str(exc)
    print(json.dumps(result, indent=2))
    sys.exit(1)
PY
