"""Check shadowing and explicit imports without executing any runner."""
import importlib.util,json
from pathlib import Path
import runner

def main():
    out=runner.HERE/'import-check.json';runner.proof.require(not out.exists(),'refuse replacing import checks')
    shadow=Path(importlib.util.find_spec('native_image_parity').origin).resolve()
    assert shadow!=runner.ORIGINAL.resolve(), 'fixture must reproduce the actual shadowing'
    module,identity=runner.load_local_runner();assert module.run.__module__=='_clef_positional_frozen_image_runner_v1'
    assert identity['module_file']==identity['run_sourcefile']==str(runner.ORIGINAL)
    rejected=[]
    for name,path,digest in [('shared-path',shadow,None),('wrong-digest',runner.ORIGINAL,'0'*64),('missing-path',runner.HERE/'absent.py',None)]:
        try:runner.load_local_runner(path,digest)
        except (ValueError,FileNotFoundError):rejected.append(name)
        else:raise AssertionError(name)
    runner.proof.write(out,{'status':'complete','model_loaded':False,'runner_called':False,'GPU_used':False,
        'ordinary_import_resolves_to':str(shadow),'actual_explicit_import':identity,'rejected':rejected,
        'source_sha256':{str(p):runner.proof.sha(p) for p in [Path(__file__),runner.HERE/'runner.py',runner.ORIGINAL]}})
    print(json.dumps({'status':'complete','rejected':rejected,'actual_explicit_import':identity}))
if __name__=='__main__':main()
