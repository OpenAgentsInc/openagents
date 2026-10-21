import concurrent.futures,json,pathlib,subprocess,time,urllib.request,urllib.error
root=pathlib.Path.home()
def props():
 data=subprocess.check_output(['systemctl','show','psionic-flint','-p','MainPID','-p','NRestarts'],text=True)
 return dict(s.split('=',1) for s in data.splitlines())
def call(prompt,max_tokens=512,greedy=True):
 d=dict(model='flint',messages=[dict(role='user',content=prompt)],max_tokens=max_tokens,seed=42)
 if greedy:d['temperature']=0
 r=urllib.request.Request('http://localhost:8080/v1/chat/completions',data=json.dumps(d).encode(),headers={'Content-Type':'application/json'})
 try:
  with urllib.request.urlopen(r,timeout=920) as q:return {'status':q.status,'body':json.load(q)}
 except urllib.error.HTTPError as e:return {'status':e.code,'body':json.load(e)}
 except Exception as e:return {'error':type(e).__name__,'detail':str(e)}
before=props();started=time.monotonic()
with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
 future=pool.submit(call,'Explain Rust ownership in detail, including several code examples.')
 time.sleep(.3)
 busy=call('Hello',16)
 assert busy.get('status')==429,busy
 subprocess.run(['sudo','systemctl','kill','--signal=KILL','psionic-flint'],check=True)
 interrupted=future.result()
 assert 'error' in interrupted,interrupted
for i in range(180):
 try:
  with urllib.request.urlopen('http://localhost:8080/healthz',timeout=2) as q:
   if q.status==200:break
 except Exception:pass
 time.sleep(1)
else:raise RuntimeError('service did not recover')
after=props();assert before['MainPID']!=after['MainPID'] and int(after['NRestarts'])>int(before['NRestarts'])
answer=call('What is 2 + 2? Reply with just the number.',128)
assert answer.get('status')==200 and answer['body']['choices'][0]['message']['content'].strip()=='4',answer
sampled=call('What is 2 + 2? Reply with just the number.',128,False)
assert sampled.get('status')==200 and sampled['body']['choices'][0]['finish_reason']=='stop' and sampled['body']['choices'][0]['message']['content'].strip()=='4',sampled
processes=subprocess.check_output(['nvidia-smi','--query-compute-apps=pid,process_name,used_gpu_memory','--format=csv,noheader'],text=True)
report=dict(before=before,after=after,interrupted=interrupted,busy=busy,recovery_seconds=time.monotonic()-started,answer=answer,sampled=sampled,gpu_processes=processes)
(root/'flint-recovery.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k not in ['answer','sampled']},indent=2))
