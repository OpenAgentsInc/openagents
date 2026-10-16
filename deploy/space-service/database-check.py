#!/usr/bin/env python3
"""Create the owned native database and verify TLS with the control identity."""
import base64
import hashlib
import http.client
import json
import os
from pathlib import Path
import subprocess

PROJECT = "openagentsgemini"
PREFIX = "oa-space-service"


def crc32c(data):
    value = 0xffffffff
    for byte in data:
        value ^= byte
        for _ in range(8):
            value = (value >> 1) ^ (0x82f63b78 if value & 1 else 0)
    return value ^ 0xffffffff


def credential():
    metadata = http.client.HTTPConnection("169.254.169.254", timeout=5)
    metadata.request("GET", "/computeMetadata/v1/instance/service-accounts/default/token", headers={"Metadata-Flavor": "Google"})
    response = metadata.getresponse()
    if response.status != 200 or response.getheader("Metadata-Flavor") != "Google":
        raise RuntimeError("The control service identity is unavailable.")
    token = json.loads(response.read(65536))["access_token"]
    metadata.close()
    connection = http.client.HTTPSConnection("secretmanager.googleapis.com", timeout=15)
    try:
        path = f"/v1/projects/{PROJECT}/secrets/{PREFIX}-native-database/versions/1:access"
        connection.request("GET", path, headers={"Authorization": "Bearer " + token})
        response = connection.getresponse()
        if response.status != 200:
            raise RuntimeError("The database credential is unavailable.")
        payload = json.loads(response.read(131072))["payload"]
        data = base64.b64decode(payload["data"], validate=True)
        assert len(data) <= 65536 and crc32c(data) == int(payload["dataCrc32c"])
        return json.loads(data)
    finally:
        connection.close()


def main():
    assert os.getuid() == 0 and crc32c(b"123456789") == 0xe3069283
    data = credential()
    assert data["database"] == "space" and data["bootstrap_login"] == "space_bootstrap"
    assert hashlib.sha256(data["server_ca_pem"].encode()).hexdigest() == data["server_ca_sha256"]
    ca = Path("/space/config/postgres-ca.crt")
    ca.write_text(data["server_ca_pem"])
    os.chmod(ca, 0o600)
    environment = {**os.environ, "PGHOST": data["private_ip"], "PGPORT": "5432", "PGUSER": data["bootstrap_login"],
        "PGPASSWORD": data["bootstrap_password"], "PGDATABASE": "postgres", "PGSSLMODE": "verify-ca",
        "PGSSLROOTCERT": str(ca), "PGCONNECT_TIMEOUT": "10", "PSQL_HISTORY": "/dev/null"}
    args = ["psql", "--no-psqlrc", "--no-password", "--tuples-only", "--no-align", "--set=ON_ERROR_STOP=1"]
    query = "SELECT ssl,version FROM pg_stat_ssl WHERE pid=pg_backend_pid();"
    positive = subprocess.run(args + ["--command", query], env=environment, capture_output=True, timeout=20)
    if positive.returncode or not positive.stdout.startswith(b"t|TLS"):
        raise RuntimeError("The private database TLS check failed.")
    negative = subprocess.run(args + ["--command", "SELECT 1"], env={**environment, "PGSSLMODE": "disable"}, capture_output=True, timeout=20)
    if not negative.returncode:
        raise RuntimeError("The database accepted an unencrypted connection.")
    sql = "SELECT 'CREATE DATABASE space OWNER space_bootstrap' WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname='space');\n\\gexec\n"
    created = subprocess.run(args, input=sql.encode(), env=environment, capture_output=True, timeout=20)
    if created.returncode:
        raise RuntimeError("The native database could not be created.")
    owner = subprocess.run(args + ["--command", "SELECT pg_get_userbyid(datdba) FROM pg_database WHERE datname='space'"], env=environment, capture_output=True, timeout=20)
    if owner.returncode or owner.stdout.strip() != b"space_bootstrap":
        raise RuntimeError("The native database owner differs.")
    print(json.dumps({"tls": positive.stdout.decode().strip(), "unencrypted_connection_refused": True,
                      "database": "space", "owner": "space_bootstrap", "password_in_argv": False}))


if __name__ == "__main__":
    main()
