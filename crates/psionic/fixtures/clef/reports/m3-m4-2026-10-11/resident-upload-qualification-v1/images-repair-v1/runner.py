"""Load the original frozen image runner by exact path and preserve attempt1."""
import importlib.util,inspect,json,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;QUAL=HERE.parent
sys.path.insert(0,str(QUAL))
import proof
ORIGINAL_DESCRIPTOR='b0448dfd6314cfa0e1b53570d865f66d0a060923bdebf48e47637651d2c1600b'
CANDIDATE='c2539ca0fe9b38ac31691872b59f44e0a14afcc4de401433b5651da83cfac549'
TEXT='f977f1aafdb8492e746246a7b3faf54841ecf39d3966921d69b26cc532a73e0e'
ORIGINAL=QUAL/'native_image_parity.py'

def load_local_runner(path=ORIGINAL,expected=None):
    proof.require(Path(path).resolve()==ORIGINAL.resolve(),'only original frozen local runner is allowed')
    descriptor=proof.load(QUAL/'qualification-source.json')
    digest=descriptor['files'][str(ORIGINAL)] if expected is None else expected
    proof.require(proof.sha(path)==digest,'image runner hash differs')
    spec=importlib.util.spec_from_file_location('_clef_positional_frozen_image_runner_v1',path)
    proof.require(spec is not None and spec.loader is not None,'cannot load fixed image runner')
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    proof.require(Path(module.__file__).resolve()==ORIGINAL.resolve(),'loaded module path differs')
    proof.require(Path(inspect.getsourcefile(module.run)).resolve()==ORIGINAL.resolve(),'loaded run function source differs')
    proof.require(proof.sha(module.__file__)==digest,'loaded module changed')
    return module,{'module_file':str(Path(module.__file__).resolve()),'run_sourcefile':inspect.getsourcefile(module.run),'sha256':digest,'unique_module_name':spec.name}

def frozen():
    proof.require(proof.sha(QUAL/'qualification-source.json')==ORIGINAL_DESCRIPTOR,'original harness changed')
    proof.require(proof.sha(QUAL/'candidate.json')==CANDIDATE and proof.sha(QUAL/'text-1/summary.json')==TEXT,'successful prerequisites changed')
    proof.frozen();candidate=proof.candidate()
    descriptor=proof.load(HERE/'source.json');proof.verify_files(descriptor['files'])
    proof.require(descriptor['original_harness_sha256']==ORIGINAL_DESCRIPTOR,'repair binding differs')
    return candidate

def run():
    proof.require_leases();proof.clean_inherited_environment()
    record=HERE/'execution.json';proof.require(not record.exists(),'refuse replacing repair execution')
    result={'status':'failed','passed':False,'attempt':'images-2','original_attempt_preserved':True,
            'bounds':{'max_rss_gib':52,'minimum_start_available_gib':64,'minimum_available_gib':12,'startup_timeout':600,'request_timeout':300,'deadline_seconds':1800},
            'original_harness_sha256':ORIGINAL_DESCRIPTOR,'candidate_sha256':CANDIDATE,'text_sha256':TEXT}
    proof.write(record,result)
    try:
        candidate=frozen();proof.disk_check(start=True)
        prior=proof.load(QUAL/'images-1/summary.json')
        proof.require(prior['status']=='failed' and prior['error_type']=='KeyError' and prior['error']=="'sources_rechecked'",'original failure differs')
        module,identity=load_local_runner();result['executed_runner']=identity;result['repair_source_sha256']=proof.sha(HERE/'source.json');proof.write(record,result)
        config=proof.load(QUAL/'images-config.json');config.update(binary=candidate['binary'],output=str(QUAL/'images-2'))
        proof.require(config['env']==proof.ENV and config['chunk']==512,'fixed mode changed')
        proof.require(all(config[k]==v for k,v in result['bounds'].items()),'fixed bounds changed')
        proof.require(not (QUAL/'images-2').exists(),'refuse replacing image outputs')
        code=module.run(config);result['runner_exit']=code
        run=proof.load(QUAL/'images-2/run.json');data=proof.load(QUAL/'images-2/0-images.json')
        proof.require(code==0 and run['status']=='complete' and run['sources_rechecked'],'image server run failed')
        proof.require(run['memory']['owned_group_stopped'] and not any(run['memory'].get(k) for k in ('memory_rejection','cleanup_error','sampling_error')),'image cleanup failed')
        proof.require(data['passed'] and len(data['cases'])==6 and sum(len(c['question_comparisons']) for c in data['cases'])==14,'image coverage differs')
        proof.require(len(data['http_refusals'])==10 and all(c['passed'] for c in data['http_refusals']),'refusal coverage differs')
        for case in data['cases']:
            response=case['response'];proof.bench.validate_uncached_response(200,response)
            proof.require(response['psionic']['backend']=='metal' and response['psionic']['truncated_state_tokens']==0,'image backend/truncation differs')
        frozen();proof.disk_check();result.update(status='complete',passed=True,cases=6,questions=14,refusals=10,sources_rechecked=True,
            result_sha256=proof.sha(QUAL/'images-2/0-images.json'),run_sha256=proof.sha(QUAL/'images-2/run.json'),
            acceptance='Unchanged image top/token/expected-content plus refusal gate; probabilities diagnostic.')
    except BaseException as error:result.update(error_type=type(error).__name__,error=str(error))
    finally:
        try:frozen();proof.disk_check();result['post_cleanup_sources_rechecked']=True
        except BaseException as error:result.update(status='failed',passed=False,postcheck_error=str(error))
        proof.write(record,result)
    print(json.dumps(result,indent=2));return 0 if result['passed'] else 1
if __name__=='__main__':raise SystemExit(run())
