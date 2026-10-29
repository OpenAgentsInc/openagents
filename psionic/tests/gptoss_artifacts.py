#!/usr/bin/env python3
"""Owned original-artifact acquisition/verification. Download DATA only, no upstream code.
Large files stay in the caller's cache, never Git. Each completed file is SHA-256 pinned.
"""
import argparse,concurrent.futures,hashlib,json,os,time,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
PINS=ROOT/'psionic/fixtures/gptoss/original/pins.json'
REVISIONS={'20b':'6cee5e81ee83917806bbde320786a8fb61efebee','120b':'b5c939de8f754692c1647ca79fbf85e8c1e70f8a'}
CACHE=Path.home()/'.cache/psionic/gptoss-original'
def digest(path):
    h=hashlib.sha256()
    with path.open('rb') as f:
        for b in iter(lambda:f.read(8<<20),b''):h.update(b)
    return h.hexdigest()
def url(model,name):return f'https://huggingface.co/openai/gpt-oss-{model}/resolve/{REVISIONS[model]}/{name}'
def metadata():
    pins={'schema':1,'sources':{}}
    for model,revision in REVISIONS.items():
        with urllib.request.urlopen(f'https://huggingface.co/api/models/openai/gpt-oss-{model}/revision/{revision}?blobs=true',timeout=60) as r:meta=json.load(r)
        assert meta['sha']==revision
        files=[]
        for f in meta['siblings']:
            if not f['rfilename'].startswith('original/'):continue
            entry={'name':f['rfilename'],'bytes':f['size']}
            if 'lfs' in f:entry['sha256']=f['lfs']['sha256']
            else:
                with urllib.request.urlopen(url(model,entry['name']),timeout=60) as r:data=r.read()
                assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==f['blobId']
                assert len(data)==entry['bytes'];entry['sha256']=hashlib.sha256(data).hexdigest()
                target=CACHE/model/revision/entry['name'];target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
            files.append(entry)
        pins['sources'][model]={'repository':'openai/gpt-oss-'+model,'revision':revision,'files':files}
    PINS.write_text(json.dumps(pins,indent=2)+'\n')

def download(model,f):
    target=CACHE/model/REVISIONS[model]/f['name'];target.parent.mkdir(parents=True,exist_ok=True)
    if target.exists() and target.stat().st_size==f['bytes'] and digest(target)==f['sha256']:
        print('verified cached',model,f['name'],flush=True);return
    part=target.with_name(target.name+'.part')
    for attempt in range(8):
        offset=part.stat().st_size if part.exists() else 0
        if offset==f['bytes']:break
        req=urllib.request.Request(url(model,f['name'])+f'?download=true&psionic={time.time_ns()}',headers={'Range':f'bytes={offset}-'})
        try:
            with urllib.request.urlopen(req,timeout=120) as r:
                if offset and r.status!=206:offset=0
                with part.open('ab' if offset else 'wb') as out:
                    mark=time.monotonic()
                    while True:
                        block=r.read(8<<20)
                        if not block:break
                        out.write(block);offset+=len(block)
                        if time.monotonic()-mark>30:
                            print('download',model,f['name'],offset,'/',f['bytes'],flush=True);mark=time.monotonic()
            if offset==f['bytes']:break
        except Exception as e:
            # Exception URLs may contain signed redirect parameters. Never print them.
            print('retry',model,f['name'],type(e).__name__,attempt+1,flush=True);time.sleep(min(30,2**attempt))
    assert part.stat().st_size==f['bytes'],'incomplete original: '+f['name']
    assert digest(part)==f['sha256'],'original SHA-256 mismatch: '+f['name']
    os.replace(part,target);print('verified',model,f['name'],f['bytes'],flush=True)

def header(model,name):
    req=urllib.request.Request(url(model,name)+'?psionic_header='+str(time.time_ns()),headers={'Range':'bytes=0-1048583'})
    with urllib.request.urlopen(req,timeout=60) as r:
        raw=r.read(8);n=int.from_bytes(raw,'little');assert 2<=n<=1048576;data=r.read(n);assert len(data)==n
    target=ROOT/'psionic/fixtures/gptoss/original'/model/(Path(name).name+'.header.json');target.parent.mkdir(parents=True,exist_ok=True)
    target.write_bytes(data+b'\n');print('retained header',model,name,len(json.loads(data)),flush=True)

def main():
    p=argparse.ArgumentParser();p.add_argument('--pin',action='store_true');p.add_argument('--download',choices=['20b','120b','both']);p.add_argument('--headers',action='store_true');args=p.parse_args()
    if args.pin:metadata()
    pins=json.loads(PINS.read_text())
    if args.headers:
        for model,source in pins['sources'].items():
            for f in source['files']:
                if f['name'].endswith('.safetensors'):header(model,f['name'])
    if args.download:
        jobs=[(model,f) for model,source in pins['sources'].items() if args.download in (model,'both') for f in source['files']]
        with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
            for result in pool.map(lambda pair:download(*pair),jobs):pass
    if not any((args.pin,args.headers,args.download)):print('Original source pins:',', '.join(pins['sources']))
if __name__=='__main__':main()
