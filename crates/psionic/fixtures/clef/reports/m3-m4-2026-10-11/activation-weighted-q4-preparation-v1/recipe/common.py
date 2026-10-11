"""Pinned checkpoint and request helpers shared by scratch preparation tools."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys

HERE=Path(__file__).resolve().parent
BASE=HERE.parent
ROOT=Path('<OPENAGENTS_WORKTREE>')
CHECKPOINT=BASE/'artifacts/clef-bf16'
PRIOR_IDENTITY=HERE/'checkpoint-identity.json'
PRIOR_IDENTITY_SHA='b610c69aa78d3f8ff1811a6922bd6775c150ed7657e5224f2c0a7a01c0f3b506'
REVISION='0b331204bb13fbd2ca93a64df1956f5b55478ce5'
VERIFIED_SHA='4cc3f8a38f851984baa481857de737facf17f71c63b632ceda7d510e5ee751eb'


def require(value,message):
    if not value:raise ValueError(message)


def canonical(value):return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False,allow_nan=False).encode()
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def signature(path):
    value=Path(path).stat();return {k:v for k,v in [('bytes',value.st_size),('inode',value.st_ino),('mtime_ns',value.st_mtime_ns),('ctime_ns',value.st_ctime_ns)]}
def write(path,value):Path(path).write_text(json.dumps(value,indent=2,allow_nan=False)+'\n')


def checkpoint_identity():
    require(sha(CHECKPOINT/'verified.json')==VERIFIED_SHA,'verified checkpoint manifest changed')
    verified=json.loads((CHECKPOINT/'verified.json').read_text())
    require(verified['complete'] and verified['revision']==REVISION,'unexpected checkpoint identity')
    # Read only checkpoint file identities from the prior clean reference.
    # No case inputs, outputs, scores, or errors are consumed by this experiment.
    require(sha(PRIOR_IDENTITY)==PRIOR_IDENTITY_SHA,'frozen checkpoint identity changed')
    prior=json.loads(PRIOR_IDENTITY.read_text())
    require(prior['revision']==REVISION and prior['status']=='complete','historical checkpoint identity unavailable')
    previous=prior['artifacts'];del prior
    artifacts={}
    for entry in verified['files']:
        name=entry['filename'];require(Path(name).name==name and name in previous,'unexpected checkpoint file')
        actual=signature(CHECKPOINT/name)
        require(actual=={k:previous[name][k] for k in actual},'checkpoint changed since historical reference: '+name)
        small=not name.startswith('model-')
        if small:require(sha(CHECKPOINT/name)==entry['sha256'],'checkpoint file hash changed: '+name)
        artifacts[name]={'sha256':entry['sha256'],'hash_rechecked':small,**actual}
    return {'repository':'Cloudflare/clef','revision':REVISION,'artifacts':artifacts,
            'prior_identity_receipt_sha256':sha(PRIOR_IDENTITY),'full_shard_hashes_rechecked':False}


def check_checkpoint(identity):
    for name,value in identity['artifacts'].items():
        require(signature(CHECKPOINT/name)=={k:value[k] for k in ('bytes','inode','mtime_ns','ctime_ns')},'checkpoint changed: '+name)


def load_official():
    spec=importlib.util.spec_from_file_location('clef_activation_official',CHECKPOINT/'joint_schema_model.py')
    module=importlib.util.module_from_spec(spec);sys.modules[spec.name]=module;spec.loader.exec_module(module)
    return module


def load_corpus(directory):
    directory=Path(directory);manifest=json.loads((directory/'manifest.json').read_text())
    require(sha(directory/'corpus.json')==manifest['corpus_sha256'],'frozen corpus changed')
    corpus=json.loads((directory/'corpus.json').read_text());rows=corpus['records']
    require(len(rows)==120 and len({r['id'] for r in rows})==120,'corpus identities differ')
    groups={part:{r['family'] for r in rows if r['partition']==part} for part in ('calibration','development')}
    require(not groups['calibration']&groups['development'],'calibration/development family overlap')
    require(sum(r['partition']=='calibration' for r in rows)==96 and sum(r['partition']=='development' for r in rows)==24,'partition sizes differ')
    require(len({r['request_sha256'] for r in rows})==120,'duplicate request')
    for row in rows:require(hashlib.sha256(canonical(row['request'])).hexdigest()==row['request_sha256'],'request changed')
    return rows,manifest
