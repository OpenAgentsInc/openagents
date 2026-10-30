#!/usr/bin/env python3
"""Reproducible owned loopback workloads. Python is measurement tooling only."""
import argparse,hashlib,json,math,os,platform,re,subprocess,time,urllib.request
from pathlib import Path

def rank(values,p):return sorted(values)[(p*len(values)+99)//100-1]
def post(url,body):
    data=json.dumps(body,separators=(',',':')).encode();start=time.perf_counter_ns()
    with urllib.request.urlopen(urllib.request.Request(url+'/v1/systemone',data,{'Content-Type':'application/json'}),timeout=120) as r:
        assert r.status==200;answer=json.load(r)
    return (time.perf_counter_ns()-start)//1000,answer

def main():
    a=argparse.ArgumentParser();a.add_argument('--binary',required=True);a.add_argument('--engine',choices=['toy','elephant-gpu'],default='toy');a.add_argument('--device-mib',type=int);a.add_argument('--model-dir');a.add_argument('--table');a.add_argument('--out',required=True);a.add_argument('--samples',type=int,default=100);a.add_argument('--repeats',default='1,4,8');a.add_argument('--questions',type=int,default=1);a.add_argument('--legacy',action='store_true');a.add_argument('--no-prefix-reuse',action='store_true');a.add_argument('--startup-seconds',type=int,default=720);args=a.parse_args()
    assert 1<=args.samples<=4096 and 1<=args.questions<=64
    out=Path(args.out).resolve();out.mkdir(parents=True,exist_ok=True);metrics=out/'metrics.jsonl';log=out/'server.log';binary=str(Path(args.binary).resolve())
    command=[binary,'--'+args.engine]+([] if args.engine=='toy' else [args.model_dir,'--table',args.table])+['--port','0','--capacity','512']
    if args.device_mib:command+=['--device-mib',str(args.device_mib)]
    if not args.legacy:command+=['--metrics',str(metrics)]
    if args.no_prefix_reuse:command+=['--no-prefix-reuse']
    record={'schema':'psionic-decision-benchmark-v1','binary_sha256':hashlib.sha256(Path(binary).read_bytes()).hexdigest(),'command':command,'platform':platform.platform(),'load_at_start':os.getloadavg(),'samples_per_workload':args.samples,'questions':args.questions,'workloads':[],'limitations':['Serial loopback samples on a shared host; no quiet-host or fleet SLA claim.','P50/P95/P99 use nearest ranks. Request time ends after reading the complete response body.','CPU prefix timing includes its discarded readout; CPU suffix includes readout. Device model time is joint prefix/suffix and includes its GPU head.','Tokens per second are logical input rate; reused prefix tokens are reported separately.']}
    process=None
    try:
        started=time.monotonic();stream=log.open('w');process=subprocess.Popen(command,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
        while time.monotonic()-started<args.startup_seconds:
            text=log.read_text();ports=re.findall(r'on http://127\.0\.0\.1:(\d+)/v1/systemone',text)
            if ports:break
            if process.poll() is not None:raise AssertionError('server failed before readiness: '+text[-1500:])
            time.sleep(.1)
        else:raise TimeoutError('server startup exceeded its bounded window')
        status=Path('/proc')/str(process.pid)/'status'
        if status.exists():record['process_memory_at_ready']={line.split(':',1)[0]:line.split(':',1)[1].strip() for line in status.read_text().splitlines() if line.startswith(('VmSize:','VmRSS:','VmHWM:'))}
        record['startup_wall_seconds']=time.monotonic()-started;url='http://127.0.0.1:'+ports[-1]
        if not args.legacy:
            with urllib.request.urlopen(url+'/readyz',timeout=5) as r:record['ready']=json.load(r)
            assert record['ready']['ready']
        model='psionic-toy' if args.engine=='toy' else 'elephant-1'
        for repeat in map(int,args.repeats.split(',')):
            body={'model':model,'state':' '.join(['Water is wet.']*repeat),'questions':{str(i):{'type':'choice','instructions':'Is the water wet?','criteria':{'yes':None,'no':None}} for i in range(args.questions)}}
            _,expected=post(url,body)
            if repeat==1:
                want=.437947251 if model=='psionic-toy' else .757196285
                assert abs(expected['answers']['0']['probabilities']['yes']-want)<(1e-8 if model=='psionic-toy' else .0005)
            observations=[]
            for _ in range(args.samples):
                us,result=post(url,body);assert result['answers']==expected['answers'] and result['service']==expected['service'];observations.append(us)
            row={'repeat':repeat,'request':body,'request_sha256':hashlib.sha256(json.dumps(body,sort_keys=True,separators=(',',':')).encode()).hexdigest(),'retained_answer':expected['answers'],'identity':expected['service']['identity'],'client_us':observations,'p50_us':rank(observations,50),'p95_us':rank(observations,95),'p99_us':rank(observations,99)}
            record['workloads'].append(row);(out/'benchmark.json').write_text(json.dumps(record,indent=2)+'\n')
        record['load_at_end']=os.getloadavg()
    finally:
        if process:
            process.terminate()
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
        if metrics.exists():
            rows=[json.loads(x) for x in metrics.read_text().splitlines()];record['startup']=[r for r in rows if r['kind']==1]
            good=[r for r in rows if r['kind']==0 and r['status']==200];at=0
            for workload in record['workloads']:
                batch=good[at+1:at+1+args.samples];at+=1+args.samples
                assert len(batch)==args.samples
                workload['observed_tokens']=batch[0]['tokens'];workload['reused_tokens']=batch[0]['reused_tokens'];workload['logical_input_tokens_per_second']=batch[0]['tokens']*1e6/workload['p50_us']
        (out/'benchmark.json').write_text(json.dumps(record,indent=2)+'\n')
    for w in record['workloads']:print(f"repeat={w['repeat']} questions={args.questions}: p50={w['p50_us']/1000:.3f} p95={w['p95_us']/1000:.3f} p99={w['p99_us']/1000:.3f} ms")
if __name__=='__main__':main()
