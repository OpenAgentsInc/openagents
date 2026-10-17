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


def candidate():
    from cluster_secrets import identity, secret
    value=credential(); candidate=secret('computers-database',identity())
    environment={**os.environ,'PGHOST':value['private_ip'],'PGDATABASE':'space_computers','PGUSER':'space_computers',
        'PGPASSWORD':candidate['password'],'PGSSLMODE':'verify-ca','PGSSLROOTCERT':'/space/config/postgres-ca.crt','PGCONNECT_TIMEOUT':'5','PSQL_HISTORY':'/dev/null'}
    command=['psql','--no-psqlrc','--no-password','--tuples-only','--no-align','--set=ON_ERROR_STOP=1']
    query="SELECT current_user,ssl,rolsuper,rolcreatedb,rolcreaterole,pg_has_role(current_user,'cloudsqlsuperuser','MEMBER') FROM pg_stat_ssl,pg_roles WHERE pid=pg_backend_pid() AND rolname=current_user;"
    checked=subprocess.run(command+['--command',query],env=environment,capture_output=True,timeout=15)
    if checked.returncode or checked.stdout.decode().strip()!='space_computers|t|f|f|f|f': raise RuntimeError('Candidate database TLS or privilege check failed.')
    negative=subprocess.run(command+['--command','SELECT 1'],env={**environment,'PGDATABASE':'space'},capture_output=True,timeout=15)
    if not negative.returncode: raise RuntimeError('The candidate role can connect to native SPACE data.')
    reverse=subprocess.run(command+['--command','SELECT 1'],env={**environment,'PGUSER':'space_user','PGPASSWORD':value['role_passwords']['USER']},capture_output=True,timeout=15)
    if not reverse.returncode: raise RuntimeError('The native role can connect to the candidate database.')
    print(json.dumps({'candidate_role':'space_computers','tls':True,'superuser':False,'cloudsqlsuperuser':False,'createdb':False,'createrole':False,'cross_database_connections_refused':2}))


def candidate_limits():
    from cluster_secrets import identity,secret
    value=credential(); saved=secret('computers-database',identity())
    environment={**os.environ,'PGHOST':value['private_ip'],'PGDATABASE':'space_computers','PGUSER':'space_computers',
        'PGPASSWORD':saved['password'],'PGSSLMODE':'verify-ca','PGSSLROOTCERT':'/space/config/postgres-ca.crt','PGCONNECT_TIMEOUT':'5','PSQL_HISTORY':'/dev/null'}
    # Bootstrap only: require an empty, migrated candidate. Later changes use
    # oa-computers-usage so its checks include committed allocations.
    sql="""BEGIN;
SELECT pg_advisory_xact_lock(11814);
DO $$ BEGIN
 IF EXISTS (SELECT 1 FROM computers.computers) THEN RAISE EXCEPTION 'Configure limits before creating qualification computers'; END IF;
END $$;
INSERT INTO computers.usage_limits(scope,subject,computer_count,running_vcpus,running_memory_mib,turn_seconds,stopped_retention_seconds)
VALUES ('service','',2,4,8192,60,3600),('account','space-qualification-11814',2,4,8192,60,3600)
ON CONFLICT(scope,subject) DO NOTHING;
COMMIT;
"""
    result=subprocess.run(['psql','--no-psqlrc','--no-password','--set=ON_ERROR_STOP=1'],input=sql.encode(),env=environment,capture_output=True,timeout=20)
    if result.returncode: raise RuntimeError('Configure the empty candidate limits before creating computers: '+result.stderr.decode()[:512])
    print(json.dumps({'computer_count':2,'running_vcpus':4,'running_memory_mib':8192,'turn_seconds':60,'stopped_retention_seconds':3600,'scope':['service','qualification account']}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--activate", action="store_true")
    parser.add_argument("--candidate",action="store_true")
    parser.add_argument("--candidate-limits",action="store_true")
    args=parser.parse_args()
    if args.candidate_limits: candidate_limits()
    elif args.candidate: candidate()
    else: main(args.activate)
