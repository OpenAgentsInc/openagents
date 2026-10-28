"""Independent official protoc interoperability and original content parity.
Tooling only; run in the pinned proto/tools environment. No code from the
source repo is imported or executed. The native binary is ASan/UBSan built.
"""
import json, math, os, struct, subprocess, sys, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
protoc=os.environ['PROTOC']
with tempfile.TemporaryDirectory(prefix='haunt-oracle-') as temporary:
 subprocess.run([protoc,'-I'+str(ROOT/'proto'),'--python_out='+temporary,'openagents/metareal/haunt/v1alpha1/haunt.proto'],check=True)
 sys.path.insert(0,temporary)
 from openagents.metareal.haunt.v1alpha1.haunt_pb2 import Resource,Save
 resources=ROOT/'apps/openagents-ios/OpenAgents/Resources'
 r=Resource.FromString((resources/'Haunt.pb').read_bytes())
 source=json.loads((ROOT/'verse/haunt/house.json').read_text());capture=json.loads((ROOT/'verse/haunt/compiled.json').read_text())
 entities={e.id:e for e in r.entities}
 assert len(source['objects'])==120 and len(source['doors'])==21
 for item in source['objects']:
  if not item.get('mesh'):continue
  e=entities[item['id']];assert r.meshes[e.mesh].name==item['mesh'],(item['id'],r.meshes[e.mesh].name,item['mesh'])
  for k in ['x','y','z']:assert abs(getattr(e.position,k)-item.get(k,0))<0.00001,(item['id'],k)
  assert abs(e.yaw-item.get('yaw',0)*math.pi/180)<0.00001
  assert abs(e.scale-item.get('scale',1))<0.00001
 for door in source['doors']:assert entities[door['id']].motion==5
 assert sum(a.room for a in r.areas)==13
 assert [a.id for a in r.areas]==[a['id'] for a in source['areas']]
 assert len(r.zones)==9 and len(r.people)==4 and len(r.lights)==26
 assert r.meshes[entities['mortimer'].mesh].name=='Custom_Fig_Caretaker'
 assert r.meshes[entities['agatha'].mesh].name=='Custom_Fig_Witch'
 assert len([e for e in r.entities if e.motion==6])==len(capture['trees'])
 assert all(r.meshes[e.mesh].name in ('Custom_Tree_Pine','Custom_Tree_Dead') for e in r.entities if e.motion==6)
 assert len((resources/'Haunt.vertices').read_bytes())==r.vertex_count*48
 assert len((resources/'Haunt.indices').read_bytes())==r.index_count*4
 saved=Save.FromString(Path('/tmp/haunt-save-oracle.pb').read_bytes())
 assert saved.version==1 and saved.world_id=='haunted-house' and saved.fired_zones==511
 assert len(saved.people)==5 and saved.people[-1].name=='Ada' and saved.people[-1].brain=='follower'
 assert saved.people[0].knowledge=='The key is in the kitchen.'
 assert not saved.zones[0].once and len(saved.zones)==9
 saved.title='Official protoc';saved.feedback='Independent writer';saved.instance_id='oracle-world'
 oracle=Path(temporary)/'official.pb';oracle.write_bytes(saved.SerializeToString(deterministic=True))
 subprocess.run([sys.argv[1],str(resources),str(oracle)],check=True)
 print('Original 120 objects, 21 doors, 13 rooms, forest and actors: parity passed')
 print('Official protoc: Coax Save -> protobuf -> official writer -> Coax restore passed')
