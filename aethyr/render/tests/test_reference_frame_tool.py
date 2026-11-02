#!/usr/bin/env python3
"""Backend receipt failures must refuse before a renderer executable is reused."""
import json
from pathlib import Path
import tempfile
import unittest
from check_reference_frame import admitted_backend,frame_config_paths,snapshot_frame_config,reuse_native_executable
from check_pipeline import sha

class BackendReceipts(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name);self.backend=self.root/'backend';self.backend.mkdir();self.entry='aethyr/render/tests/reference-frame-native.coax';p=self.root/self.entry;p.parent.mkdir(parents=True);p.write_text('checked Coax source fixture')
  self.record=dict(status='rendered',compiler_sha256='compiler',compiler_source_digest='digest',compiler_changed=False,compiler_sources_match=True,source_changes=[],source_sha256={self.entry:sha(p),'tool.py':'old-tool'},artifact_changes=[])
  for key,name in [('generated_c','reference-frame.c'),('generated_header','reference-frame.h'),('executable','reference-frame')]:
   p=self.backend/name;p.write_text('completed '+key);self.record[key+'_sha256']=sha(p)
  self.save()
 def save(self):(self.backend/'measurement.json').write_text(json.dumps(self.record))
 def load(self,compiler='compiler',digest='digest'):return admitted_backend(self.backend,compiler,digest,self.root)
 def test_completed_artifacts_retain_exact_coax_closure_across_tool_changes(self):
  old,sources,artifacts=self.load();self.assertEqual(sources,{self.entry:self.record['source_sha256'][self.entry]});self.assertEqual(artifacts['executable'],(self.backend/'reference-frame').resolve());self.assertEqual(old['source_sha256']['tool.py'],'old-tool')
 def test_incomplete_or_changed_receipts_do_not_authorize_reuse(self):
  for field,value in [('status','inputs-changed'),('status','prepared'),('source_changes',['a.coax']),('compiler_changed',True),('artifact_changes',['header']),('input_changes',['u.bin'])]:
   with self.subTest(field=field,value=value):
    original=self.record.get(field);self.record[field]=value;self.save()
    with self.assertRaises(ValueError):self.load()
    self.record[field]=original
  self.save()
 def test_source_drift_and_other_entries_are_rejected(self):
  (self.root/self.entry).write_text('new Coax source')
  with self.assertRaisesRegex(ValueError,'Coax source changed'):self.load()
  self.record['source_sha256']={'other.coax':'unused'};self.save()
  with self.assertRaisesRegex(ValueError,'another Coax entry'):self.load()
 def test_compiler_and_every_artifact_identity_are_checked(self):
  with self.assertRaises(ValueError):self.load(compiler='other')
  with self.assertRaises(ValueError):self.load(digest='other')
  self.record['compiler_sources_match']=False;self.save()
  with self.assertRaises(ValueError):self.load()
  self.record['compiler_sources_match']=True;self.save()
  for name in ['reference-frame.c','reference-frame.h','reference-frame']:
   with self.subTest(name=name):
    p=self.backend/name;original=p.read_bytes();p.write_bytes(original+b' changed')
    with self.assertRaisesRegex(ValueError,'artifact changed'):self.load()
    p.write_bytes(original)


class NativeProfiles(unittest.TestCase):
 def setUp(self):
  self.before={'aethyr/render/tests/reference-frame-native.c':'driver','aethyr/render/tests/frame-input.h':'loader'}
  self.old={'native_mode':'debug','c_mode':'checked','source_sha256':dict(self.before)}
 def test_checked_emission_rebuilds_for_sanitizers_and_other_compiler_flags(self):
  self.assertTrue(reuse_native_executable(self.old,'debug',self.before))
  self.assertFalse(reuse_native_executable(self.old,'sanitized',self.before))
  self.assertFalse(reuse_native_executable(self.old,'sanitized-o1',self.before))
  self.assertFalse(reuse_native_executable(self.old,'checked-o2',self.before))
  self.before['aethyr/render/tests/frame-input.h']='new loader'
  self.assertFalse(reuse_native_executable(self.old,'debug',self.before))
 def test_checked_and_wrapping_emissions_cannot_cross_profiles(self):
  with self.assertRaisesRegex(ValueError,'checked/wrapping'):reuse_native_executable(self.old,'release',self.before)
  self.old.update(native_mode='release',c_mode='wrapping')
  self.assertTrue(reuse_native_executable(self.old,'release',self.before))
  for mode in ['debug','sanitized','sanitized-o1','checked-o2']:
   with self.subTest(mode=mode),self.assertRaisesRegex(ValueError,'checked/wrapping'):reuse_native_executable(self.old,mode,self.before)
 def test_historical_checked_o2_identity_and_missing_driver_identity(self):
  self.old.pop('native_mode');self.old.pop('c_mode')
  self.assertTrue(reuse_native_executable(self.old,'checked-o2',self.before))
  self.old['source_sha256'].pop('aethyr/render/tests/reference-frame-native.c')
  self.assertFalse(reuse_native_executable(self.old,'checked-o2',self.before))


class RecordedTransport(unittest.TestCase):
 def test_snapshot_preserves_names_counts_and_every_texture_media_payload(self):
  with tempfile.TemporaryDirectory()as temp:
   root=Path(temp);inputs=root/'inputs';inputs.mkdir();paths=[]
   for name in ['u.bin','hud.bin','draws.bin','render.json','stage.tree','texture.bin','media.txt']:
    path=root/name;path.write_bytes(name.encode());paths.append(path)
   u,hud,draws,package,tree,texture,media=paths
   text=f"17 320 180 8070 16 0 0 0 0 0 0 0 0\n{u}\n{hud}\n{draws}\n-\n{package}\n1\n12 {tree}\n1\nu.bin {u} 4 4 4 0 4 1\ntextures 1\nsans_atlas rgba8unorm 1 2 {texture}\n2 2 0 8 16\n1 1 16 4 4\nmedia_text {media}\nmedia_images 1\n1 /same 1 2 2\n"
   source=root/'source.config';source.write_text(text);out=root/'out.config';files,pkg,trees,capture,counts=snapshot_frame_config(source,package,inputs,out)
   rows,refs,observed,indices=frame_config_paths(out.read_text());self.assertEqual(observed,counts);self.assertEqual(counts[3],8070);self.assertEqual(rows[9][0],'u.bin');self.assertEqual(rows[11][:4],['sans_atlas','rgba8unorm','1','2']);self.assertEqual(rows[12],['2','2','0','8','16']);self.assertEqual(capture.read_text(),text);self.assertEqual(pkg.read_bytes(),package.read_bytes());self.assertEqual(trees[0][0],12)
   for original,dest in files.items():self.assertEqual(Path(original).read_bytes(),dest.read_bytes())
 def test_shared_rfi_whitespace_framing_preserves_every_observation(self):
  text='0 80 120 3 16 1 0 0 0 0 0 0 0\nu.bin\nhud.bin\ndraw.bin\nindex.bin\nrender.json\n1\n5 stage.tree\n1\ns0.bin data.bin 16 12 16 4 16 1\ntextures 1\nmedia_image-0 rgba8unorm 1 1 image.bin\n2 1 0 8 8\nmedia_text text.bin\nmedia_images 1\n1 /image 1 2 1\n'
  expected=frame_config_paths(text);staged='\n'.join(text.split())+'\n';self.assertEqual(frame_config_paths(staged),expected)
  compact=' '.join(text.split());self.assertEqual(frame_config_paths(compact),expected)
 def test_truncation_unknown_suffix_and_wrong_requested_package_refuse(self):
  with self.assertRaises(ValueError):frame_config_paths('0 1 2')
  base='0 2 2 0 16 0 0 0 0 0 0 0 0\n-\n-\n-\n-\n-\n0\n0\n'
  with self.assertRaises(ValueError):frame_config_paths(base+'textures 1\n')
  with self.assertRaises(ValueError):frame_config_paths(base+'unknown value\n')
  with tempfile.TemporaryDirectory()as temp:
   root=Path(temp);a=root/'a.json';b=root/'b.json';a.write_text('a');b.write_text('b');config=root/'frame.config';config.write_text(base.replace('\n-\n0\n0',f'\n{a}\n0\n0'))
   with self.assertRaisesRegex(ValueError,'package differs'):snapshot_frame_config(config,b,root,root/'out')

if __name__=='__main__':unittest.main()
