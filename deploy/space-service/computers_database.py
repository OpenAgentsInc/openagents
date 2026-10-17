#!/usr/bin/env python3
"""Create the candidate's separate database with a limited existing-role login."""
import json
import os
import subprocess
from database_check import credential


def main():
    if os.getuid()!=0: raise RuntimeError('Run database setup through the owned control identity.')
    value=credential()
    environment={**os.environ,'PGHOST':value['private_ip'],'PGDATABASE':'postgres','PGUSER':value['bootstrap_login'],
        'PGPASSWORD':value['bootstrap_password'],'PGSSLMODE':'verify-ca','PGSSLROOTCERT':'/space/config/postgres-ca.crt','PGCONNECT_TIMEOUT':'5','PSQL_HISTORY':'/dev/null'}
    sql=r"""DO $$ BEGIN
IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname='space_computers') THEN
 IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='space_computers' AND shobj_description(oid,'pg_authid')='OpenAgents maintained SPACE service (#11814)') THEN
  RAISE EXCEPTION 'The candidate database role belongs to another deployment';
 END IF;
ELSE
 CREATE ROLE space_computers LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS;
 COMMENT ON ROLE space_computers IS 'OpenAgents maintained SPACE service (#11814)';
END IF;
END $$;
GRANT space_computers TO space_bootstrap;
SELECT 'CREATE DATABASE space_computers OWNER space_computers' WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname='space_computers');
\gexec
REVOKE CONNECT ON DATABASE space_computers FROM PUBLIC;
GRANT CONNECT ON DATABASE space_computers TO space_computers,space_bootstrap;
REVOKE CONNECT ON DATABASE space FROM PUBLIC;
GRANT CONNECT ON DATABASE space TO space_bootstrap,space_superadmin,space_user,space_ro,space_migrator,space_creds;
"""
    result=subprocess.run(['psql','--no-psqlrc','--no-password','--set=ON_ERROR_STOP=1'],input=sql.encode(),env=environment,capture_output=True,timeout=25)
    if result.returncode: raise RuntimeError('Candidate database setup failed: '+result.stderr.decode()[:512])
    print(json.dumps({'database':'space_computers','native_database':'space','public_connect_revoked':True,'passwords_in_sql_or_argv':False}))


if __name__=='__main__': main()
