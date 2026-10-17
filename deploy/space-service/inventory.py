#!/usr/bin/env python3
"""Inventory maintained SPACE resources without deleting or adopting anything."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import json
from urllib.parse import urlencode
from broker import cloud, PROJECT, PREFIX, REGION, MARKER
import storage


def owned(value):
    labels=value.get('labels',value.get('metadata',{}).get('labels',value.get('settings',{}).get('userLabels',{})))
    return labels.get('openagents-issue')=='11814' or value.get('description')==MARKER


def collect(kind,args):
    rows=cloud(*args)
    result=[]
    for value in rows:
        name=value.get('name',value.get('metadata',{}).get('name',value.get('email','')))
        if PREFIX not in name: continue
        item={'kind':kind,'name':name,'owned':owned(value)}
        if kind=='vm':
            item.update({'state':value['status'],'zone':value['zone'].rsplit('/',1)[-1],
                'machine_type':value['machineType'].rsplit('/',1)[-1],
                'disks':[{'name':d['source'].rsplit('/',1)[-1],'auto_delete':d.get('autoDelete',False)} for d in value['disks']],
                'google_identities':len(value.get('serviceAccounts',[])),
                'public_addresses':sum(len(n.get('accessConfigs',[])) for n in value.get('networkInterfaces',[]))})
        elif kind=='disk':
            item.update({'gib':int(value['sizeGb']),'users':len(value.get('users',[]))})
        elif kind=='database':
            s=value['settings'];item.update({'state':value['state'],'tier':s['tier'],
                'disk_gib':int(s['dataDiskSizeGb']),'growth_limit_gib':int(s['storageAutoResizeLimit']),
                'public_ip_enabled':s['ipConfiguration'].get('ipv4Enabled',False)})
        elif kind=='cloud_run':
            annotations=value['spec']['template']['metadata'].get('annotations',{})
            item.update({'revision':value.get('status',{}).get('latestReadyRevisionName'),
                'ingress':value['metadata'].get('annotations',{}).get('run.googleapis.com/ingress'),
                'max_instances':annotations.get('autoscaling.knative.dev/maxScale')})
        elif kind=='router':
            item['nat']=[{key:n.get(key) for key in ('name','natIpAllocateOption','sourceSubnetworkIpRangesToNat','minPortsPerVm','maxPortsPerVm','enableDynamicPortAllocation')} for n in value.get('nats',[])]
        result.append(item)
    return result


def objects():
    result=[]
    with storage.private_tunnel() as port:
        storage.PRIVATE_PORT=port
        for purpose in ('templates','snapshots'):
            bucket=f'{PROJECT}-{PREFIX}-{purpose}'
            metadata=storage.api('GET','b/'+bucket+'?fields=name,labels,softDeletePolicy,ipFilter')
            if not owned(metadata): raise RuntimeError('The bucket ownership differs.')
            token=None;count=0;size=0;prefixes=set()
            for _ in range(100):
                query={'maxResults':1000,'fields':'nextPageToken,items(name,size)'}
                if token: query['pageToken']=token
                page=storage.api('GET','b/'+bucket+'/o?'+urlencode(query))
                for item in page.get('items',[]):
                    count+=1;size+=int(item['size']);prefixes.add(item['name'].split('/',1)[0])
                token=page.get('nextPageToken')
                if not token: break
            else: raise RuntimeError('The object inventory exceeded its page bound; do not report it as complete.')
            result.append({'bucket':bucket,'active_objects':count,'active_bytes':size,
                'top_level_prefixes':sorted(prefixes),'soft_delete':metadata.get('softDeletePolicy'),
                'ip_filter_enabled':metadata.get('ipFilter',{}).get('mode')=='Enabled',
                'includes_soft_deleted_objects_or_unfinished_uploads':False})
        storage.PRIVATE_PORT=None
    return result


def main(include_objects):
    commands=[('vm',('compute','instances','list')),('disk',('compute','disks','list')),
        ('network',('compute','networks','list')),('subnet',('compute','networks','subnets','list')),
        ('firewall',('compute','firewall-rules','list')),('router',('compute','routers','list')),
        ('address',('compute','addresses','list')),('database',('sql','instances','list')),
        ('cloud_run',('run','services','list','--region='+REGION)),
        ('repository',('artifacts','repositories','list','--location='+REGION)),
        ('secret',('secrets','list')),('identity',('iam','service-accounts','list'))]
    with ThreadPoolExecutor(max_workers=4) as pool:
        groups=list(pool.map(lambda command:collect(*command),commands))
    resources=[item for group in groups for item in group]
    unknown=[{'kind':r['kind'],'name':r['name']} for r in resources if not r['owned']]
    unattached=[r['name'] for r in resources if r['owned'] and r['kind']=='disk' and r['users']==0]
    print(json.dumps({'observed_at':datetime.now(timezone.utc).isoformat(),'project':PROJECT,
        'resources':resources,'unconfirmed_ownership':unknown,'unattached_owned_disks':unattached,
        'objects':objects() if include_objects else None,'deletions_performed':0},indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--objects',action='store_true',help='Count active objects over the private connection.')
    main(parser.parse_args().objects)
