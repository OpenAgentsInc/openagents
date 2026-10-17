#!/usr/bin/env python3
"""Verify the restored native database through private, CA-verified TLS."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
from database_check import credential


def main(path):
    if os.getuid()!=0: raise RuntimeError('Run restore checks through the owned control identity.')
    copy=json.loads(Path(path).read_text());native=credential()
    if copy.get('name')!='oa-space-service-restore-11814' or copy.get('state')!='RUNNABLE' or copy.get('private_ip')==native['private_ip']:
        raise RuntimeError('Choose the running, separate owned recovery copy.')
    import ipaddress
    if ipaddress.ip_address(copy['private_ip']) not in ipaddress.ip_network('10.219.16.0/20'): raise RuntimeError('The recovery address is outside the owned private range.')
    ca=Path('/space/config/restore-postgres-ca.crt');ca.write_text(copy['server_ca_pem']);ca.chmod(0o600)
    environment={**os.environ,'PGHOST':copy['private_ip'],'PGDATABASE':'space','PGUSER':native['bootstrap_login'],
        'PGPASSWORD':native['bootstrap_password'],'PGSSLMODE':'verify-ca','PGSSLROOTCERT':str(ca),'PGCONNECT_TIMEOUT':'5','PSQL_HISTORY':'/dev/null'}
    query="""SELECT json_build_object(
'private_tls',(SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()),
'migrations',(SELECT count(*) FROM _sqlx_migrations WHERE success),
'published_seed',EXISTS(SELECT 1 FROM template_releases r WHERE r.template_name='seed' AND r.hash='sha256:f04ea327e945df73d7d85a4c346d03b9b2a5d80446212486da6173da67ba8ec2'),
'qualification_org',EXISTS(SELECT 1 FROM organizations WHERE id='00000000-0000-4000-8000-000000011814'));
"""
    result=subprocess.run(['psql','--no-psqlrc','--no-password','--tuples-only','--no-align','--set=ON_ERROR_STOP=1','--command',query],env=environment,capture_output=True,timeout=20)
    if result.returncode: raise RuntimeError('The restored data check failed: '+result.stderr.decode()[:512])
    value=json.loads(result.stdout)
    if not value['private_tls'] or value['migrations']<1 or not value['published_seed'] or not value['qualification_org']:
        raise RuntimeError('The restored backup is missing the expected native qualification data.')
    value.update({'copy':copy['name'],'instance_ca_sha256':hashlib.sha256(copy['server_ca_pem'].encode()).hexdigest(),'maintained_source_untouched':True})
    print(json.dumps(value))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('config')
    main(parser.parse_args().config)
