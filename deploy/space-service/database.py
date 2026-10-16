#!/usr/bin/env python3
"""Prepare native database credentials without putting passwords in argv."""
import hashlib
import http.client
import json
import secrets
import subprocess
import time
from urllib.parse import urlencode
from broker import cloud, PROJECT, PREFIX, LABELS


def sql_api(method, path, body=None):
    token = subprocess.check_output(["gcloud", "auth", "print-access-token"], stderr=subprocess.DEVNULL).decode().strip()
    connection = http.client.HTTPSConnection("sqladmin.googleapis.com", timeout=15)
    try:
        connection.request(method, f"/sql/v1beta4/projects/{PROJECT}/" + path,
            body=json.dumps(body).encode() if body is not None else None,
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
        response = connection.getresponse()
        if not 200 <= response.status < 300:
            raise RuntimeError(f"Database configuration failed with HTTP {response.status}.")
        value = response.read(1048577)
        if len(value) > 1048576:
            raise RuntimeError("The database response exceeded its size limit.")
        return json.loads(value)
    finally:
        connection.close()


def main():
    instance = cloud("sql", "instances", "describe", PREFIX)
    if instance["settings"].get("userLabels", {}).get("openagents-issue") != "11814":
        raise RuntimeError("The database belongs to another deployment.")
    ip = instance["settings"]["ipConfiguration"]
    if instance["state"] != "RUNNABLE" or ip["sslMode"] != "ENCRYPTED_ONLY" or ip["serverCaMode"] != "GOOGLE_MANAGED_INTERNAL_CA" or ip["ipv4Enabled"]:
        raise RuntimeError("Require the private, TLS-only database with its own instance CA.")
    address = next(row["ipAddress"] for row in instance["ipAddresses"] if row["type"] == "PRIVATE")
    certificate = instance["serverCaCert"]["cert"]
    secret_name = PREFIX + "-native-database"
    existing = cloud("secrets", "describe", secret_name, missing=True)
    if existing is None:
        query = urlencode({"sslmode": "verify-ca", "sslrootcert": "/var/run/space-postgres-ca/ca.crt"})
        credential = {"database": "space", "bootstrap_login": "space_bootstrap", "bootstrap_password": secrets.token_urlsafe(48),
            "private_ip": address, "server_ca_pem": certificate,
            "server_ca_sha256": hashlib.sha256(certificate.encode()).hexdigest(),
            "base_url": f"postgresql://space_user@{address}/space?{query}",
            "bootstrap_url": f"postgresql://space_bootstrap@{address}/space?{query}",
            "role_passwords": {role: secrets.token_urlsafe(48) for role in ("SUPERADMIN", "USER", "RO", "MIGRATOR", "CREDS")}}
        cloud("secrets", "create", secret_name, "--replication-policy=automatic", f"--labels={LABELS}",
              "--data-file=-", data=json.dumps(credential).encode())
    else:
        if existing.get("labels", {}).get("openagents-issue") != "11814":
            raise RuntimeError("The database credential belongs to another deployment.")
        encoded = subprocess.check_output(["gcloud", "secrets", "versions", "access", "1", "--secret=" + secret_name,
                                           "--project=" + PROJECT, "--quiet"], stderr=subprocess.DEVNULL)
        if len(encoded) > 65536:
            raise RuntimeError("The database credential exceeds its size limit.")
        credential = json.loads(encoded)
    if credential["private_ip"] != address or credential["server_ca_pem"] != certificate:
        raise RuntimeError("The database address or CA changed. Review the saved database references.")
    users = sql_api("GET", f"instances/{PREFIX}/users")
    if not any(user["name"] == credential["bootstrap_login"] for user in users.get("items", [])):
        operation = sql_api("POST", f"instances/{PREFIX}/users", {
            "name": credential["bootstrap_login"], "password": credential["bootstrap_password"], "type": "BUILT_IN"})
        deadline = time.monotonic() + 40
        while operation["status"] != "DONE" and time.monotonic() < deadline:
            time.sleep(1)
            operation = sql_api("GET", "operations/" + operation["name"])
        if operation["status"] != "DONE" or operation.get("error"):
            raise RuntimeError("Database user creation has not completed successfully. Check its operation before retrying.")
    cloud("secrets", "add-iam-policy-binding", secret_name,
          f"--member=serviceAccount:{PREFIX}-control@{PROJECT}.iam.gserviceaccount.com", "--role=roles/secretmanager.secretAccessor")
    print(json.dumps({"secret": secret_name, "version": 1, "bootstrap_login": credential["bootstrap_login"],
                      "tls_mode": "verify-ca", "instance_exclusive_ca_sha256": credential["server_ca_sha256"],
                      "passwords_in_connection_urls": False, "database_creation_and_native_migration": "pending"}))


if __name__ == "__main__":
    main()
