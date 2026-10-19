#!/bin/sh
# #11816 rerun on the final private Computers image: control-node cases and trials.
# Each step records its own file under v3/; a failed step is kept and the run continues.
# Steps continue after a failure; each records its own output.
T=/space/config/deployment-tools; Q="python3 $T/q16.py"; R=/space/qualification-11816; V=$R/v3
mkdir -p $V $R/v1-archive; cd $R
# Earlier records were exported to the repository; keep them out of this run's files.
mv -f trials-*.jsonl case-*.json fault-*.json isolation.json egress*.json $R/v1-archive/ 2>/dev/null
mk() { $Q call POST /v1/computers "{\"size\":\"small\",\"idempotency_key\":\"q16-v3-$1-$(date +%s)\"}" --who ${2:-a} | python3 -c 'import json,sys;print(json.load(sys.stdin)["value"]["id"])'; }
ready() { $Q wait $1 awake --seconds 600 >/dev/null && $Q seed $1 --who ${2:-a} >/dev/null; }
gone() { $Q call DELETE /v1/computers/$1 --who ${2:-a} >/dev/null; $Q wait $1 deleted --seconds 900 --who ${2:-a} >/dev/null; }
fresh() { c=$(mk $1); ready $c; echo $c; }
step() { echo "== $(date -u +%T) $*"; }
stranded() { $Q call GET /v1/computers/$1 | grep -q "couldn't confirm"; }

step cases-1; A=$(fresh cases1)
python3 $T/q16.py isolation $A trn_none > $V/isolation.log 2>&1; mv -f isolation.json $V/isolation.json 2>/dev/null
for c in attach lost_turn_reply cancellation; do
  stranded $A && { gone $A; A=$(fresh cases-$c); }
  python3 $T/q16_cases.py $c $A a > $V/run-$c.log 2>&1; mv -f case-$c.json $V/ 2>/dev/null
done
gone $A
step duplicates; python3 $T/q16_cases.py duplicates a > $V/run-duplicates.log 2>&1; mv -f case-duplicates.json $V/ 2>/dev/null

step capacity; X=$(mk capx); Y=$(mk capy); $Q wait $X awake >/dev/null; $Q wait $Y awake >/dev/null
$Q call POST /v1/computers '{"size":"small","idempotency_key":"q16-v3-cap3"}' > $V/capacity-third.json 2>&1
$Q call POST /v1/computers '{"size":"medium","idempotency_key":"q16-v3-cap3m"}' >> $V/capacity-third.json 2>&1
step quotas; $Q call POST /v1/computers/$X/turns '{"command":"sleep 75; echo over-limit","timeout_seconds":120}' > $V/quota-over.json 2>&1
python3 - "$X" > $V/quota-at-limit.json 2>&1 <<'PY'
import json,sys,importlib.util
spec=importlib.util.spec_from_file_location('q16','/space/config/deployment-tools/q16.py'); q=importlib.util.module_from_spec(spec); spec.loader.exec_module(q)
tid,r=q.turn(sys.argv[1],'sleep 75; echo over-limit',seconds=180,timeout=60); print(json.dumps({'turn':tid,'result':q.brief(r),'stdout':r.get('stdout')}))
PY
$Q call GET /v1/usage > $V/usage.json 2>&1
step egress; python3 $T/q16_egress.py $Y a > $V/egress.log 2>&1; cp -f egress.json $V/egress.json 2>/dev/null
step files-and-controls; python3 - "$Y" > $V/files-commands-controls.json 2>&1 <<'PY'
import json,sys,time,http.client,importlib.util,hashlib
spec=importlib.util.spec_from_file_location('q16','/space/config/deployment-tools/q16.py'); q=importlib.util.module_from_spec(spec); spec.loader.exec_module(q)
c=sys.argv[1]; out={'computer':c}
st,v,_=q.call('GET',f'/v1/computers/{c}');
if v.get('state')!='awake': q.call('POST',f'/v1/computers/{c}/wake',{}); q.wait_state(c,'awake',600)
def raw(m,p,body=None):
    k=http.client.HTTPSConnection(q.HOST,timeout=60); h={'Authorization':'Bearer '+q.fixture()['computers_token'],'X-OpenAgents-Account':q.owner('a'),'X-OpenAgents-Test':'1','Content-Type':'application/octet-stream'}
    if body is not None: h['Content-Length']=str(len(body))
    k.request(m,p,body=body,headers=h); r=k.getresponse(); b=r.read(65536); k.close(); return r.status,b
data=b'OpenAgents SDK binary fixture\x00\xff\x80\n'
out['put']=raw('PUT',f'/v1/computers/{c}/files?path=sdk-fixture.bin',data)[0]
s,b=raw('GET',f'/v1/computers/{c}/files?path=sdk-fixture.bin'); out['get']=s; out['read_back_equal']=(b==data)
st,v,_=q.call('POST',f'/v1/computers/{c}/commands',{'command':'sha256sum sdk-fixture.bin','idempotency_key':'q16-v3-cmd-'+str(int(time.time()))}); out['command_create']=st; cid=(v or {}).get('id')
for _ in range(120):
    if not cid: break
    st,v,_=q.call('GET',f'/v1/computers/{c}/commands/{cid}')
    if (v or {}).get('state') in ('succeeded','failed','cancelled','unknown'): break
    time.sleep(0.5)
out['command_state']=(v or {}).get('state'); st,o,_=q.call('GET',f'/v1/computers/{c}/commands/{cid}/output') if cid else (0,{},0)
import base64; stdout=base64.b64decode((o or {}).get('stdout','') or b'').decode(errors='replace') if isinstance((o or {}).get('stdout'),str) else str(o)[:200]
out['command_checksum_ok']=stdout.split(' ')[0]==hashlib.sha256(data).hexdigest()
for name,m,p in [('fork','POST',f'/v1/computers/{c}/fork'),('snapshots','POST',f'/v1/computers/{c}/snapshots'),('terminal','POST',f'/v1/computers/{c}/terminal'),('preview','GET',f'/v1/computers/{c}/preview')]:
    out[name]=q.call(m,p,{} if m=='POST' else None)[0]
st,v,_=q.call('GET','/v1/capabilities'); out['capabilities']=v.get('placements')
print(json.dumps(out))
PY
gone $X; gone $Y

step faults; F=$(fresh faults)
for f in api_restart controller_restart delayed provider_outage; do
  stranded $F && { gone $F; F=$(fresh faults-$f); }
  python3 $T/q16_faults.py $f $F a > $V/run-fault-$f.log 2>&1; mv -f fault-$f.json $V/ 2>/dev/null
done
gone $F

step trials-cold
$Q trials cold v3-cold-warm-c1 --count 20 --owners a > $V/run-cold-c1.log 2>&1
$Q trials cold v3-cold-warm-c2 --count 3 --concurrency 2 --owners a,a > $V/run-cold-c2.log 2>&1
step trials-wake; X=$(fresh wakex); Y=$(fresh wakey)
sh $T/q16_fix_wake.sh $X $Y 0 > $V/run-wake.log 2>&1
for c in $(grep -o 'final .*' $V/run-wake.log | tail -1 | cut -d' ' -f2-); do gone $c; done
mv -f trials-v3-* $V/ 2>/dev/null; mv -f trials-fix-wake-warm-c1.jsonl $V/trials-v3-wake-warm-c1.jsonl 2>/dev/null; mv -f trials-fix-wake-warm-c2.jsonl $V/trials-v3-wake-warm-c2.jsonl 2>/dev/null
step done; echo done-full
