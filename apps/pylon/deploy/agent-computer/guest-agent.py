#!/usr/bin/env python3
# OpenAgents Agent Computer vsock guest agent (#8503 / CX-3 #8547).
#
# This is the PROVEN in-guest control-channel agent extracted verbatim from the
# validated baked rootfs on agent-computer-gce-1 (rootfsDigest
# sha256:2eb563b3f52e...; see agent-computer-image.manifest.json) so the bake
# is source-controlled. Protocol (reference for the host-side client and for
# `oa-codex-control`'s `guest_exec` / `guest_copy_out` wiring): connect the
# firecracker vsock UDS, send `CONNECT 1024\n`, then length-prefixed
# (big-endian u32) JSON messages {op: ping|exec|copyout}.
#
# It binds AF_VSOCK :1024 (no inbound network — vsock only), runs as
# `agent-guest.service`, and carries no credentials of its own.
import socket, struct, json, subprocess, base64, io, tarfile, os
PORT=1024
def rall(c,n):
    b=b''
    while len(b)<n:
        p=c.recv(n-len(b))
        if not p: raise EOFError()
        b+=p
    return b
def rmsg(c):
    (n,)=struct.unpack('>I',rall(c,4)); return json.loads(rall(c,n).decode())
def wmsg(c,o):
    d=json.dumps(o).encode(); c.sendall(struct.pack('>I',len(d))+d)
def handle(r):
    op=r.get('op')
    if op=='ping': return {'code':0,'output':'ready'}
    if op=='exec':
        try:
            env=dict(os.environ); env.update(r.get('env',{}))
            p=subprocess.run(r['command'],capture_output=True,text=True,timeout=r.get('timeout',3600),cwd=r.get('cwd'),env=env)
            return {'code':p.returncode,'output':(p.stdout or '')+(p.stderr or '')}
        except Exception as e:
            return {'code':127,'output':f'guest-exec-error: {e}'}
    if op=='copyout':
        buf=io.BytesIO()
        with tarfile.open(fileobj=buf,mode='w') as t:
            if os.path.exists(r['path']): t.add(r['path'],arcname=os.path.basename(r['path'].rstrip('/')))
        return {'code':0,'b64tar':base64.b64encode(buf.getvalue()).decode()}
    return {'code':2,'output':f'unknown op {op}'}
def main():
    s=socket.socket(socket.AF_VSOCK,socket.SOCK_STREAM)
    s.bind((socket.VMADDR_CID_ANY,PORT)); s.listen(16)
    open('/opt/agent/ready','w').write('1')
    while True:
        c,_=s.accept()
        try: wmsg(c,handle(rmsg(c)))
        except Exception: pass
        finally: c.close()
main()
