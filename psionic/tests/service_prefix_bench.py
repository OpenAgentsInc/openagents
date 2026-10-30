#!/usr/bin/env python3
"""Paired CPU prefix measurements; two owned toy servers, one active call at a time."""
import argparse,hashlib,json,os,re,subprocess,time,urllib.request
from pathlib import Path

def rank(xs,p):return sorted(xs)[(len(xs)*p+99)//100-1]
def post(url,body):
    started=time.perf_counter_ns()
    with urllib.request.urlopen(urllib.request.Request(url+'/v1/systemone',json.dumps(body).encode()),timeout=120) as response:answer=json.load(response)
    return (time.perf_counter_ns()-started)//1000,answer

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--binary',required=True);parser.add_argument('--out',required=True);parser.add_argument('--samples',type=int,default=100);a=parser.parse_args();assert 1<=a.samples<=4096
    out=Path(a.out).resolve();out.mkdir(parents=True,exist_ok=True);binary=Path(a.binary).resolve();record={'schema':'psionic-prefix-pairs-v1','binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'load_at_start':os.getloadavg(),'workloads':[],'limitations':['Owned toy CPU model; no Elephant throughput claim.','Alternating pair order reduces batch-order bias, but this remains a shared-host observation.','Nearest-rank percentiles; all raw pairs and metadata are retained.']};processes=[];urls=[]
    try:
        for reuse in (False,True):
            log=out/('after.log' if reuse else 'before.log');metrics=out/('after.jsonl' if reuse else 'before.jsonl');stream=log.open('w')
            cmd=[str(binary),'--toy','--capacity','512','--port','0','--metrics',str(metrics)]+([] if reuse else ['--no-prefix-reuse'])
            p=subprocess.Popen(cmd,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True);processes.append(p);deadline=time.monotonic()+120
            while time.monotonic()<deadline:
                ports=re.findall(r'on http://127\.0\.0\.1:(\d+)/v1/systemone',log.read_text())
                if ports:urls.append('http://127.0.0.1:'+ports[-1]);break
                assert p.poll() is None,log.read_text();time.sleep(.02)
            else:raise AssertionError('warmup timeout')
        for repeat in (1,4,8):
            q={'type':'choice','instructions':'Is the water wet?','criteria':{'yes':None,'no':None}}
            body={'model':'psionic-toy','state':' '.join(['Water is wet.']*repeat),'questions':{str(i):q for i in range(3)}}
            expected=post(urls[0],body)[1];assert post(urls[1],body)[1]['answers']==expected['answers'];pairs=[]
            for i in range(a.samples):
                times=[0,0]
                for which in ((0,1) if i%2==0 else (1,0)):
                    times[which],answer=post(urls[which],body);assert answer['answers']==expected['answers'] and answer['service']==expected['service']
                pairs.append({'before_us':times[0],'after_us':times[1],'first':'before' if i%2==0 else 'after'})
            row={'repeat':repeat,'request':body,'retained_answer':expected['answers'],'identity':expected['service']['identity'],'pairs':pairs}
            for name in ('before','after'):
                xs=[x[name+'_us'] for x in pairs];row[name]={f'p{p}_us':rank(xs,p) for p in (50,95,99)}
            record['workloads'].append(row);(out/'pairs.json').write_text(json.dumps(record,indent=2)+'\n');print(repeat,row['before'],row['after'],flush=True)
        record['load_at_end']=os.getloadavg()
    finally:
        for p in processes:
            p.terminate()
            try:p.wait(timeout=10)
            except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
        (out/'pairs.json').write_text(json.dumps(record,indent=2)+'\n')
if __name__=='__main__':main()
