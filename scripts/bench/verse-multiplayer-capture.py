#!/usr/bin/env python3
"""Record a temporary two-player/spectator chamber through delayed loopback TLS."""
import argparse, json, os, pathlib, socket, subprocess, tempfile, time
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--asset-dir',type=pathlib.Path,required=True)
parser.add_argument('--binaries',type=pathlib.Path,required=True)
parser.add_argument('--seconds',type=int,default=60)
parser.add_argument('--delay-ms',type=int,default=40)
parser.add_argument('--jitter-ms',type=int,default=20)
args=parser.parse_args()
if not (1<=args.seconds<=120 and 0<=args.delay_ms<=250 and 0<=args.jitter_ms<=100):
    parser.error('Duration, delay, or jitter exceeds fixture bounds')
root=pathlib.Path(tempfile.mkdtemp(prefix='verse-prediction-delayed-'))
os.chmod(root,0o700)
print('Scratch artifacts '+str(root),flush=True)
repo=pathlib.Path(__file__).resolve().parents[2]
assets=args.asset_dir.resolve()
binaries=args.binaries.resolve()
def openssl(*args):
    return subprocess.check_output(['openssl',*map(str,args)],stderr=subprocess.DEVNULL)
keys=[]
for role in ['primary','player','spectator']:
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
scene=str(repo/'assets/verse/original/ritual.json');pack=str(assets/'runtime-pack.json')
host={'listen':address,'instance':220,'scene':scene,'pack':pack,'certificate_der':str(root/'cert.der'),'private_key_der':str(root/'tls.der'),'enrollments':[{'public_key':k,'role':({'type':'player','spawn':[3,0,-22]} if role=='player' else {'type':role})} for role,k in zip(['primary','player','spectator'],keys)]}
(root/'host.json').write_text(json.dumps(host))
env=dict(os.environ)
(root/'home').mkdir()
env['HOME']=str(root/'home')
logs=[];processes=[]
try:
    log=open(root/'host.log','w');logs.append(log)
    hostp=subprocess.Popen([str(binaries/'verse_host'),str(root/'host.json')],stdout=log,stderr=log,env=env);processes.append(hostp)
    for _ in range(100):
        if hostp.poll() is not None:raise RuntimeError('Host failed')
        if 'listening' in (root/'host.log').read_text():break
        time.sleep(.1)
    else:raise RuntimeError('Host readiness timed out')
    proxylog=open(root/'proxy.log','w');logs.append(proxylog)
    proxy=subprocess.Popen(['python3',str(repo/'scripts/bench/verse-delayed-route.py'),'--destination-port',str(port),'--delay-ms',str(args.delay_ms),'--jitter-ms',str(args.jitter_ms),'--seconds',str(min(300,args.seconds+100)),'--ready',str(root/'proxy-ready.json'),'--receipt',str(root/'proxy-receipt.json')],stdout=proxylog,stderr=proxylog,env=env)
    processes.append(proxy)
    for _ in range(100):
        if proxy.poll() is not None:raise RuntimeError('Proxy failed')
        if (root/'proxy-ready.json').exists():break
        time.sleep(.05)
    else:raise RuntimeError('Proxy readiness timed out')
    delayed_address=json.loads((root/'proxy-ready.json').read_text())['address']
    print('Host and delayed route ready; launching three native clients',flush=True)
    clients=[]
    for role in ['primary','player','spectator']:
        cfg={'address':delayed_address,'server_name':'localhost','instance':220,'trust_der':str(root/'cert.der'),'key_file':str(root/(role+'.key')),'pack':pack,'scene':scene,'dir':str(assets),'record':{'output':str(root/(role+'.mp4')),'seconds':args.seconds,'controller':role!='spectator','respawn':role!='spectator','movement':role!='spectator'}}
        path=root/(role+'.json');path.write_text(json.dumps(cfg))
        log=open(root/(role+'.log'),'w');logs.append(log)
        p=subprocess.Popen([str(binaries/'verse_remote'),str(path)],stdout=log,stderr=log,env=env);processes.append(p);clients.append((role,p))
    started=time.monotonic()
    while any(p.poll() is None for _,p in clients):
        if time.monotonic()-started>args.seconds+100:raise RuntimeError('Client deadline exceeded')
        time.sleep(1)
    codes={role:p.returncode for role,p in clients}
    print('Client exits '+json.dumps(codes),flush=True)
    proxy.terminate();proxy.wait(timeout=10)
    hostp.terminate();hostp.wait(timeout=10)
    print('Host exit '+str(hostp.returncode),flush=True)
    (root/'exits.json').write_text(json.dumps({'clients':codes,'host':hostp.returncode,'proxy':proxy.returncode}))
    if any(codes.values()) or hostp.returncode:raise RuntimeError('Acceptance process failed')
finally:
    for p in processes:
        if p.poll() is None:
            p.terminate()
            try:p.wait(timeout=10)
            except subprocess.TimeoutExpired:p.kill();p.wait()
    for log in logs:log.close()
print('Artifacts '+str(root),flush=True)
