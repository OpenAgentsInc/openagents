#!/usr/bin/env python3
"""Public classified checked addition remains analyzable in all aggregate profiles."""
import hashlib,json,subprocess,sys,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/tests/nostr/secret_checked_arithmetic_fixture.coax'
COMPILER=ROOT/'coax/bin/coax'
def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def main():
    source={str(SOURCE.resolve()):sha(SOURCE)}; compiler=sha(COMPILER); rows=[{'name':'x','type':'u64','classification':'public'}]; results=[]
    with tempfile.TemporaryDirectory(prefix='coax-public-arithmetic-') as tmp:
        d=Path(tmp)
        for profile in ('secret-check','public-view-check','large-source-audit'):
            tool=ROOT/'tools/coax-nostr'/profile/'aggregate.py'; policy=(tool.parent/'aggregate.coax').resolve(); policy_hash=sha(policy)
            if profile=='large-source-audit':
                p=subprocess.Popen([sys.executable,str(tool),str(SOURCE),'--entry','checked_add','--classification-stdio'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
                hello=json.loads(p.stdout.readline())
                if hello['sources']!=source or hello['compiler_sha256']!=compiler or hello['audit_inputs'].get(str(policy))!=policy_hash: raise AssertionError(hello)
                manifest={k:hello[k] for k in ('entry','sources','compiler_sha256','audit_inputs','audit_artifact_sha256','profile')}
                manifest.update(schema='coax.secret-large-classification-views.v3',views_disjoint=True,live_aligned_exclusive=True,public_only=True,parameters=[dict(rows[0],view=None)])
                out,err=p.communicate(json.dumps(manifest)+'\n',timeout=180)
                if p.returncode: raise AssertionError((profile,out,err))
                result=json.loads(out)
            else:
                classification=d/'classification.json';report=d/'report.json'
                classification.write_text(json.dumps({'schema':'coax.secret-classification.v1','entry':'checked_add','sources':source,'parameters':rows}))
                p=subprocess.run([sys.executable,str(tool),str(SOURCE),'--entry','checked_add','--classification',str(classification),'--report',str(report)],capture_output=True,text=True,timeout=180)
                if p.returncode: raise AssertionError((profile,p.stdout,p.stderr))
                result=json.loads(report.read_text())
            if (result['accepted'] is not True or result['error_name']!='accepted' or result['return_dependency_mask']!=0 or result['source_sha256']!=source or result['compiler_sha256']!=compiler or result['audit_input_sha256'].get(str(policy))!=policy_hash): raise AssertionError((profile,result))
            inputs=result['audit_input_sha256']
            if any(sha(path)!=digest for path,digest in inputs.items()): raise AssertionError('audit input drift')
            results.append({'profile':profile,'policy_sha256':policy_hash,'audit_inputs':inputs,'accepted_public_addition':True})
    if sha(SOURCE)!=next(iter(source.values())) or sha(COMPILER)!=compiler: raise AssertionError('source/compiler drift')
    print(json.dumps({'source_sha256':source,'compiler_sha256':compiler,'results':results},indent=2))
if __name__=='__main__':main()
