#!/usr/bin/env python3
"""#11816: files, commands, and refused controls on one SPACE computer (argument: computer ID)."""
import json,sys,time,http.client,importlib.util,hashlib
spec=importlib.util.spec_from_file_location('q16','/space/config/deployment-tools/q16.py'); q=importlib.util.module_from_spec(spec); spec.loader.exec_module(q)
c=sys.argv[1]; out={'computer':c}
st,v,_=q.call('GET',f'/v1/computers/{c}');
if v.get('state')!='awake': q.call('POST',f'/v1/computers/{c}/wake',{}); q.wait_state(c,'awake',600)
def raw(m,p,body=None):
    k=http.client.HTTPSConnection(q.HOST,timeout=60); h={'Authorization':'Bearer '+q.fixture()['computers_token'],'X-OpenAgents-Account':q.owner('a'),'X-OpenAgents-Test':'1','Content-Type':'application/octet-stream'}
    if body is not None: h['Content-Length']=str(len(body)); h['x-content-sha256']=hashlib.sha256(body).hexdigest()
    k.request(m,p,body=body,headers=h); r=k.getresponse(); b=r.read(65536); k.close(); return r.status,b
data=b'OpenAgents SDK binary fixture\x00\xff\x80\n'
out['put']=raw('PUT',f'/v1/computers/{c}/files?path=sdk-fixture.bin',data)[0]
s,b=raw('GET',f'/v1/computers/{c}/files?path=sdk-fixture.bin'); out['get']=s; out['read_back_equal']=(b==data)
st,v,_=q.call('POST',f'/v1/computers/{c}/commands',{'command':'sha256sum sdk-fixture.bin','timeout_seconds':30,'idempotency_key':'q16-v3-cmd-'+str(int(time.time()))}); out['command_create']=st; out['command_create_reply']=v if st>=400 else None; cid=(v or {}).get('id')
for _ in range(120):
    if not cid: break
    st,v,_=q.call('GET',f'/v1/computers/{c}/commands/{cid}')
    if (v or {}).get('state') in ('succeeded','failed','cancelled','unknown'): break
    time.sleep(0.5)
out['command_state']=(v or {}).get('state'); st,o,_=q.call('GET',f'/v1/computers/{c}/commands/{cid}/output') if cid else (0,{},0)
import base64; stdout=base64.b64decode((o or {}).get('stdout','') or b'').decode(errors='replace') if isinstance((o or {}).get('stdout'),str) else str(o)[:200]
out['output_fields']=sorted((o or {}).keys()) if isinstance(o,dict) else None
out['command_checksum_ok']=stdout.split(' ')[0]==hashlib.sha256(data).hexdigest()
for name,m,p in [('fork','POST',f'/v1/computers/{c}/fork'),('snapshots','POST',f'/v1/computers/{c}/snapshots'),('terminal','POST',f'/v1/computers/{c}/terminal'),('preview','GET',f'/v1/computers/{c}/preview')]:
    out[name]=q.call(m,p,{} if m=='POST' else None)[0]
st,v,_=q.call('GET','/v1/capabilities'); out['capabilities']=v.get('placements')
print(json.dumps(out))
