#!/usr/bin/env python3
"""Activate and verify existing native database logins without SQL passwords."""
import argparse
import json
import os
import subprocess
from database_check import credential

ROLES = {"SUPERADMIN": "space_superadmin", "USER": "space_user", "RO": "space_ro", "MIGRATOR": "space_migrator", "CREDS": "space_creds"}


def main(activate):
    if os.getuid() != 0:
        raise RuntimeError("Run the database check through the owned control identity.")
    value = credential()
    environment = {**os.environ, "PGHOST": value["private_ip"], "PGDATABASE": "space", "PGUSER": value["bootstrap_login"],
        "PGPASSWORD": value["bootstrap_password"], "PGSSLMODE": "verify-ca", "PGSSLROOTCERT": "/space/config/postgres-ca.crt",
        "PGCONNECT_TIMEOUT": "5", "PSQL_HISTORY": "/dev/null"}
    command = ["psql", "--no-psqlrc", "--no-password", "--tuples-only", "--no-align", "--set=ON_ERROR_STOP=1"]
    if activate:
        # Role names are fixed, and each ALTER requires the bootstrap's ADMIN
        # membership from native provisioning. No password enters SQL.
        sql = "BEGIN;\n" + "\n".join(f"ALTER ROLE {role} LOGIN;" for role in ROLES.values()) + "\nCOMMIT;"
        result = subprocess.run(command, input=sql.encode(), env=environment, capture_output=True, timeout=20)
        if result.returncode:
            raise RuntimeError("Existing role activation failed.")
        print("Five existing native logins activated; no password was sent through SQL.")
        return
    metadata = []
    for key, role in ROLES.items():
        query = "SELECT current_user, ssl, rolsuper, pg_has_role(current_user, 'cloudsqlsuperuser', 'MEMBER') FROM pg_stat_ssl, pg_roles WHERE pid=pg_backend_pid() AND rolname=current_user;"
        result = subprocess.run(command + ["--command", query], env={**environment, "PGUSER": role, "PGPASSWORD": value["role_passwords"][key]}, capture_output=True, timeout=15)
        expected = f"{role}|t|f|f"
        if result.returncode or result.stdout.decode().strip() != expected:
            raise RuntimeError(f"The {role} login, TLS, or managed-superuser fence check failed.")
        metadata.append({"role": role, "tls": True, "superuser": False, "cloudsqlsuperuser": False})
    print(json.dumps({"verified": metadata, "passwords_in_sql_or_argv": False}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--activate", action="store_true")
    main(parser.parse_args().activate)
