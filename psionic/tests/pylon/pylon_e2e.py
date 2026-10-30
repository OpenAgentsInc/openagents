"""Chapter 19 end to end: a Pylon answers decisions over our own Coax relay.

Tooling only (standard library). Builds the Coax relay and the Pylon with
ASan and UBSan, starts the relay on 127.0.0.1 (with --store, so its events
survive a restart), an engine, the Pylon, and the TEST-ONLY buyer
(buyer.ts), then checks the Pylon's own log and that no sanitizer reported.
Midway the relay is stopped while a job is with the engine and started again
on the same port and store; the Pylon must reconnect, re-authenticate,
republish its beacon, resubscribe and answer both that job and one sent
while it was away. Jobs are ephemeral (kind 25910): no relay stores them,
so the buyer sends that job's same signed event again once the Pylon's
beacon is back, and the Pylon must answer it exactly once. Never touches a public relay: the only relay is the one
started here, on a free local port.

With --wss the Pylon reaches the relay through wss://localhost:PORT: a TLS
proxy here (Python's ssl module) in front of the plain relay, with a
throwaway CA and localhost leaf that openssl generates into OUT_DIR for this
run. The Pylon is built with -DCXTLS_TEST_TRUST and trusts only that CA
(CXTLS_TEST_CA); nothing is committed.

  python3 -I psionic/tests/pylon/pylon_e2e.py [--wss] [OUT_DIR]

Environment:
  COAX     the compiler (default coax/bin/coax)
  NO_BUILD 1 reuses the binaries already in OUT_DIR
  SERVE    a built psionic-serve (default: built into OUT_DIR, release)
  ENGINE   http://HOST:PORT of a running /v1/systemone (default: start
           psionic-serve --toy here with --fixture-delay-ms 2500)
  MODEL    the engine's model name (default psionic-toy; elephant-1 with ENGINE)
  RESTART  0 skips the relay restart case (default 1)
  BUN, NOBLE  bun, and a node_modules holding @noble and nostr-effect
"""
import os, pathlib, shutil, socket, ssl, subprocess, sys, tempfile, threading, time

ROOT = pathlib.Path(__file__).resolve().parents[3]
HERE = pathlib.Path(__file__).resolve().parent
COAX = os.environ.get('COAX', str(ROOT / 'coax/bin/coax'))
BUN = shutil.which(os.environ.get('BUN', 'bun')) or 'bun'
NOBLE = os.environ.get('NOBLE', '/home/christopherdavid/oa-board/coax/web/project/node_modules')
ENGINE = os.environ.get('ENGINE', '')
MODEL = os.environ.get('MODEL', 'elephant-1' if ENGINE else 'psionic-toy')
SLUG = 'ch19-local'
RESTART = os.environ.get('RESTART', '1') != '0'
WSS = '--wss' in sys.argv[1:]
ARGS = [a for a in sys.argv[1:] if a != '--wss']
failed = []


def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what, flush=True)
    if not ok:
        failed.append(what)


def free_port():
    s = socket.socket(); s.bind(('127.0.0.1', 0)); p = s.getsockname()[1]; s.close(); return p


def wait_port(port, seconds):
    end = time.time() + seconds
    while time.time() < end:
        try:
            socket.create_connection(('127.0.0.1', port), 0.5).close(); return True
        except OSError:
            time.sleep(0.2)
    return False


def wait_line(path, text, seconds, proc):
    end = time.time() + seconds
    while time.time() < end:
        if text in pathlib.Path(path).read_text(errors='replace'):
            return True
        if proc.poll() is not None:
            return False
        time.sleep(0.2)
    return False


def main():
    out = pathlib.Path(ARGS[0] if ARGS else tempfile.mkdtemp(prefix='pylon-e2e-'))
    out.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, 'COAX': COAX}
    t = time.time()
    if os.environ.get('NO_BUILD') != '1':
        build(out, env)
    serve = os.environ.get('SERVE', str(out / 'serve/psionic-serve'))
    print('built in %.0f s' % (time.time() - t), flush=True)
    return run(out, serve)


def build(out, env):
    subprocess.run([str(ROOT / 'coax/relay/build.sh'), str(out / 'relay'), '--sanitize'], check=True, env=env, stdout=subprocess.DEVNULL)
    subprocess.run([str(ROOT / 'psionic/platform/pylon-build.sh'), str(out / 'pylon-build'), 'sanitized', '--test-trust'], check=True, env=env,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if not ENGINE and not os.environ.get('SERVE'):
        # serve-build.sh also builds the supervisor; only psionic-serve is needed here.
        subprocess.run([str(ROOT / 'psionic/platform/serve-build.sh'), str(out / 'serve'), 'release', 'plain'], env=env,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def certs(d):
    """A one-day test CA and a localhost leaf it signs, in d (as coax/tests/net/relay_e2e.py)."""
    def run(*args):
        subprocess.run(['openssl', *args], check=True, capture_output=True)
    run('req', '-x509', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:prime256v1', '-nodes', '-days', '1',
        '-keyout', str(d / 'ca.key'), '-out', str(d / 'ca.pem'), '-subj', '/CN=Pylon e2e test CA',
        '-addext', 'basicConstraints=critical,CA:TRUE', '-addext', 'keyUsage=critical,keyCertSign,cRLSign')
    run('req', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:prime256v1', '-nodes',
        '-keyout', str(d / 'leaf.key'), '-out', str(d / 'leaf.csr'), '-subj', '/CN=localhost')
    (d / 'leaf.ext').write_text('subjectAltName=DNS:localhost\nbasicConstraints=critical,CA:FALSE\n'
                                'keyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\n')
    run('x509', '-req', '-in', str(d / 'leaf.csr'), '-CA', str(d / 'ca.pem'), '-CAkey', str(d / 'ca.key'),
        '-set_serial', '0x' + os.urandom(8).hex(), '-days', '1', '-extfile', str(d / 'leaf.ext'), '-out', str(d / 'leaf.pem'))
    return d / 'ca.pem', d / 'leaf.pem', d / 'leaf.key'


class TlsProxy:
    """wss:// in front of the plain relay: TLS on 127.0.0.1:port, bytes piped
    to 127.0.0.1:upstream. When the relay side ends, the client gets
    close_notify, as a TLS relay closing a connection sends."""
    def __init__(self, cert, key, upstream):
        self.upstream = upstream
        self.ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        self.ctx.minimum_version = ssl.TLSVersion.TLSv1_2
        self.ctx.load_cert_chain(str(cert), str(key))
        self.server = socket.socket()
        self.server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.server.bind(('127.0.0.1', 0)); self.server.listen(16)
        self.port = self.server.getsockname()[1]
        self.handshakes = 0
        threading.Thread(target=self.accept, daemon=True).start()

    def accept(self):
        while True:
            try:
                raw, _ = self.server.accept()
            except OSError:
                return
            threading.Thread(target=self.serve, args=(raw,), daemon=True).start()

    def serve(self, raw):
        try:
            tls = self.ctx.wrap_socket(raw, server_side=True)
        except (OSError, ssl.SSLError):
            raw.close(); return
        self.handshakes += 1
        try:
            up = socket.create_connection(('127.0.0.1', self.upstream), 2)
        except OSError:
            self.end(tls); return
        up.settimeout(None)
        done = threading.Event()

        def pump(a, b):
            try:
                while True:
                    data = a.recv(65536)
                    if not data:
                        break
                    b.sendall(data)
            except (OSError, ssl.SSLError):
                pass
            done.set()
        threading.Thread(target=pump, args=(tls, up), daemon=True).start()
        threading.Thread(target=pump, args=(up, tls), daemon=True).start()
        done.wait()
        try:
            up.close()
        except OSError:
            pass
        self.end(tls)

    @staticmethod
    def end(tls):
        try:
            tls.unwrap().close()
        except (OSError, ValueError, ssl.SSLError):
            try:
                tls.close()
            except OSError:
                pass

    def stop(self):
        self.server.close()


def start_relay(out, rport, url, n):
    log = open(out / ('relay-%d.log' % n), 'w')
    return subprocess.Popen([str(out / 'relay'), '--listen', '127.0.0.1', '--port', str(rport), '--url', url,
                             '--store', str(out / 'relay.store')], stdout=log, stderr=subprocess.STDOUT)


def stop(p):
    if p.poll() is None:
        p.terminate()
        try:
            p.wait(10)
        except subprocess.TimeoutExpired:
            p.kill(); p.wait()


def run(out, serve):
    procs = []
    relays = []
    proxy = None
    try:
        rport = free_port()
        store = out / 'relay.store'
        if store.exists():
            store.unlink()
        env = dict(os.environ)
        if WSS:
            ca, cert, key = certs(out)
            proxy = TlsProxy(cert, key, rport)
            relay_url = 'wss://localhost:%d' % proxy.port
            env['CXTLS_TEST_CA'] = str(ca)
        else:
            relay_url = 'ws://localhost:%d' % rport
        env.pop('CXTLS_TEST_CA', None) if not WSS else None
        relays.append(start_relay(out, rport, relay_url, 1))
        check(wait_port(rport, 30), 'the Coax relay listens on 127.0.0.1:%d (NIP-42 URL %s, --store)' % (rport, relay_url))
        engine = ENGINE
        if not engine:
            eport = free_port()
            serve_log = open(out / 'serve.log', 'w')
            procs.append(subprocess.Popen([serve, '--toy', '--port', str(eport), '--fixture-delay-ms', '2500'], stdout=serve_log, stderr=subprocess.STDOUT))
            check(wait_port(eport, 60), 'psionic-serve --toy listens on 127.0.0.1:%d' % eport)
            engine = 'http://127.0.0.1:%d' % eport
        stop_file, restart_file, up_file = out / 'stop', out / 'restart', out / 'up'
        for f in (stop_file, restart_file, up_file):
            if f.exists():
                f.unlink()
        pylon_log = out / 'pylon.log'
        pylon = subprocess.Popen([str(out / 'pylon-build/pylon'), '--relay', relay_url, '--engine', engine, '--model', MODEL,
                                  '--slug', SLUG, '--label', 'Chapter 19 local', '--test-key-label', 'psionic ch19 pylon test key',
                                  '--signer-bun', BUN, '--signer-script', str(HERE / 'sign_double.ts'), '--node-path', NOBLE,
                                  '--stop-file', str(stop_file), '--decide-ms', '30000'],
                                 stdout=open(pylon_log, 'w'), stderr=open(out / 'pylon.err', 'w'), env=env)
        procs.append(pylon)
        check(wait_line(pylon_log, 'pylon: online, waiting for jobs', 120, pylon), 'the Pylon authenticated, published its list and beacon, and subscribed')
        benv = {**os.environ, 'NODE_PATH': NOBLE, 'RELAY': relay_url, 'CONNECT': 'ws://localhost:%d' % rport, 'SLUG': SLUG,
                'MODEL': MODEL, 'STOP_FILE': str(stop_file), 'CANCEL': '0' if ENGINE else '1'}
        if RESTART:
            benv.update(RESTART_FILE=str(restart_file), UP_FILE=str(up_file))
        buyer_out = open(out / 'buyer.log', 'w')
        buyer = subprocess.Popen([BUN, str(HERE / 'buyer.ts')], env=benv, stdout=buyer_out, stderr=subprocess.STDOUT)
        end = time.time() + 600
        restarted = False
        while buyer.poll() is None and time.time() < end:
            if RESTART and not restarted and restart_file.exists():
                # r-7 is with the engine (2.5 s with the toy): stop the relay
                # under it, keep it away long enough for the Pylon's answer to
                # wait and its first reconnects to fail, then start it again.
                stop(relays[-1])
                print('     relay stopped at %.1f s; down for 4 s' % (time.time() - t_start), flush=True)
                time.sleep(4)
                relays.append(start_relay(out, rport, relay_url, 2))
                check(wait_port(rport, 30), 'the relay restarted on the same port and store')
                up_file.write_text('up\n')
                restarted = True
            time.sleep(0.1)
        if buyer.poll() is None:
            buyer.kill()
        buyer.wait()
        buyer_out.close()
        btext = (out / 'buyer.log').read_text(errors='replace')
        sys.stdout.write(btext)
        check(buyer.returncode == 0 and 'buyer: ok' in btext, 'the buyer\'s checks held')
        try:
            pylon.wait(60)
        except subprocess.TimeoutExpired:
            pass
        log = pylon_log.read_text(errors='replace')
        err = (out / 'pylon.err').read_text(errors='replace')
        print('--- pylon log ---\n' + log + '--- end ---', flush=True)
        check(pylon.returncode == 0 and 'pylon: offline.' in log, 'the Pylon stopped cleanly after the offline beacon')
        check('refused 0,' in log and 'forged 0' in log, 'the relay accepted every event the Pylon published; no forged input')
        if RESTART:
            opens = [l for l in log.splitlines() if ' open to ' in l]
            auths = [l for l in log.splitlines() if 'pylon: authenticated' in l]
            beacons = [l for l in log.splitlines() if 'pylon: beacon ' in l and '(connection 1)' not in l]
            reqs = [l for l in log.splitlines() if 'pylon: REQ ' in l]
            check(len(opens) >= 2 and 'ended (' in log, 'the Pylon saw the drop and reconnected (%d connections opened)' % len(opens))
            check(len(auths) >= 2 and len(set(a.split('(connection ')[1] for a in auths)) == len(auths),
                  'it re-authenticated on each connection, once per new challenge (%d AUTHs)' % len(auths))
            check(len(reqs) >= 2 and int(reqs[-1].split(' since ')[1].split()[0]) > int(reqs[0].split(' since ')[1].split()[0]),
                  'it resubscribed with since moved to the newest job delivered (%s -> %s)' % (reqs[0].split(' since ')[1].split()[0], reqs[-1].split(' since ')[1].split()[0]))
            check(any('online' in b for b in beacons), 'it republished its beacon on the new connection')
        sanitizer = [s for s in ('AddressSanitizer', 'runtime error', 'LeakSanitizer') if s in err]
        check(not sanitizer, 'no sanitizer report from the Pylon')
        rl = ''.join(f.read_text(errors='replace') for f in sorted(out.glob('relay-*.log')))
        check(not any(s in rl for s in ('AddressSanitizer', 'runtime error')), 'no sanitizer report from the relay')
        if proxy:
            check(proxy.handshakes >= (2 if RESTART else 1), 'the Pylon spoke TLS to the proxy (%d TLS handshakes)' % proxy.handshakes)
    finally:
        for p in procs + relays:
            stop(p)
        if proxy:
            proxy.stop()
    print('pylon e2e%s: ok' % (' (wss)' if WSS else '') if not failed else 'pylon e2e: %d failed' % len(failed))
    return 0 if not failed else 1


t_start = time.time()


if __name__ == '__main__':
    sys.exit(main())
