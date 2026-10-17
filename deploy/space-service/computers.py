#!/usr/bin/env python3
"""Prepare the isolated Computers candidate's numeric secret versions."""
import argparse
import json
import secrets
import subprocess
import time
from urllib.parse import quote
from broker import cloud, PROJECT, PREFIX, LABELS
from database import sql_api


def saved(name):
    data=subprocess.check_output(['gcloud','secrets','versions','access','1','--secret='+PREFIX+'-'+name,'--project='+PROJECT,'--quiet'],stderr=subprocess.DEVNULL)
    if len(data)>65536: raise RuntimeError('The saved credential exceeds its size limit.')
    return data


def install(name,data):
    resource=PREFIX+'-'+name
    previous=cloud('secrets','describe',resource,missing=True)
    if previous is None:
        cloud('secrets','create',resource,'--replication-policy=automatic','--labels='+LABELS,'--data-file=-',data=data)
    elif previous.get('labels',{}).get('openagents-issue')!='11814' or saved(name)!=data:
        raise RuntimeError('The saved candidate credential differs. Review its existing consumers before changing it.')


def configure(passwords):
    instance=cloud('sql','instances','describe',PREFIX)
    if instance['settings'].get('userLabels',{}).get('openagents-issue')!='11814': raise RuntimeError('The native database belongs to another deployment.')
    native=json.loads(saved('native-database'))
    existing=cloud('secrets','describe',PREFIX+'-computers-database',missing=True)
    if existing is None:
        password=secrets.token_urlsafe(48)
        query='sslmode=verify-ca&sslrootcert=/var/run/space-postgres-ca/ca.crt'
        value={'role':'space_computers','password':password,'database':'space_computers',
            'dsn':f'postgresql://space_computers:{quote(password,safe="")}@{native["private_ip"]}/space_computers?{query}'}
        install('computers-database',json.dumps(value).encode())
    else: value=json.loads(saved('computers-database'))
    fixture=json.loads(saved('qualification'))
    install('computers-dsn',value['dsn'].encode())
    install('computers-token',fixture['computers_token'].encode())
    install('computers-space-token',fixture['api_key'].encode())
    install('computers-postgres-ca',native['server_ca_pem'].encode())
    if passwords:
        users=sql_api('GET',f'instances/{PREFIX}/users')
        if not any(u['name']=='space_computers' for u in users.get('items',[])): raise RuntimeError('Create the limited candidate role first.')
        operation=sql_api('PUT',f'instances/{PREFIX}/users?name=space_computers',{'name':'space_computers','password':value['password'],'type':'BUILT_IN'})
        deadline=time.monotonic()+40
        while operation['status']!='DONE' and time.monotonic()<deadline:
            time.sleep(1); operation=sql_api('GET','operations/'+operation['name'])
        if operation['status']!='DONE' or operation.get('error'): raise RuntimeError('Inspect the candidate password operation before retrying.')
    print(json.dumps({'database':'space_computers','numeric_secret_version':1,'passwords_in_sql_or_argv':False,'password_update_requested':passwords}))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--set-password',action='store_true')
    configure(parser.parse_args().set_password)
