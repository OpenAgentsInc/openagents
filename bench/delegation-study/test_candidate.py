import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

import candidate


class CandidateTests(unittest.TestCase):
    def test_deletion_identity_binds_which_file_was_deleted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            with tarfile.open(root/'candidate.tar.gz','w:gz'):pass
            before={'kind':'file','sha256':'1'*64,'bytes':1,'mode':0o644}
            changes={'a.rs':{'before':before,'after':None}}
            (root/'changes.json').write_text(json.dumps(changes))
            first,payload=candidate.write_manifest(root,'1'*40,'2'*64,changes)
            changes={'b.rs':{'before':before,'after':None}}
            (root/'changes.json').write_text(json.dumps(changes))
            second,same_payload=candidate.write_manifest(root,'1'*40,'2'*64,changes)
            self.assertEqual(payload,same_payload)
            self.assertNotEqual(first,second)

    def test_payload_and_sidecar_must_match_manifest(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);body=b'base fix\n'
            with tarfile.open(root/'candidate.tar.gz','w:gz') as archive:
                info=tarfile.TarInfo('source.rs');info.mode=0o644;info.size=len(body);archive.addfile(info,io.BytesIO(body))
            changes={'source.rs':{'before':None,'after':{'kind':'file','sha256':hashlib.sha256(body).hexdigest(),'bytes':len(body),'mode':0o644}}}
            (root/'changes.json').write_text(json.dumps(changes))
            identity,_=candidate.write_manifest(root,'1'*40,'2'*64,changes)
            candidate.validate(root,identity,'1'*40,'2'*64)
            (root/'changes.json').write_text('{}')
            with self.assertRaises(ValueError):candidate.validate(root,identity)

    def test_validation_respects_shared_capture_deadline(self):
        from capture_limits import CaptureLimit
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            with tarfile.open(root/'candidate.tar.gz','w:gz'):pass
            (root/'changes.json').write_text('{}')
            identity,_=candidate.write_manifest(root,'1'*40,'2'*64,{})
            with self.assertRaises(CaptureLimit):candidate.validate(root,identity,deadline=0)


if __name__=='__main__':unittest.main()
