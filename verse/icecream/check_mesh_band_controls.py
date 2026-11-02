#!/usr/bin/env python3
"""Named producer-loss controls for the actual repaired Coax room geometry.

This is a scoped negative-control gate, not whole-library mutation coverage.
It compiles changed Coax copies normally and requires the native verifier to
execute and reject them. A compilation error never counts as a detected loss.
"""
from pathlib import Path
import argparse,hashlib,json,os,signal,subprocess,time
ROOT=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--qualification-dir',type=Path,required=True);p.add_argument('--work-dir',type=Path,required=True);a=p.parse_args();qdir=a.qualification_dir.resolve();work=a.work_dir.resolve();work.mkdir(parents=True,exist_ok=True)
 q=json.loads((qdir/'qualification.json').read_text());assert q['status']=='pass';coax=Path(os.environ.get('COAX',str(ROOT/'coax/bin/coax'))).resolve();assert sha(coax)==q['compiler_sha256'];assert all(sha(n)==h for n,h in q['source_sha256'].items());assert sha(qdir/'oracle.bin')==q['oracle_sha256']
 original=(HERE/'mesh.coax').read_text();loop=original[original.index('  for iwbt:0..<(middle-n0)/3 {'):original.index('  lower:=piece(v,middle,w,s0,s1,0,bottom,trim,0.05f32);')]
 caller="""      if open {
        ijpost:=ifx k==0 then ijposts[0..<ijleft] else ijposts[ijleft..<ijright];
        n=room_open_wall_bands(v,n,w,s0,s1,colour,trim,k==0,ijpost);
      } else {n = room_wall_bands(v, n, w, s0, s1, colour, trim);}"""
 cases=[
  ('caller-restores-overlap',caller,'      n = piece(v,n,w,s0,s1,0,WALL_HEIGHT,colour,0);\n      n = piece(v,n,w,s0,s1,0,0.28f32,trim,0.05f32);\n      n = piece(v,n,w,s0,s1,WALL_HEIGHT-0.22f32,WALL_HEIGHT,trim,0.05f32);','mesh'),
  ('seam-snap-omitted',loop,'','bands'),
  ('partial-triangle-guard','if iwbat+3<=v.count {','if iwbat<v.count {','bands'),
  ('upper-trim-colour','return piece(v,lower,w,s0,s1,top,WALL_HEIGHT,trim,0.05f32);','return piece(v,lower,w,s0,s1,top,WALL_HEIGHT,colour,0.05f32);','bands'),
  ('lower-trim-omitted','lower:=piece(v,middle,w,s0,s1,0,bottom,trim,0.05f32);','lower:=middle;','bands'),
  ('middle-extent-restored','middle:=piece(v,n0,w,s0,s1,bottom,top,colour,0);','middle:=piece(v,n0,w,s0,s1,0,WALL_HEIGHT,colour,0);','bands'),
  ('jamb-culling-omitted','  room_clip_jamb_caps(ijbands[0..<ijcount],w.horizontal,at_end,room_door_box(post));','','bands'),
  ('jamb-exposed-top-lowered','ijface[ijv].p.y=box.hi.y;','ijface[ijv].p.y=box.hi.y-0.01f32;','bands'),
  ('hidden-cap-retained','for ijv:0..<6 {ijface[ijv].p=ijpoint;}','for ijv:0..<6 {ijface[ijv].p=ijface[ijv].p;}','bands'),
  ('open-publish-partial-triangle','if ijn+3<=v.count {','if ijn<v.count {','bands'),
  ('wrong-actual-post-published','n = room_publish_triangles(v,n,ijposts[0..<ijleft]);','n = room_publish_triangles(v,n,ijposts[ijleft..<ijright]);','mesh'),
  ('exposed-overhang-culled','|| room_component(ijrect.hi,ijcross)>room_component(box.hi,ijcross)','|| false','coax:actual donor bounds preserve exposed trim overhang'),
  ('closed-box-winding-unchecked','if ijdet!=(ifx positive then 1 else -1) {return RoomDoorFace.{};}','','coax:emitted closed boxes and malformed donors'),
 ]
 records=[];started=time.monotonic();prior={}
 old_record=work/'controls.json'
 if old_record.exists():
  previous=json.loads(old_record.read_text());assert previous['compiler_sha256']==sha(coax)and previous['qualified_receipt_sha256']==sha(qdir/'qualification.json')
  preserved=work/('prior-'+sha(old_record)+'.json');preserved.write_bytes(old_record.read_bytes());prior={r['name']:r for r in previous['controls']if r['status']=='caught'}
 def run(args,log,expected=0):
  with log.open('wb')as f:
   child=subprocess.Popen(list(map(str,args)),cwd=ROOT,stdout=f,stderr=subprocess.STDOUT,start_new_session=True)
   try:code=child.wait(timeout=min(180,820-(time.monotonic()-started)))
   finally:
    if child.poll()is None:
     try:os.killpg(child.pid,signal.SIGKILL)
     except ProcessLookupError:pass
     child.wait()
  assert code in (expected if isinstance(expected,tuple)else(expected,)),(args,code,log)
  return dict(args=list(map(str,args)),exit=code,log=log.name,log_sha256=sha(log))
 try:
  for name,old,new,verifier in cases:
   assert original.count(old)==1,(name,'control site drift');d=work/name;d.mkdir(exist_ok=True)
   if name in prior and hashlib.sha256(original.replace(old,new).encode()).hexdigest()==prior[name]['source_sha256']:
    retained=prior[name];assert sha(d/'verse/icecream/mesh.coax')==retained['source_sha256'];assert sha(d/'IceCream.c')==retained['generated_c_sha256'];assert sha(d/'IceCream.h')==retained['header_sha256'];assert retained['executable_sha256'] is None or sha(d/'control')==retained['executable_sha256'];assert all(sha(d/c['log'])==c['log_sha256']for c in retained['commands']);records.append(retained);continue
   for s,h in q['source_sha256'].items():
    source=Path(s)
    if source.suffix=='.coax':
     dest=d/source.relative_to(ROOT);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(source.read_bytes())
   changed=d/'verse/icecream/mesh.coax';changed.write_text(original.replace(old,new));entry=d/'verse/icecream/mesh-band-native.coax';commands=[]
   commands.append(run([coax,'c',entry,'--library','--header',d/'IceCream.h'],d/'IceCream.c'))
   if verifier.startswith('coax:'):
    commands.append(run([coax,'test',changed,'--library','--only',verifier.split(':',1)[1]],d/'execution.log',1))
    output=(d/'execution.log').read_text();assert 'error[E0406]: assertion failed'in output and 'error[E0400]'not in output
    executable_hash=None
   else:
    driver=HERE/('mesh-check.c'if verifier=='mesh'else'mesh-band-native.c');exe=d/'control';commands.append(run([os.environ.get('CC','/usr/bin/clang'),'-std=c11','-O2','-ffp-contract=off','-I',d,d/'IceCream.c',driver,'-lm','-o',exe],d/'compile.log'))
    args=[exe,qdir/'oracle.bin']if verifier=='mesh'else[exe];bounds=name in ('partial-triangle-guard','open-publish-partial-triangle');commands.append(run(args,d/'execution.log',(1,-6)if bounds else 1))
    output=(d/'execution.log').read_text();assert ('index out of bounds'in output)if bounds else(('band check failed'in output)if verifier=='bands'else('0 failures'not in output and'FAIL house'in output))
    executable_hash=sha(exe)
   records.append(dict(name=name,status='caught',verifier=verifier,source_sha256=sha(changed),generated_c_sha256=sha(d/'IceCream.c'),header_sha256=sha(d/'IceCream.h'),executable_sha256=executable_hash,commands=commands))
 finally:
  changes=[n for n,h in q['source_sha256'].items()if sha(n)!=h]
  (work/'controls.json').write_text(json.dumps(dict(schema='aethyr.icecream.mesh-bands.controls.v1',status='pass'if len(records)==len(cases)and not changes else'failed',scope=f'{len(cases)} named actual-Coax producer controls; successful normal C emission required before native or focused Coax assertion rejection. Not whole-file zero-survivor mutation.',compiler_sha256=sha(coax),tool_sha256=sha(Path(__file__)),qualified_receipt_sha256=sha(qdir/'qualification.json'),controls=records,source_changes=changes,elapsed_seconds=time.monotonic()-started),indent=2)+'\n')
 assert not changes
 print(json.dumps(dict(status='pass',caught=len(records))))
if __name__=='__main__':main()
