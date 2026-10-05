#!/usr/bin/env python3
"""Record a temporary twenty-player battle with one native primary window through a controlled TLS route."""
import argparse, hashlib, json, os, pathlib, shlex, shutil, socket, subprocess, tempfile, time
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--asset-dir',type=pathlib.Path,required=True)
parser.add_argument('--binaries',type=pathlib.Path,required=True)
parser.add_argument('--compiled-revision',required=True)
parser.add_argument('--client-compiled-revision',help='Compiled revision of the native client')
parser.add_argument('--load-compiled-revision',help='Compiled revision of the headless load driver')
parser.add_argument('--seconds',type=int,default=60)
parser.add_argument('--players',type=int,default=20)
parser.add_argument('--movement-frames',action='store_true',help='Request interval movement for native and headless battle clients')
parser.add_argument('--ssh-host',help='Run the fixture server through SSH on a separate Linux machine')
parser.add_argument('--remote-binary',help='Absolute path to the pinned Linux server binary')
parser.add_argument('--remote-user',default='christopherdavid',help='Linux account that owns the fixture')
parser.add_argument('--sample-host',action='store_true',help='Take a five-second macOS CPU sample of the owned host during the capture')
parser.add_argument('--persistent',action='store_true',help='Enable host saves in the isolated scratch directory')
parser.add_argument('--gpu-timing',action='store_true',default=os.environ.get('VERSE_GPU_TIMING')=='1',help='Request optional native GPU timestamps and record the request in the workload')
parser.add_argument('--delay-ms',type=int,default=40)
parser.add_argument('--jitter-ms',type=int,default=20)
args=parser.parse_args()
if bool(args.ssh_host) != bool(args.remote_binary):
    parser.error('SSH host and remote binary must be supplied together')
if args.ssh_host and args.sample_host:
    parser.error('macOS CPU sampling is unavailable for a Linux server')
if args.remote_binary and not pathlib.PurePosixPath(args.remote_binary).is_absolute():
    parser.error('Remote binary must be an absolute path')
if args.sample_host and shutil.which('sample') is None:
    parser.error('Host CPU sampling requires the macOS sample command')
if not (1<=args.seconds<=90 and 2<=args.players<=20 and 0<=args.delay_ms<=250 and 0<=args.jitter_ms<=100):
    parser.error('Duration, delay, or jitter exceeds fixture bounds')
root=pathlib.Path(tempfile.mkdtemp(prefix='verse-battle-scale-'))
os.chmod(root,0o700)
print('Scratch artifacts '+str(root),flush=True)
host_cpu_samples=[]
host_cpu_errors=0
def sample_host_cpu(process):
    global host_cpu_errors
    if args.ssh_host:
        return
    try:
        raw=subprocess.run(['ps','-o','time=','-p',str(process.pid)],capture_output=True,text=True,
                           env={**os.environ,'LC_ALL':'C'},timeout=2)
        if raw.returncode or not raw.stdout.strip():
            return
        value=raw.stdout.strip()
        days=0
        if '-' in value:
            day,value=value.split('-',1);days=int(day)
        seconds=0.
        for part in value.split(':'):
            seconds=seconds*60+float(part)
        if len(host_cpu_samples)<160:
            host_cpu_samples.append({'elapsed_seconds':time.monotonic()-host_cpu_started,
                                     'cumulative_cpu_seconds':days*86400+seconds})
    except (OSError,ValueError,subprocess.TimeoutExpired):
        host_cpu_errors+=1

repo=pathlib.Path(__file__).resolve().parents[2]
assets=args.asset_dir.resolve()
binaries=args.binaries.resolve()
def openssl(*args):
    return subprocess.check_output(['openssl',*map(str,args)],stderr=subprocess.DEVNULL)
keys=[]
roles=['primary']+[f'load{i:02}' for i in range(args.players-1)]
for role in roles:
    pem=root/(role+'.pem');pem.write_bytes(openssl('ecparam','-name','secp256k1','-genkey','-noout'))
    der=openssl('ec','-in',pem,'-outform','DER')
    assert der[5:7]==b'\x04\x20'
    private=root/(role+'.key');private.write_text(der[7:39].hex());os.chmod(private,0o600)
    pub=openssl('ec','-in',pem,'-pubout','-outform','DER')[-65:]
    assert pub[0]==4
    keys.append(pub[1:33].hex())
cert=root/'cert.pem';private=root/'tls.pem'
openssl('req','-x509','-newkey','rsa:2048','-nodes','-keyout',private,'-out',cert,'-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost','-addext','basicConstraints=critical,CA:FALSE','-addext','extendedKeyUsage=serverAuth')
(root/'cert.der').write_bytes(openssl('x509','-in',cert,'-outform','DER'))
(root/'tls.der').write_bytes(openssl('pkcs8','-topk8','-nocrypt','-in',private,'-outform','DER'));os.chmod(root/'tls.der',0o600)
with socket.socket() as sock:
    sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
address=f'127.0.0.1:{port}'
authored=json.loads((repo/'assets/verse/original/ritual.json').read_text())
cultists=[a for a in authored['actors'] if a['model'].startswith('cultist')]
import copy
for i in range(40-1-len(cultists)):
    actor=copy.deepcopy(cultists[0]);actor['id']=200+i
    authored['actors'].append(actor);cultists.append(actor)
for i,actor in enumerate(cultists):
    actor['position']=[-4.5+3*(i%4),0,-9+2*(i//4)]
    actor['health']=20000
scene=str(root/'battle.json');(root/'battle.json').write_text(json.dumps(authored))
pack=str(assets/'runtime-pack.json')
host={'authored_combat_health':True,'listen':address,'instance':220,'scene':scene,'pack':pack,'certificate_der':str(root/'cert.der'),'private_key_der':str(root/'tls.der'),'enrollments':[{'public_key':k,'role':({'type':'primary'} if role=='primary' else {'type':'player','spawn':[-6+3*((i-1)%5),0,-21+2*((i-1)//5)]})} for i,(role,k) in enumerate(zip(roles,keys))]}
if args.persistent:
    host['state_dir']=str(root/'state')
(root/'host.json').write_text(json.dumps(host))
env=dict(os.environ)
env['VERSE_GPU_TIMING']='1' if args.gpu_timing else '0'
(root/'home').mkdir()
env['HOME']=str(root/'home')
(root/'workload.json').write_text(json.dumps({'players':args.players,'hostile_npcs':40,'cultist_health':20000,'native_clients':1,'headless_clients':args.players-1,'seconds':args.seconds,'persistent_storage':args.persistent,'gpu_timestamps_requested':args.gpu_timing,'native_movement_frames_requested':args.movement_frames,'headless_movement_frames_requested':args.movement_frames,'compiled_revisions':{'host':args.compiled_revision,'headless_clients':args.load_compiled_revision or args.compiled_revision,'native_client':args.client_compiled_revision or args.compiled_revision},'fixture_sha256':hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),'fixture_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'limits':['NPC health is raised in the authored load scene to sustain spell and AI work.','Headless player connections do not establish rendering performance on their machines.','Native player and load generator share one host machine.','Server runs on Linux through SSH.' if args.ssh_host else 'Server shares the Mac with clients.']}))
logs=[];processes=[]
remote_root=None
hostp=None
def remote(command, **kwargs):
    return subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=10',args.ssh_host,
                           'runuser -u '+shlex.quote(args.remote_user)+' -- sh -c '+shlex.quote(command)],
                          check=True, **kwargs)
def stop_host():
    if hostp is None or hostp.poll() is not None:
        return
    if args.ssh_host:
        hostp.stdin.close()
    else:
        hostp.terminate()
    try:
        hostp.wait(timeout=30)
    except subprocess.TimeoutExpired:
        if not args.ssh_host:
            hostp.kill();hostp.wait()
        raise
sample_process=None
sample_receipt={'requested':args.sample_host,'status':'not_started',
                'limits':['CPU sampling adds observer overhead and does not establish performance acceptance.']}
try:
    log=open(root/'host.log','w');logs.append(log)
    if args.ssh_host:
        remote_root=remote('mktemp -d /tmp/verse-battle-host-XXXXXXXX',capture_output=True,text=True).stdout.strip()
        if not remote_root.startswith('/tmp/verse-battle-host-') or '/' in remote_root[len('/tmp/'):]:
            raise RuntimeError('Unexpected remote scratch path')
        remote('mkdir '+shlex.quote(remote_root+'/assets')+' '+shlex.quote(remote_root+'/home'))
        with tempfile.TemporaryFile() as bundle:
            subprocess.run(['tar','-cf','-', '-C',str(assets),'.'],stdout=bundle,check=True)
            bundle.seek(0)
            remote('tar -xf - -C '+shlex.quote(remote_root+'/assets'),stdin=bundle)
        port_script='import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])'
        remote_port=int(remote('python3 -c '+shlex.quote(port_script),capture_output=True,text=True).stdout)
        remote_config=dict(host)
        for field,name in [('scene','battle.json'),('certificate_der','cert.der'),('private_key_der','tls.der')]:
            remote_config[field]=remote_root+'/'+name
        remote_config['pack']=remote_root+'/assets/runtime-pack.json'
        remote_config['listen']='127.0.0.1:'+str(remote_port)
        if args.persistent:
            remote_config['state_dir']=remote_root+'/state'
        (root/'remote-host.json').write_text(json.dumps(remote_config))
        with tempfile.TemporaryFile() as bundle:
            subprocess.run(['tar','-cf','-', '-C',str(root),'battle.json','cert.der','tls.der','remote-host.json'],stdout=bundle,check=True)
            bundle.seek(0)
            remote('tar -xf - -C '+shlex.quote(remote_root),stdin=bundle)
        command=("trap 'kill -TERM \"$fixture_pid\" 2>/dev/null; wait \"$fixture_pid\"' EXIT; "
                 +'HOME='+shlex.quote(remote_root+'/home')+' '+shlex.quote(args.remote_binary)+' '+shlex.quote(remote_root+'/remote-host.json')
                 +' </dev/null & fixture_pid=$!; cat >/dev/null; kill -TERM "$fixture_pid"; wait "$fixture_pid"; result=$?; trap - EXIT; exit "$result"')
        hostp=subprocess.Popen(['ssh','-o','BatchMode=yes',args.ssh_host,
                               'runuser -u '+shlex.quote(args.remote_user)+' -- sh -c '+shlex.quote(command)],
                              stdin=subprocess.PIPE,stdout=log,stderr=log)
        tunnel=subprocess.Popen(['ssh','-o','BatchMode=yes','-o','ExitOnForwardFailure=yes','-N',
                                 '-L',f'{port}:127.0.0.1:{remote_port}',args.ssh_host],stdout=log,stderr=log)
        processes.append(tunnel)
        (root/'remote-receipt.json').write_text(json.dumps({'schema':'verse.battle.remote-host.v1','host':args.ssh_host,
            'compiled_revision':args.compiled_revision,'binary_sha256':remote('sha256sum '+shlex.quote(args.remote_binary),capture_output=True,text=True).stdout.split()[0],
            'system':remote('uname -srmo; ps -eo comm,pcpu --sort=-pcpu | head -12',capture_output=True,text=True).stdout,
            'limits':['Linux server CPU is shared with other work.','SSH forwarding adds transport overhead.','Native client and headless drivers share the Mac.','Host CPU process samples are unavailable in this fixture.']}))
    else:
        hostp=subprocess.Popen([str(binaries/'verse_host'),str(root/'host.json')],stdout=log,stderr=log,env=env)
    processes.append(hostp)
    for _ in range(100):
        if hostp.poll() is not None:raise RuntimeError('Host failed')
        if 'listening' in (root/'host.log').read_text():break
        time.sleep(.1)
    else:raise RuntimeError('Host readiness timed out')
    if args.ssh_host:
        for _ in range(100):
            if tunnel.poll() is not None:
                raise RuntimeError('SSH forwarding failed')
            try:
                with socket.create_connection(('127.0.0.1',port),timeout=.1):
                    break
            except OSError:
                time.sleep(.1)
        else:raise RuntimeError('SSH forwarding readiness timed out')
    proxylog=open(root/'proxy.log','w');logs.append(proxylog)
    proxy=subprocess.Popen(['python3',str(repo/'scripts/bench/verse-delayed-route.py'),'--destination-port',str(port),'--connections',str(args.players),'--delay-ms',str(args.delay_ms),'--jitter-ms',str(args.jitter_ms),'--seconds',str(min(300,args.seconds+100)),'--ready',str(root/'proxy-ready.json'),'--receipt',str(root/'proxy-receipt.json')],stdout=proxylog,stderr=proxylog,env=env)
    processes.append(proxy)
    for _ in range(100):
        if proxy.poll() is not None:raise RuntimeError('Proxy failed')
        if (root/'proxy-ready.json').exists():break
        time.sleep(.05)
    else:raise RuntimeError('Proxy readiness timed out')
    delayed_address=json.loads((root/'proxy-ready.json').read_text())['address']
    loadcfg={'address':delayed_address,'server_name':'localhost','instance':220,'trust_der':str(root/'cert.der'),'keys':[str(root/(r+'.key')) for r in roles[1:]],'pack':pack,'scene':scene,'dir':str(assets),'seconds':args.seconds+30,'output':str(root/'load-receipt.json'),'movement_frames':args.movement_frames}
    (root/'load-config.json').write_text(json.dumps(loadcfg))
    loadlog=open(root/'load.log','w');logs.append(loadlog)
    loadp=subprocess.Popen([str(binaries/'verse_load'),str(root/'load-config.json')],stdout=loadlog,stderr=loadlog,env=env);processes.append(loadp)
    for _ in range(400):
        if loadp.poll() is not None:raise RuntimeError('Headless load failed before readiness')
        if 'Load ready' in (root/'load.log').read_text():break
        time.sleep(.1)
    else:raise RuntimeError('Headless load readiness timed out')
    print('Headless load ready; launching the native primary player',flush=True)
    clients=[]
    for role in ['primary']:
        cfg={'address':delayed_address,'server_name':'localhost','instance':220,'trust_der':str(root/'cert.der'),'key_file':str(root/(role+'.key')),'pack':pack,'scene':scene,'dir':str(assets),'record':{'output':str(root/(role+'.mp4')),'seconds':args.seconds,'controller':role!='spectator','respawn':role!='spectator','movement':role!='spectator','movement_frames':args.movement_frames and role!='spectator'}}
        path=root/(role+'.json');path.write_text(json.dumps(cfg))
        log=open(root/(role+'.log'),'w');logs.append(log)
        p=subprocess.Popen([str(binaries/'verse_remote'),str(path)],stdout=log,stderr=log,env=env);processes.append(p);clients.append((role,p))
    started=time.monotonic()
    host_cpu_started=started
    sample_host_cpu(hostp)
    while any(p.poll() is None for _,p in clients):
        if time.monotonic()-started>args.seconds+100:raise RuntimeError('Client deadline exceeded')
        if args.sample_host and sample_process is None and time.monotonic()-started>=10:
            samplelog=open(root/'sample.log','w');logs.append(samplelog)
            sample_process=subprocess.Popen(['sample',str(hostp.pid),'5','1','-file',str(root/'host-cpu-sample.txt')],stdout=samplelog,stderr=samplelog,env=env)
            processes.append(sample_process)
            sample_receipt.update({'status':'running','host_pid':hostp.pid,'duration_seconds':5,
                                   'interval_ms':1,'capture_start_seconds':time.monotonic()-started,
                                   'compiled_revision':args.compiled_revision})
        sample_host_cpu(hostp)
        time.sleep(1)
    loadp.wait(timeout=50)
    if sample_process is not None:
        sample_receipt['exit_code']=sample_process.wait(timeout=30)
        sample_receipt['status']='complete' if sample_process.returncode==0 else 'failed'
    sample_host_cpu(hostp)
    codes={role:p.returncode for role,p in clients};codes['load']=loadp.returncode
    print('Client exits '+json.dumps(codes),flush=True)
    proxy.terminate();proxy.wait(timeout=10)
    stop_host()
    print('Host exit '+str(hostp.returncode),flush=True)
    (root/'exits.json').write_text(json.dumps({'clients':codes,'host':hostp.returncode,'proxy':proxy.returncode}))
    if any(codes.values()) or hostp.returncode:raise RuntimeError('Acceptance process failed')
finally:
    try:
        stop_host()
    except (OSError,subprocess.TimeoutExpired) as error:
        print('Host cleanup could not be confirmed: '+str(error),flush=True)
    for p in processes:
        if p.poll() is None:
            p.terminate()
            try:p.wait(timeout=10)
            except subprocess.TimeoutExpired:p.kill();p.wait()
    if remote_root and (hostp is None or hostp.returncode==0):
        remote('rm -rf -- '+shlex.quote(remote_root))
    elif remote_root:
        print('Retained remote scratch after unsuccessful host exit: '+remote_root,flush=True)
    for log in logs:log.close()
    if sample_process is not None and sample_receipt['status']=='running':
        sample_receipt.update({'status':'interrupted','exit_code':sample_process.returncode})
    (root/'sample-receipt.json').write_text(json.dumps(sample_receipt,indent=2)+'\n')
    (root/'host-cpu.json').write_text(json.dumps({
        'schema':'verse.host.cpu-observation.v1','samples':host_cpu_samples,'sample_errors':host_cpu_errors,
        'limits':['External process CPU time includes all host threads, not isolated simulation stages.',
                  'CPU time precision depends on the system ps implementation.',
                  'Samples cover native capture and load completion; startup is excluded.']},indent=2)+'\n')
print('Artifacts '+str(root),flush=True)
