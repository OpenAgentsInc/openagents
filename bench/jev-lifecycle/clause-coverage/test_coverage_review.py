"""Synthetic source overlays exercise evidence admission before any paid call."""
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import coverage_review as review

HARNESS = Path(os.environ.get('CLAUSE_REVIEW_HARNESS', Path.cwd()))


class Fixture:
    def __init__(self, root):
        self.repo=Path(root)/'repo';self.repo.mkdir();self.candidate=Path(root)/'candidate';self.candidate.mkdir()
        self.files={'crates/toy/Cargo.toml':b'[package]\nname="toy"\nversion="0.1.0"\n',
                    'crates/toy/src/lib.rs':b'pub fn call(value: u32) -> u32 { api(value) }\nfn api(value: u32) -> u32 { value }\n',
                    'crates/toy/src/api.rs':b'pub fn receive(value: u32) -> u32 { value }\n',
                    'crates/toy/src/helpers.rs':b'fn hidden(value: u32) -> u32 { value }\n',
                    'crates/toy/tests/ordinary.rs':b'#[test] fn ordinary() {}\n',
                    'docs/contract.md':b'# Public contract\nPreserve input value.\n'}
        for name,raw in self.files.items():
            p=self.repo/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(raw)
        self.git('init','-q');self.git('-c','user.name=Synthetic','-c','user.email=synthetic@example.invalid','add','.')
        self.git('-c','user.name=Synthetic','-c','user.email=synthetic@example.invalid','commit','-qm','Synthetic fixture')
        self.commit=self.git('rev-parse','HEAD').decode().strip()
        self.ctx,self.gateway,self.api=review.modules(HARNESS)
        bindings=self.ctx.tree(self.repo,self.commit)
        rows=[]
        for name,raw in self.files.items():
            declarations=[]
            if name.endswith('.rs'):
                start=raw.find(b'pub fn')
                if start>=0:
                    end=raw.index(b'{',start)
                    declarations=[{'kind':'function_item','qualified_name':'sample','parse_has_error':False,
                                   'signature':{'start_byte':start,'end_byte':end}}]
            rows.append({'path':name,'blob':bindings[name],'size':len(raw),'sha256':review.sha(raw),'syntax':{'declarations':declarations}})
        self.index={'commit':self.commit,'syntax':{'extractor_version':'briefing-lab-rust-v1'},'files':rows}
        self.task={'id':'synthetic','title':'Preserve the public behavior','prompt':'Preserve the input value. Read the public contract.',
                   'source_commit':self.commit,'packages':['toy'],'allowed_paths':['crates/toy/'],
                   'required_public_readings':[{'path':'docs/contract.md','sha256':review.sha(self.files['docs/contract.md'])}]}

    def git(self,*args):
        return subprocess.check_output(['git','-C',str(self.repo),*args],stderr=subprocess.PIPE)

    def freeze_candidate(self,changes):
        metadata={}
        with tarfile.open(self.candidate/'candidate.tar.gz','w:gz') as archive:
            for name,raw in changes.items():
                before=self.files.get(name)
                meta=lambda value:None if value is None else {'kind':'file','bytes':len(value),'sha256':review.sha(value),'mode':0o644}
                metadata[name]={'before':meta(before),'after':meta(raw)}
                if raw is not None:
                    member=tarfile.TarInfo(name);member.size=len(raw);member.mode=0o644;archive.addfile(member,io.BytesIO(raw))
        (self.candidate/'changes.json').write_text(json.dumps(metadata))
        identity,_=self.api.write_manifest(self.candidate,self.commit,'a'*64,metadata)
        return identity

    def prepare(self,changes,**kwargs):
        return review.prepare(HARNESS,self.repo,self.task,self.index,self.candidate,self.freeze_candidate(changes),'a'*64,**kwargs)


class CoverageTests(unittest.TestCase):
    def test_candidate_source_replaces_base_and_unchanged_entrypoint_is_complete(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);new=b'pub fn receive(value: u32) -> u32 { value.saturating_add(0) }\n'
            output=fixture.prepare({'crates/toy/src/api.rs':new})
            self.assertTrue(output['ready'],output['errors'])
            state=output['request']['state'];files={r['path']:r for r in state['current_source']}
            self.assertEqual(files['crates/toy/src/api.rs']['text'],new.decode())
            self.assertEqual(files['crates/toy/src/lib.rs']['text'],fixture.files['crates/toy/src/lib.rs'].decode())
            self.assertEqual(files['crates/toy/src/api.rs']['origin'],'candidate')
            self.assertEqual(files['crates/toy/src/lib.rs']['origin'],'pinned_base')
            self.assertTrue(all(r['complete_file'] and not r['truncated'] for r in files.values()))
            self.assertEqual(output['required_source_bytes'],len(new))
            self.assertEqual(output['request_bytes'],len(review.encoded(output['request'])))
            self.assertEqual(output['request_sha256'],review.sha(review.encoded(output['request'])))
            self.assertEqual(output['model_calls'],0)

    def test_required_contract_stays_pinned_when_candidate_edits_it(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory)
            output=fixture.prepare({'docs/contract.md':b'New candidate prose is not the specification.\n'})
            self.assertTrue(output['ready'],output['errors'])
            self.assertEqual(output['request']['state']['pinned_contracts'][0]['text'],fixture.files['docs/contract.md'].decode())

    def test_full_mandatory_budget_skips_without_truncation_or_model_call(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory)
            output=fixture.prepare({'crates/toy/src/api.rs':b'// '+b'x'*(review.MAX_REQUEST+1)+b'\n'})
            self.assertFalse(output['ready']);self.assertEqual(output['status'],'skipped_mandatory_request_bound')
            self.assertIsNone(output['request']);self.assertFalse(output['source_truncated']);self.assertEqual(output['model_calls'],0)
            self.assertGreater(output['mandatory_request_bytes'],review.MAX_REQUEST)

    def test_optional_budget_omits_whole_files(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory)
            initial=fixture.prepare({'crates/toy/src/api.rs':b'pub fn receive(value: u32) -> u32 { value }\n'})
            with patch.object(review,'MAX_REQUEST',initial['mandatory_request_bytes']+1050):
                output=fixture.prepare({'crates/toy/src/api.rs':b'pub fn receive(value: u32) -> u32 { value }\n'})
            self.assertTrue(output['ready'],output['errors'])
            self.assertTrue(any(r['status']=='optional_request_byte_bound' for r in output['omissions']))
            self.assertFalse(any(r['truncated'] for r in output['request']['state']['current_source']))

    def test_invalid_utf8_and_tampered_index_or_contract_cannot_be_ready(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory)
            self.assertFalse(fixture.prepare({'crates/toy/src/api.rs':b'\xff'})['ready'])
            fixture.index['files'][1]['sha256']='b'*64
            self.assertFalse(fixture.prepare({})['ready'])
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.task['required_public_readings'][0]['sha256']='b'*64
            self.assertFalse(fixture.prepare({})['ready'])

    def test_deleted_implementation_is_explicit_and_tampered_candidate_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);output=fixture.prepare({'crates/toy/src/api.rs':None})
            self.assertTrue(output['ready'],output['errors'])
            self.assertTrue(any(r['path']=='crates/toy/src/api.rs' and r['status']=='deleted_in_candidate' for r in output['source_catalog']))
            identity=fixture.freeze_candidate({});(fixture.candidate/'changes.json').write_text('{"tampered":{}}')
            rejected=review.prepare(HARNESS,fixture.repo,fixture.task,fixture.index,fixture.candidate,identity,'a'*64)
            self.assertFalse(rejected['ready']);self.assertEqual(rejected['model_calls'],0)

    def test_private_fields_never_enter_state_and_each_clause_has_four_options(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.task.update(checker='PRIVATE_CHECKER',accepted=True,arm='SECRET_ARM',reference_patch='PRIVATE_FIX')
            output=fixture.prepare({});state=output['request']['state'];questions=output['request']['questions']
            self.assertEqual(state['clauses'],{'r01':'Preserve the input value.','r02':'Read the public contract.'})
            self.assertEqual(set(questions),set(state['clauses']))
            for key,question in questions.items():
                self.assertIn(state['clauses'][key],question['instructions'])
                self.assertEqual(set(question['criteria']),{'demonstrated','missing_handling','insufficient_evidence','non_code_requirement'})
            raw=review.encoded(output['request']).decode()
            for private in ('PRIVATE_CHECKER','SECRET_ARM','PRIVATE_FIX','"accepted"'):
                self.assertNotIn(private,raw)


if __name__=='__main__':unittest.main()
