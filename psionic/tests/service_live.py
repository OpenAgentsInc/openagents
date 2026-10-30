#!/usr/bin/env python3
"""Owned operational checks over real sockets/processes; no external service."""
import json,os,re,signal,subprocess,sys,time,urllib.request
from pathlib import Path
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();out.mkdir(parents=True,exist_ok=True)
supervisor=binary.with_name('psionic-supervise')

def wait_ready(process,log,after=0):
    deadline=time.monotonic()+120
    while time.monotonic()<deadline:
        text=log.read_text();ports=re.findall(r'on http://127\.0\.0\.1:(\d+)/v1/systemone',text)
        if len(ports)>after:return 'http://127.0.0.1:'+ports[-1]
        assert process.poll() is None,text[-2000:]
        time.sleep(.02)
    raise AssertionError('bounded warmup/readiness did not finish')
def get(url):
    with urllib.request.urlopen(url,timeout=10) as r:return json.load(r)
def ask(url,state='Water is wet.'):
    body={'model':'psionic-toy','state':state,'questions':{str(i):{'type':'choice','instructions':'Is the water wet?','criteria':{'yes':None,'no':None}} for i in range(3)}}
    return get(urllib.request.Request(url+'/v1/systemone',json.dumps(body).encode()))
def finish(p):
    p.terminate()
    try:assert p.wait(timeout=10)==0
    except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait();raise

answers=[]
for reuse in (False,True):
    log=out/('reuse.log' if reuse else 'baseline.log');metrics=out/('reuse.jsonl' if reuse else 'baseline.jsonl')
    args=[str(binary),'--toy','--port','0','--capacity','512','--metrics',str(metrics)]+([] if reuse else ['--no-prefix-reuse'])
    with log.open('w') as stream:
        p=subprocess.Popen(args,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
        try:
            url=wait_ready(p,log);health=get(url+'/readyz');assert health['ready'] and len(health['build'])==64 and len(health['checkpoint'])==64;assert get(url+'/healthz')['ready']
            answer=ask(url);answers.append(answer['answers']);assert answer['usage'] is None
            ask(url,'MARKER-PRIVATE-STATE-do-not-log')
        finally:finish(p)
    rows=[json.loads(x) for x in metrics.read_text().splitlines()];assert rows[0]['kind']==1 and rows[0]['authenticate_us']>0
    requests=[r for r in rows if r['kind']==0];assert len(requests)==2
    for r in requests:
        assert r['status']==200 and r['questions']==3 and r['prefix_hits']==(2 if reuse else 0)
        assert r['reused_tokens']==(2*r['question_phases_us'][0]['prefix_tokens'] if reuse else 0)
        assert r['tokens']>r['prefix_tokens']>0 and len(r['question_phases_us'])==3
    assert 'MARKER-PRIVATE' not in metrics.read_text() and 'MARKER-PRIVATE' not in log.read_text()
assert answers[0]==answers[1]
for extra,word in ((['--fixture-bad-golden'],'golden'),(['--host-mib','1'],'limit_exceeded')):
    r=subprocess.run([str(binary),'--toy','--port','0']+extra,capture_output=True,text=True,timeout=120)
    assert r.returncode==1 and word in r.stderr and 'on http://' not in r.stdout
r=subprocess.run([str(supervisor),'--restart-limit','2','--backoff-ms','1','--',str(binary),'--toy','--port','0','--fixture-bad-golden'],capture_output=True,text=True,timeout=120)
assert r.returncode==1 and r.stderr.count('psionic-supervise: child=')==3 and r.stderr.count('restart=')==2
log=out/'supervised.log'
with log.open('w') as stream:
    p=subprocess.Popen([str(supervisor),'--backoff-ms','1','--',str(binary),'--toy','--port','0','--capacity','512'],stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
    try:
        url=wait_ready(p,log);assert get(url+'/readyz')['ready'];child=int(re.findall(r'child=(\d+)',log.read_text())[-1]);os.kill(child,signal.SIGKILL)
        url=wait_ready(p,log,after=1);assert get(url+'/readyz')['ready'];assert ask(url)['answers']==answers[0]
    finally:finish(p)
print('live operations: exact prefix parity, private usage/phases, readiness, failed promotion, budget refusal, bounded crash restart and clean shutdown pass')
