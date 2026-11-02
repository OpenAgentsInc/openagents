#!/usr/bin/env python3
"""Tool-only launch controls: every refused freeze leaves the train spy idle."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock

_spec=importlib.util.spec_from_file_location('residual_launch',Path(__file__).with_name('residual_launch.py'))
launch=importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(launch)

class FreezeLaunchTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.root=Path(self.tmp.name).resolve()
        def make(name,body=b'pinned'):
            p=self.root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(body);return launch.fact(p)
        compiler=make('compiler/coax');cc=make('compiler/cc')
        provenance=make('compiler/provenance.record',('coax-provenance-v1\nfingerprint\n'+compiler['sha256']+'\nsource\n').encode())
        data=[make('data/'+n) for n in launch.DATA_FILES]
        native=[make('native/'+n) for n in ('residual.c','residual.h','residual')]
        self.freeze=self.root/'freeze.json'
        self.freeze.write_text(json.dumps(dict(compiler=compiler,compiler_provenance=provenance,
            experiment=make('experiment.json'),inputs=[make('source/model.coax'),compiler,cc],
            dataset=data,native_release=native,preflight={'verified':True})))
        self.sha=launch.fact(self.freeze)['sha256'];self.cc=Path(cc['path']);self.spy=Mock(return_value=Mock(returncode=0))
    def tearDown(self):self.tmp.cleanup()
    def invoke(self):return launch.run_frozen(self.freeze,self.sha,self.root,self.root/'new-run',self.cc,execute=self.spy)
    def test_valid_complete_freeze_invokes_only_its_binary_data_and_manifest(self):
        self.assertEqual(self.invoke(),0);self.spy.assert_called_once();argv=self.spy.call_args.args[0]
        self.assertEqual(argv,[str(self.root/'native/residual'),'--train',str(self.root/'data'),str(self.root/'experiment.json'),str(self.root/'new-run')])
        self.assertEqual(self.spy.call_args.kwargs['timeout'],600)
    def test_missing_each_required_artifact_never_invokes_train(self):
        for name in ['compiler/coax','compiler/cc','compiler/provenance.record','native/residual.c','native/residual.h','native/residual','experiment.json','source/model.coax',*('data/'+n for n in launch.DATA_FILES)]:
            with self.subTest(name=name):
                p=self.root/name;body=p.read_bytes();p.unlink()
                with self.assertRaises((ValueError,OSError)):self.invoke()
                p.write_bytes(body);self.spy.assert_not_called()
    def test_drifted_source_and_freeze_never_invoke_train(self):
        p=self.root/'source/model.coax';p.write_bytes(b'drift')
        with self.assertRaises(ValueError):self.invoke()
        self.spy.assert_not_called()
        with self.assertRaises(ValueError):launch.run_frozen(self.freeze,'0'*64,self.root,self.root/'new-run',self.cc,execute=self.spy)
        self.spy.assert_not_called()
    def test_missing_provenance_record_or_native_inventory_never_invokes_train(self):
        data=json.loads(self.freeze.read_text())
        for key in ('compiler_provenance','native_release'):
            broken=dict(data);broken.pop(key);self.freeze.write_text(json.dumps(broken));self.sha=launch.fact(self.freeze)['sha256']
            with self.assertRaises((ValueError,KeyError)):self.invoke()
            self.spy.assert_not_called()

if __name__=='__main__':unittest.main()
