import concurrent.futures,json,os,re,socket,subprocess,sys,time,urllib.request,urllib.error
from pathlib import Path
binary,model,table,out=map(Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True);log=out/'server.log';metrics=out/'metrics.jsonl';record={'schema':'psionic-device-operations-v1'}
with log.open('w') as stream:
 p=subprocess.Popen([str(binary),'--elephant-metal',str(model),'--table',str(table),'--port','0','--capacity','512','--metrics',str(metrics)],stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
 try:
  deadline=time.monotonic()+120
  while time.monotonic()<deadline:
   ports=re.findall(r'on http://127\.0\.0\.1:(\d+)/v1/systemone',log.read_text())
   if ports:break
   assert p.poll() is None,log.read_text()[-1000:];time.sleep(.05)
  else:raise AssertionError('startup timeout')
  port=int(ports[-1]);url=f'http://127.0.0.1:{port}'
  def body(repeat=1,q=1):return {'model':'elephant-1','state':' '.join(['Water is wet.']*repeat),'questions':{str(i):{'type':'choice','instructions':'Is the water wet?','criteria':{'yes':None,'no':None}} for i in range(q)}}
  def post(b):
   with urllib.request.urlopen(urllib.request.Request(url+'/v1/systemone',json.dumps(b).encode()),timeout=30) as response:return json.load(response)
  with urllib.request.urlopen(url+'/readyz') as r:record['health']=json.load(r)
  with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
   first=pool.submit(post,body(80,3));time.sleep(.03);started=time.monotonic()
   try:post(body());raise AssertionError('a second job was queued')
   except urllib.error.HTTPError as e:assert e.code==503 and json.load(e)['error']['code']=='busy'
   record['busy_response_ms']=(time.monotonic()-started)*1000;assert not first.done();record['primary_after_busy']=first.result()
  for _ in range(2):post(body(32,3))
  data=json.dumps(body(80,3)).encode();s=socket.create_connection(('127.0.0.1',port));s.sendall(b'POST /v1/systemone HTTP/1.1\r\nContent-Length: '+str(len(data)).encode()+b'\r\n\r\n'+data);time.sleep(.03);s.close();time.sleep(.2)
  for i in range(30):
   try:answer=post(body());break
   except urllib.error.HTTPError as e:assert e.code==503;time.sleep(.05)
  else:raise AssertionError('cancelled slot did not clear')
  assert abs(answer['answers']['0']['probabilities']['yes']-.757196285)<.0005;record['answer_after_cancel']=answer
 finally:p.terminate();assert p.wait(timeout=10)==0
rows=[json.loads(x) for x in metrics.read_text().splitlines()];assert any(r['kind']==3 for r in rows) and any(r['kind']==2 for r in rows)
shared=[r for r in rows if r['status']==200 and r['questions']==3];assert len(shared)>=3 and all(r['prefix_hits']==2 and r['question_phases_us'][0]['prefix_hit']==0 for r in shared)
record['success']=True;record['prefix_hit_rows']=[r['prefix_hits'] for r in shared];(out/'operations.json').write_text(json.dumps(record,indent=2)+'\n');print('actual Metal original model: busy refusal before active work completes, per-request forks, cancellation and subsequent golden answer pass')
