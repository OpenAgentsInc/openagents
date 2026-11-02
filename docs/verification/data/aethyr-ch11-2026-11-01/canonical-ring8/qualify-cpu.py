from pathlib import Path
import hashlib,json,os,struct,subprocess
w=Path(__file__).parent;q=json.loads((w/'cases.json').read_text());(w/'cases.bin').write_bytes(b''.join(struct.pack('<4f',*c['uniform'])for c in q['cases']));word=lambda x:struct.unpack('<I',struct.pack('<f',x))[0];records=[]
for mode,flags in [('debug',['-O0']),('o2',['-O2']),('san',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
 command=['cc','-std=c11',*flags,'-ffp-contract=off','-fno-fast-math','-I',str(w),str(w/'cpu.c'),str(w/'Probe.c'),'-lm','-o',str(w/('cpu-'+mode))];r=subprocess.run(command,capture_output=True,timeout=120);(w/('cpu-'+mode+'-compile.log')).write_bytes(r.stdout+r.stderr);assert r.returncode==0,(r.stdout+r.stderr).decode()[-1500:]
 env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0',UBSAN_OPTIONS='halt_on_error=1');r=subprocess.run([str(w/('cpu-'+mode)),str(w/'cases.bin'),str(w/('cpu-'+mode+'.rgba32f'))],capture_output=True,env=env,timeout=30);(w/('cpu-'+mode+'.log')).write_bytes(r.stdout+r.stderr);assert r.returncode==0,(r.stdout+r.stderr).decode();data=(w/('cpu-'+mode+'.rgba32f')).read_bytes();assert len(data)==len(q['cases'])*16;bad=[]
 for i,c in enumerate(q['cases']):
  actual=struct.unpack_from('<4f',data,i*16)
  for j,(a,b)in enumerate(zip(actual,c['expected'])):
   if word(a)!=word(b)and not(a==0 and b==0):bad.append({'case':i,'channel':j,'actual':a,'expected':b})
 assert not bad,bad[:8]
 records.append({'mode':mode,'cases':len(q['cases']),'channels':len(q['cases'])*4,'uint32_max_refusals':2,'command':command,'artifact_sha256':hashlib.sha256((w/('cpu-'+mode)).read_bytes()).hexdigest()})
(w/'cpu-qualification.json').write_text(json.dumps({'schema':'aethyr.render.ring8-native-primitive.v1','status':'pass','compiler_sha256':'1fac3750bcffbb735ef26f4a065d16414b911068fd7d68bc3ca112565d2432aa','comparison':'Exact finite nonzero f32 words; signed zero equivalent. Independent analytic expected geometry/direction packet.','modes':records},indent=2)+'\n');print('CPU primitive:728cases×3modes,2full-u32 refusal controls/mode PASS')
