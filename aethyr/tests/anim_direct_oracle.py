# Firm reason: 2 (independent oracle: reads ALP/JSON independently and checks the real CLI/API; never implements an edit).
"""Owned loopback qualification; all source, request and render data stays private."""
import argparse, hashlib, http.server, json, os, pathlib, shutil, socket, struct, subprocess, tempfile, threading, time, uuid


def digest(b):
    return hashlib.sha256(b).hexdigest()


def parts(b):
    magic, size, libs, perfs, voices = struct.unpack_from('<5I', b)
    assert magic == 0x31504c41 and size == len(b)
    return libs, perfs, [b[at:at+n] for at, n in (struct.unpack_from('<II', b, 20+8*i) for i in range(libs+perfs+voices+2))]


def proposal(kind, **values):
    return dict(schema='anim.direction.v1', kind=kind, actor=0, row=0, option=0, at=0, end=0, value=0, point=[0, 0, 0], confidence={k:950 for k in ('kind','actor','row','option','at','end','value','point')}) | values


def run(args):
    root = pathlib.Path(args.out).resolve()
    assert root.is_relative_to(pathlib.Path.home()/'.openagents')
    root.mkdir(parents=True, exist_ok=False)
    base = pathlib.Path(args.project).read_bytes()
    libs, perfs, original = parts(base)
    scene = json.loads(original[libs+perfs])
    perf = json.loads(original[libs])
    last = len(perf['layers'])-1
    path_point = scene['cast'][0]['path'][0]['position'].copy(); path_point[0] += 0.00001
    endpoint = f'http://127.0.0.1:{args.api_port}'
    cases = [
        ('speed', "Set Draconia's layer 0 speed to 0.999", proposal('speed', value=0.999)),
        ('path', f"Set Draconia's path key 0 position to {json.dumps(path_point)}", proposal('path', row=0, point=path_point)),
        ('look-at', 'Add a Head look-at for Draconia toward figure-left from 27 to 28 seconds', proposal('look-at', row=6, option=1, at=27, end=28)),
        ('hand-to', 'Add a right hand reach for Draconia toward gate-post from 20 to 21 seconds', proposal('hand-to', row=34, at=20, end=21)),
        ('hold', f"Hold Draconia's layer {last} until 28.1 seconds", proposal('hold', row=last, end=28.1)),
        ('clip-swap', "Swap Draconia's layer 2 clip to guardian-extras", proposal('clip-swap', row=2, option=2)),
        ('key', 'Add the guardian-wand-key pose for Draconia with upper mask from 20 to 21 seconds', proposal('key', row=3, option=4, at=20, end=21)),
        ('shot-add', 'Insert an S3-camera shot in shot 5 from scene time 27 to 28, keeping the surrounding footage', proposal('shot-add', row=5, option=2, at=27, end=28)),
        ('shot-remove', 'Remove shot 5 from the edit while preserving the other authored shots', proposal('shot-remove', row=5)),
        ('shot-retime', 'Retime shot 3 to scene time 12 through 18.1 and keep neighboring cuts continuous', proposal('shot-retime', row=3, at=12, end=18.1)),
        ('cut-on-event', 'Cut shot 3 to camera S3 when wand-light starts', proposal('cut-on-event', actor=1, row=3, option=2)),
    ]
    selected = {}
    seen = []

    class Model(http.server.BaseHTTPRequestHandler):
        def log_message(self, *unused):
            pass
        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            request = json.loads(body['contents'][0]['parts'][0]['text'])
            assert body['generationConfig']['responseMimeType'] == 'application/json'
            assert body['generationConfig']['responseSchema']['properties']['kind']['enum'][1] == 'speed'
            assert request['words'] == selected['words'] and request['base'] == digest(selected['base'])
            seen.append(request['words'])
            (root/(selected['name']+'.request.json')).write_text(json.dumps(request))
            if selected.get('race'):
                selected['file'].write_bytes(selected['race'])
            answer = {'candidates':[{'finishReason':'STOP','content':{'parts':[{'text':json.dumps(selected['proposal'])}]}}]}
            if selected.get('malformed'):
                answer = {'candidates':[]}
            data = json.dumps(answer).encode()
            self.send_response(200); self.send_header('Content-Type','application/json'); self.send_header('Content-Length',str(len(data))); self.end_headers(); self.wfile.write(data)

    model = http.server.ThreadingHTTPServer(('127.0.0.1', args.model_port), Model)
    threading.Thread(target=model.serve_forever, daemon=True).start()
    pg = pathlib.Path('/opt/homebrew/opt/postgresql@16/bin')
    work = pathlib.Path(tempfile.mkdtemp(prefix='a7-api-'))
    api = None
    profile = 'a7-proof-'+uuid.uuid4().hex[:12]
    keyfile = pathlib.Path.home()/'.openagents/cli/keys'/profile
    keyfile.parent.mkdir(parents=True, exist_ok=True)
    keyfile.write_text('nostr-secret-v1 '+os.urandom(32).hex()+'\n'); keyfile.chmod(0o600)
    env = os.environ.copy(); env.pop('OPENAGENTS_NSEC',None); env['OPENAGENTS_KEYCHAIN']='0'
    def cli(*words, expected=0):
        result = subprocess.run([args.cli,'--json','--profile',profile,*(['--origin',endpoint] if words[0]=='edit' else []),'labs','anim',*map(str,words)], env=env, capture_output=True, text=True, timeout=90)
        if result.returncode != expected:
            raise AssertionError(f'{words[0]} exit {result.returncode}, expected {expected}: {result.stdout[:800]} {result.stderr[-300:]}')
        return result.stdout
    try:
        subprocess.run([str(pg/'initdb'),'-D',str(work/'db'),'-U','postgres','-A','trust'], check=True, capture_output=True)
        subprocess.run([str(pg/'pg_ctl'),'-D',str(work/'db'),'-o',f'-p {args.pg_port} -k {work} -c listen_addresses=','-l',str(work/'pg.log'),'-w','start'],check=True,capture_output=True)
        subprocess.run([str(pg/'psql'),'-h',str(work),'-p',str(args.pg_port),'-U','postgres','-c','CREATE ROLE oa_a7 LOGIN','-c','CREATE DATABASE oa_a7 OWNER oa_a7'],check=True,capture_output=True)
        api_env = env | dict(OA_API_DATABASE=f'host={work} port={args.pg_port} user=oa_a7 dbname=oa_a7',OA_API_ORIGIN=endpoint,OA_NPC_TOKEN='fixture-token',OA_ANIME_TIMEOUT_MS='15000',OA_ANIME_URL=f'http://127.0.0.1:{args.model_port}/model')
        if args.live:
            token = subprocess.run(['gcloud','auth','print-access-token'],capture_output=True,text=True,check=True,timeout=30).stdout.strip()
            api_env['OA_NPC_TOKEN']=token;api_env.pop('OA_ANIME_URL');token=None
        log = (root/'api.log').open('w')
        api = subprocess.Popen([args.api_bin,'--listen',f'127.0.0.1:{args.api_port}','--migrations','coax/web/api/migrations'],env=api_env,stdout=log,stderr=log)
        import urllib.request
        for _ in range(100):
            try:
                urllib.request.urlopen(endpoint+'/api/playtest/health',timeout=1).read();break
            except Exception:
                assert api.poll() is None, 'owned API exited before readiness'
                time.sleep(.1)
        else:
            raise AssertionError('owned API never ready')
        receipts = []
        changed = {}
        for name, words, answer in cases:
            if name in args.skip.split(','):continue
            directory = root/name;directory.mkdir();file=directory/'scene.alp';file.write_bytes(base)
            selected.update(name=name,words=words,proposal=answer,base=base,file=file,race=None,malformed=False)
            started = time.monotonic()
            raw = cli('edit',file,words)
            result=json.loads(raw);assert result['outcome']=='applied' and result['kind']==name, result
            history=pathlib.Path(str(file)+'.anim-history');id=(history/'head').read_text();after=(history/(id+'.alp')).read_bytes();assert file.read_bytes()==base
            _, _, rows=parts(after);differences=[i for i,(a,b) in enumerate(zip(original,rows)) if a!=b];assert len(differences)==1 and differences[0]>=libs
            receipt_bytes=(history/(id+'.json')).read_bytes();receipt=json.loads(receipt_bytes);assert digest(receipt_bytes)==id and receipt['content']==digest(after)
            parent=receipt['parent'];parent_bytes=(history/(parent+'.json')).read_bytes();assert digest(parent_bytes)==parent and json.loads(parent_bytes)['content']==digest(base)
            diff=json.loads(cli('diff',file));assert diff['from']==parent and diff['to']==id and len(diff['parts'])==1
            (directory/'check.json').write_text(cli('check',file))
            changed[name]=after
            cli('undo',file);assert (history/'head').read_text()==parent and (history/(parent+'.alp')).read_bytes()==base
            receipts.append(dict(kind=name,version=id,changed_part=differences[0],confidence_milli=result['confidence_milli'],elapsed_ms=round((time.monotonic()-started)*1000),undo_exact=True))
            (root/'progress.json').write_text(json.dumps(receipts,indent=2)+'\n')
            print('PASS',name,flush=True)
            if name=='speed' and not args.live:
                # Same content under different words is a distinct immutable fork.
                selected['words']='Use speed .999 for the first layer'
                fork=json.loads(cli('edit',file,selected['words']));fork_id=(history/'head').read_text()
                assert fork_id!=id and (history/(id+'.json')).read_bytes()==receipt_bytes
                assert (history/(fork_id+'.alp')).read_bytes()==after
                cli('undo',file);assert (history/'head').read_text()==parent
                selected['proposal']=answer | {'confidence':{k:960 for k in answer['confidence']}}
                confidence_fork=json.loads(cli('edit',file,selected['words']));confidence_id=(history/'head').read_text();assert confidence_id not in (id,fork_id)
                def refused(verb):
                    r=subprocess.run([args.cli,'--json','--profile',profile,'labs','anim',verb,str(file)],env=env,capture_output=True,text=True,timeout=90)
                    assert r.returncode!=0,(verb,r.stdout)
                current_receipt=history/(confidence_id+'.json');saved=current_receipt.read_bytes();current_receipt.write_bytes(saved+b' ');refused('check');current_receipt.write_bytes(saved)
                snapshot=history/(confidence_id+'.alp');saved_snapshot=snapshot.read_bytes();snapshot.write_bytes(saved_snapshot[:-1]);refused('check');snapshot.write_bytes(saved_snapshot)
                parent_receipt=history/(parent+'.json');saved_parent=parent_receipt.read_bytes();parent_receipt.write_bytes(saved_parent+b' ');refused('undo');assert (history/'head').read_text()==confidence_id;parent_receipt.write_bytes(saved_parent)
                cli('undo',file);assert (history/'head').read_text()==parent
                print('PASS distinct immutable forks and corrupt receipt/payload/parent refusal',flush=True)
            # Distinct fixture principals keep quota testing separate from edit semantics.
            keyfile.write_text('nostr-secret-v1 '+os.urandom(32).hex()+'\n')
        if not args.live:
            for name, answer in [('unmatched',proposal('unmatched')),('ambiguous',proposal('speed',value=.5,confidence={k:849 for k in ('kind','actor','row','option','at','end','value','point')})),('missing',proposal('clip-swap',option=511)),('bad-screen',proposal('path',row=0,point=[100,0,100])),('malformed',proposal('speed',value=.5))]:
                file=root/(name+'.alp');file.write_bytes(base);selected.update(name=name,words=name,proposal=answer,base=base,file=file,malformed=name=='malformed',race=None)
                result=subprocess.run([args.cli,'--json','--profile',profile,'--origin',endpoint,'labs','anim','edit',str(file),name],env=env,capture_output=True,text=True,timeout=90)
                assert file.read_bytes()==base and not pathlib.Path(str(file)+'.anim-history/head').exists()
                assert (result.returncode==0)==(name in ('unmatched','ambiguous')),result.stdout
                print('PASS refusal',name,flush=True);keyfile.write_text('nostr-secret-v1 '+os.urandom(32).hex()+'\n')
            file=root/'race.alp';file.write_bytes(base);selected.update(name='race',words='race',proposal=proposal('speed',value=.999),base=base,file=file,malformed=False,race=next(iter(changed.values())))
            result=subprocess.run([args.cli,'--json','--profile',profile,'--origin',endpoint,'labs','anim','edit',str(file),'race'],env=env,capture_output=True,text=True,timeout=90)
            assert result.returncode!=0 and file.read_bytes()==next(iter(changed.values())) and not pathlib.Path(str(file)+'.anim-history/head').exists(),result.stdout
            print('PASS stale publication race',flush=True)
        (root/'receipt.json').write_text(json.dumps(dict(profile='anim-direction-a7',live=args.live,cases=receipts,model_requests=len(seen)),indent=2)+'\n')
    finally:
        if api:
            api.terminate()
            try:api.wait(timeout=10)
            except subprocess.TimeoutExpired:api.kill();api.wait()
        subprocess.run([str(pg/'pg_ctl'),'-D',str(work/'db'),'-m','immediate','stop'],capture_output=True)
        model.shutdown();keyfile.unlink(missing_ok=True);shutil.rmtree(work)


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--project',required=True);p.add_argument('--out',required=True);p.add_argument('--cli',required=True);p.add_argument('--api-bin',required=True);p.add_argument('--api-port',type=int,default=58441);p.add_argument('--pg-port',type=int,default=58442);p.add_argument('--model-port',type=int,default=58443);p.add_argument('--skip',default='');p.add_argument('--live',action='store_true');run(p.parse_args())
