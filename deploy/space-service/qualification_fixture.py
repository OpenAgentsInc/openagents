#!/usr/bin/env python3
"""Seed the separate native database with one synthetic qualification owner."""
import hashlib
import json
import os
import subprocess
from database_check import credential
# The filename installed beside this module keeps Python imports unambiguous.
from cluster_secrets import identity, secret


def main():
    if os.getuid() != 0:
        raise RuntimeError("Run fixture setup through the owned control identity.")
    fixture = secret("qualification", identity())
    if fixture["schema"] != "openagents.space.qualification.v1" or fixture["org_id"] != "00000000-0000-4000-8000-000000011814":
        raise RuntimeError("The qualification identity differs.")
    # UUIDs and token hashes are the only generated values entering SQL.
    import uuid
    org, user, key, session = (str(uuid.UUID(fixture[name])) for name in ("org_id", "user_id", "key_id", "session_id"))
    api_hash = hashlib.sha256(fixture["api_key"].encode()).hexdigest()
    session_hash = hashlib.sha256(fixture["admin_session"].encode()).hexdigest()
    sql = f"""BEGIN;
SET ROLE space_migrator;
INSERT INTO organizations (id,name,slug,snapshot_base_object_storage_uri)
VALUES ('{org}','SPACE qualification 11814','qualification-11814','s3://openagentsgemini-oa-space-service-snapshots/org/{org}/')
ON CONFLICT (id) DO NOTHING;
INSERT INTO users (id,email,password_hash,display_name,org_id,role)
VALUES ('{user}','qualification-11814@openagents.invalid','!','Qualification operator','{org}','organization_admin')
ON CONFLICT (id) DO NOTHING;
INSERT INTO api_keys (id,org_id,name,key_hash,created_by,allowed_surfaces)
VALUES ('{key}','{org}','qualification-11814','{api_hash}','{user}',ARRAY['ecw','native'])
ON CONFLICT (id) DO NOTHING;
INSERT INTO sessions (id,user_id,token_hash,expires_at)
VALUES ('{session}','{user}','{session_hash}',now()+interval '4 hours')
ON CONFLICT (id) DO UPDATE SET expires_at=excluded.expires_at;
UPDATE clusters SET managing_org_id='{org}' WHERE managing_org_id IN ('00000000-0000-0000-0000-000000000001','{org}');
COMMIT;
"""
    value = credential()
    environment = {**os.environ, "PGHOST": value["private_ip"], "PGDATABASE": "space", "PGUSER": value["bootstrap_login"],
        "PGPASSWORD": value["bootstrap_password"], "PGSSLMODE": "verify-ca", "PGSSLROOTCERT": "/space/config/postgres-ca.crt", "PGCONNECT_TIMEOUT": "5", "PSQL_HISTORY": "/dev/null"}
    result = subprocess.run(["psql", "--no-psqlrc", "--no-password", "--set=ON_ERROR_STOP=1"], input=sql.encode(), env=environment, capture_output=True, timeout=20)
    if result.returncode:
        # SQL contains hashes only; emit no raw credential or full query.
        raise RuntimeError("The synthetic native database fixture failed: " + result.stderr.decode()[:512])
    print(json.dumps({"synthetic_org": org, "password_login": "locked", "admin_session_hours": 4, "owner_account_used": False}))


if __name__ == "__main__":
    main()
