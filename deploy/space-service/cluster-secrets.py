#!/usr/bin/env python3
"""Install owned cluster secrets through the dedicated control identity."""
import argparse
import base64
import http.client
import json
import subprocess
from database_check import crc32c

PROJECT = "openagentsgemini"
PREFIX = "oa-space-service"
LABELS = {"openagents-service": "space", "openagents-issue": "11814"}


def api(host, method, path, token, body=None):
    connection = http.client.HTTPSConnection(host, timeout=15)
    try:
        connection.request(method, path, body=json.dumps(body).encode() if body is not None else None,
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
        response = connection.getresponse()
        if response.status != 200:
            raise RuntimeError(f"Private credential access failed with HTTP {response.status}.")
        encoded = response.read(1048577)
        if len(encoded) > 1048576:
            raise RuntimeError("The credential response exceeded its size limit.")
        return json.loads(encoded)
    finally:
        connection.close()


def identity():
    connection = http.client.HTTPConnection("169.254.169.254", timeout=5)
    try:
        connection.request("GET", "/computeMetadata/v1/instance/service-accounts/default/token", headers={"Metadata-Flavor": "Google"})
        response = connection.getresponse()
        if response.status != 200 or response.getheader("Metadata-Flavor") != "Google":
            raise RuntimeError("The control identity is unavailable.")
        return json.loads(response.read(65536))["access_token"]
    finally:
        connection.close()


def secret(name, token):
    value = api("secretmanager.googleapis.com", "GET", f"/v1/projects/{PROJECT}/secrets/{PREFIX}-{name}/versions/1:access", token)
    data = base64.b64decode(value["payload"]["data"], validate=True)
    if len(data) > 65536 or crc32c(data) != int(value["payload"]["dataCrc32c"]):
        raise RuntimeError("The saved credential failed its integrity check.")
    return json.loads(data)


def kubectl(*args, data=None, missing=False):
    command = ["/usr/local/bin/k3s", "kubectl", "--cache-dir=/space/kube-cache", *args]
    result = subprocess.run(command, input=json.dumps(data).encode() if data is not None else None,
                            capture_output=True, timeout=30)
    if result.returncode:
        if missing and b"NotFound" in result.stderr:
            return None
        raise RuntimeError("The cluster credential operation failed.")
    return json.loads(result.stdout) if result.stdout else {}


def install(name, values, kind="Opaque", rotate=False):
    existing = kubectl("-n", "space", "get", "secret", name, "-o", "json", missing=True)
    if existing:
        if existing["metadata"].get("labels", {}).get("openagents-issue") != "11814":
            raise RuntimeError("A secret with this name belongs to another deployment.")
        if existing.get("data") == values:
            return
        if not rotate:
            raise RuntimeError("The saved cluster credential differs. Review its references before changing it.")
    value = {"apiVersion": "v1", "kind": "Secret", "metadata": {"name": name, "namespace": "space", "labels": LABELS}, "type": kind, "data": values}
    if existing:
        value["metadata"]["resourceVersion"] = existing["metadata"]["resourceVersion"]
    kubectl("replace" if existing else "create", "-f", "-", "-o", "json", data=value)


def encoded(values):
    return {key: base64.b64encode(value.encode()).decode() for key, value in values.items()}


def pull(token):
    identity_name = f"{PREFIX}-pull@{PROJECT}.iam.gserviceaccount.com"
    generated = api("iamcredentials.googleapis.com", "POST", f"/v1/projects/-/serviceAccounts/{identity_name}:generateAccessToken", token,
                    {"scope": ["https://www.googleapis.com/auth/cloud-platform"], "lifetime": "3600s"})
    credential = "oauth2accesstoken:" + generated["accessToken"]
    config = {"auths": {"us-central1-docker.pkg.dev": {"auth": base64.b64encode(credential.encode()).decode()}}}
    install("space-image-reader", encoded({".dockerconfigjson": json.dumps(config)}), "kubernetes.io/dockerconfigjson", rotate=True)
    print(json.dumps({"image_pull_identity": identity_name, "expires": generated["expireTime"]}))


def main(pull_only):
    token = identity()
    namespace = kubectl("get", "namespace", "space", "-o", "json", missing=True)
    if namespace is None:
        if pull_only:
            raise RuntimeError("Initialize the owned namespace before refreshing image credentials.")
        kubectl("create", "-f", "-", "-o", "json", data={"apiVersion": "v1", "kind": "Namespace", "metadata": {"name": "space", "labels": LABELS}})
    elif namespace["metadata"].get("labels", {}).get("openagents-issue") != "11814":
        raise RuntimeError("The namespace belongs to another deployment.")
    if not pull_only:
        native = secret("native-secrets", token)
        if native["schema"] != "openagents.space.native-secrets.v1" or native["namespace"] != "space":
            raise RuntimeError("The native credential format differs.")
        for name, data in native["secrets"].items():
            install(name, data)
        database = secret("native-database", token)
        install("space-postgres", encoded({"POSTGRES_CONNECTION_STRING": database["base_url"]}))
        install("space-postgres-bootstrap", encoded({"POSTGRES_CONNECTION_STRING": database["bootstrap_url"], "PGPASSWORD": database["bootstrap_password"]}))
        install("space-postgres-ca", encoded({"ca.crt": database["server_ca_pem"]}))
        install("space-db-roles", encoded({f"SPACE_DB_BOOTSTRAP_{role}_PASSWORD": password for role, password in database["role_passwords"].items()}))
        store = secret("object-credentials", token)
        install("space-object-store", encoded({"SPACE_OBJECT_STORE_STATIC_ACCESS_KEY_ID": store["access_key_id"], "SPACE_OBJECT_STORE_STATIC_SECRET_ACCESS_KEY": store["secret_access_key"]}))
        print("Owned native credentials installed from numeric Secret Manager versions.")
    pull(token)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pull-only", action="store_true")
    main(parser.parse_args().pull_only)
