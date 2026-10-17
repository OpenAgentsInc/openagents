#!/usr/bin/env python3
"""Exercise recovery only on the owned disposable database copy."""
import argparse
import json
from broker import cloud, PROJECT, PREFIX, LABELS

COPY=PREFIX+'-restore-11814'


def owned(name):
    value=cloud('sql','instances','describe',name,missing=True)
    if value is not None:
        settings=value['settings']
        if settings.get('userLabels',{}).get('openagents-issue')!='11814': raise RuntimeError('The database belongs to another deployment.')
        if settings['ipConfiguration'].get('ipv4Enabled') or settings['tier']!='db-custom-1-3840': raise RuntimeError('The private recovery capacity settings differ.')
    return value


def main(command,backup):
    if COPY==PREFIX: raise RuntimeError('The recovery target must differ from the maintained service.')
    source=owned(PREFIX)
    if source is None: raise RuntimeError('The maintained native database is unavailable.')
    target=owned(COPY)
    if command=='clone':
        if target is not None: raise RuntimeError('Inspect the existing recovery copy before retrying clone.')
        operation=cloud('sql','instances','clone',PREFIX,COPY,'--async')
    elif command=='restore':
        if target is None or target['state']!='RUNNABLE' or backup is None: raise RuntimeError('Choose a completed native backup and a running recovery copy.')
        saved=cloud('sql','backups','describe',str(backup),'--instance='+PREFIX)
        if saved['status']!='SUCCESSFUL' or '11814' not in saved.get('description',''): raise RuntimeError('The backup is not the owned qualification backup.')
        operation=cloud('sql','backups','restore',str(backup),'--backup-instance='+PREFIX,'--restore-instance='+COPY,'--async')
    elif command=='delete':
        if target is None:
            print(json.dumps({'copy_absent':True})); return
        cloud('sql','instances','patch',COPY,'--no-deletion-protection')
        operation=cloud('sql','instances','delete',COPY,'--async')
    else:
        if target is None: raise RuntimeError('The recovery copy is absent.')
        if target['state']!='RUNNABLE':
            print(json.dumps({'name':COPY,'state':target['state'],'verification':'pending'}));return
        ip=target['settings']['ipConfiguration']
        if ip.get('sslMode')!='ENCRYPTED_ONLY' or ip.get('privateNetwork')!=source['settings']['ipConfiguration']['privateNetwork']: raise RuntimeError('The recovery network or TLS settings differ.')
        print(json.dumps({'name':COPY,'state':target['state'],'private_ip':next(row['ipAddress'] for row in target['ipAddresses'] if row['type']=='PRIVATE'), 'server_ca_pem':target['serverCaCert']['cert'],'maximum_disk_gib':target['settings']['storageAutoResizeLimit']}));return
    print(json.dumps({'copy':COPY,'operation':operation.get('name'),'state':operation.get('status'),'source_unchanged':True}))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command',choices=['clone','restore','status','delete'])
    parser.add_argument('--backup',type=int)
    args=parser.parse_args();main(args.command,args.backup)
