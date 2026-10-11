"""Read bounded exported logits/pooled bytes and send exact saved request bytes."""
import json,math,sys,time,urllib.error,urllib.request
from pathlib import Path
REPO=Path('@@CLEF_WORKTREE@@')
sys.path.insert(0,str(REPO/'crates/psionic/fixtures/clef/tools'))
import mac_quiet_bench as bench
import image_parity

def write(path,value):
    Path(path).write_text(json.dumps(value,indent=2,allow_nan=False)+'\n')

def post(url,raw,timeout):
    request=urllib.request.Request(url,data=raw,headers={'Content-Type':'application/json'})
    began=time.monotonic()
    try:
        with bench.HTTP.open(request,timeout=timeout) as response:body,status=response.read(),response.status
    except urllib.error.HTTPError as error:body,status=error.read(),error.code
    return time.monotonic()-began,status,body


def exports(directory,cases):
    expected={case['digest']:case for case in cases}
    table={}
    data=(directory/'rows.f16').read_bytes()
    consumed=0
    indices={digest:set() for digest in expected}
    for line in (directory/'rows.jsonl').open(encoding='utf-8'):
        row=json.loads(line)
        digest=row['request_sha256'];question=row['question'];key=(digest,question)
        if digest not in expected or question not in expected[digest]['request']['questions'] or key in table:
            raise ValueError('unexpected or duplicate exported question')
        question_config=expected[digest]['request']['questions'][question]
        kind=question_config['type']
        options=['true','false'] if kind=='noul' else ([str(i) for i in range(len(question_config['criteria']))] if isinstance(question_config['criteria'],list) else list(question_config['criteria']))
        logits=row['logits']
        if len(logits)!=len(options) or not logits or any(type(x) not in (int,float) or not math.isfinite(x) for x in logits):
            raise ValueError('missing or nonfinite exported logits')
        if row['rows'][:2]!=['last','question'] or set(row['rows'][2:])!={'option:'+x for x in options} or len(row['rows'])!=2+len(options):
            raise ValueError('exported option identity differs')
        if row['dtype']!='f16le' or type(row['width']) is not int or row['width']<=0 or row['offset']!=consumed:
            raise ValueError('invalid pooled-row export layout')
        end=consumed+len(row['rows'])*row['width']*2
        if end>len(data):raise ValueError('truncated pooled-row export')
        if row['question_index'] in indices[digest]:raise ValueError('duplicate question index')
        indices[digest].add(row['question_index'])
        table[key]={'logits':logits,'rows':row['rows'],'width':row['width'],'index':row['question_index'],
                    'pooled_rows_sha256':bench.sha(data[consumed:end]),'pooled_bytes':data[consumed:end]}
        consumed=end
    if consumed!=len(data):raise ValueError('unindexed pooled-row bytes')
    for digest,case in expected.items():
        if indices[digest]!=set(range(len(case['request']['questions']))):raise ValueError('missing exported questions')
    return table

