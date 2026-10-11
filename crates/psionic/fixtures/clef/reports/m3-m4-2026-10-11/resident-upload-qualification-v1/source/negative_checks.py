"""Model-free checks for exact comparison and malformed or cached results."""
import copy,json,struct,tempfile
from pathlib import Path
import proof,text

def main():
    out=proof.HERE/'negative-checks.json';proof.require(not out.exists(),'refuse replacing checks')
    case={'id':'fixture','digest':'sha256:fixture','request':{'model':'clef','questions':{'q':{'type':'noul'}}}}
    response={'model':'clef','usage':{'input_tokens':3,'output_tokens':0,'cached_input_tokens':0},'answers':{'q':{'type':'noul','noul':.75}},
              'psionic':{'artifact_digest':'a','head_digest':'h','prefill_chunk':512,'backend':'metal','accumulate':'f32','truncated_state_tokens':0,'timing':{'cached':False}}}
    row={'logits':[0.,1.],'rows':['last','question','option:true','option:false'],'width':5120,'index':0,'pooled_rows_sha256':'same','pooled_bytes':bytes(4*5120*2)}
    key=(case['digest'],'q');old={case['id']:response};a={key:row}
    assert text.compare([case],old,copy.deepcopy(old),a,copy.deepcopy(a))['passed']
    rejected=[]
    for mutation in ['one_ulp','negative_zero','pooled_byte','token','metadata','top','cache','cache_string','cache_ollama','nonfinite','missing_option','width']:
        new=copy.deepcopy(old);b=copy.deepcopy(a);r=new[case['id']];v=b[key]
        if mutation=='one_ulp':v['logits'][1]=struct.unpack('<f',struct.pack('<I',0x3f800001))[0]
        elif mutation=='negative_zero':v['logits'][0]=-0.
        elif mutation=='pooled_byte':v['pooled_bytes']=b'\x01'+v['pooled_bytes'][1:]
        elif mutation=='token':r['usage']['input_tokens']=4
        elif mutation=='metadata':r['psionic']['head_digest']='changed'
        elif mutation=='top':r['answers']['q']['noul']=.25
        elif mutation=='cache':r['usage']['cached_input_tokens']=1
        elif mutation=='cache_string':r['usage']['cached_input_tokens']='0'
        elif mutation=='cache_ollama':r['prompt_eval_cached_count']=1
        elif mutation=='nonfinite':v['logits'][0]=float('nan')
        elif mutation=='missing_option':v['logits'].pop()
        else:v['width']=4096
        try:accepted=text.compare([case],old,new,a,b)['passed']
        except (ValueError,RuntimeError,KeyError,OverflowError):accepted=False
        assert not accepted,mutation;rejected.append(mutation)
    with tempfile.TemporaryDirectory(prefix='export-selftest-',dir=proof.HERE) as directory:
        p=Path(directory);base={k:v for k,v in row.items() if k not in ('index','pooled_bytes','pooled_rows_sha256')}
        base.update(request_sha256=case['digest'],question='q',question_index=0,offset=0,dtype='f16le')
        for mutation in ['valid','missing','duplicate','truncated','trailing','wrong_question','wrong_options','offset']:
            r=copy.deepcopy(base);data=row['pooled_bytes']
            if mutation=='truncated':data=data[:-1]
            elif mutation=='trailing':data+=b'\x00'
            elif mutation=='wrong_question':r['question']='other'
            elif mutation=='wrong_options':r['rows'][-1]='option:other'
            elif mutation=='offset':r['offset']=1
            rows=[] if mutation=='missing' else [r,r] if mutation=='duplicate' else [r]
            (p/'rows.f16').write_bytes(data);(p/'rows.jsonl').write_text(''.join(json.dumps(x)+'\n' for x in rows))
            try:result=proof.helper.exports(p,[case]);accepted=True
            except ValueError:accepted=False
            assert accepted==(mutation=='valid'),mutation
            if accepted:assert result[key]['pooled_bytes']==row['pooled_bytes']
            else:rejected.append('export_'+mutation)
    proof.write(out,{'status':'complete','model_loaded':False,'GPU_used':False,'positive_fixtures':2,'rejected':rejected,
                     'sources':{str(p):proof.sha(p) for p in [Path(__file__),proof.HERE/'text.py',proof.HERE/'proof.py',proof.HERE/'acceptance_helpers.py']}})
    print(json.dumps({'status':'complete','rejected':len(rejected)}))
if __name__=='__main__':main()
