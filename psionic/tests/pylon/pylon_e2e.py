"""Chapter 19 end to end: a Pylon answers decisions over our own Coax relay.

Tooling only (standard library). Builds the Coax relay and the Pylon with
ASan and UBSan, starts the relay on 127.0.0.1, an engine, the Pylon, and the
TEST-ONLY buyer (buyer.ts), then checks the Pylon's own log and that no
sanitizer reported. Never touches a public relay: the only relay is the one
started here, on a free local port.

  python3 -I psionic/tests/pylon/pylon_e2e.py [OUT_DIR]

Environment:
  COAX     the compiler (default coax/bin/coax)
  NO_BUILD 1 reuses the binaries already in OUT_DIR
  SERVE    a built psionic-serve (default: built into OUT_DIR, release)
  ENGINE   http://HOST:PORT of a running /v1/systemone (default: start
           psionic-serve --toy here with --fixture-delay-ms 2500)
  MODEL    the engine's model name (default psionic-toy; elephant-1 with ENGINE)
  BUN, NOBLE  bun, and a node_modules holding @noble and nostr-effect
"""
import os, pathlib, shutil, socket, subprocess, sys, tempfile, time

ROOT = pathlib.Path(__file__).resolve().parents[3]
HERE = pathlib.Path(__file__).resolve().parent
COAX = os.environ.get('COAX', str(ROOT / 'coax/bin/coax'))
BUN = shutil.which(os.environ.get('BUN', 'bun')) or 'bun'
NOBLE = os.environ.get('NOBLE', '/home/christopherdavid/oa-board/coax/web/project/node_modules')
ENGINE = os.environ.get('ENGINE', '')
MODEL = os.environ.get('MODEL', 'elephant-1' if ENGINE else 'psionic-toy')
SLUG = 'ch19-local'
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
    out = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix='pylon-e2e-'))
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
    subprocess.run([str(ROOT / 'psionic/platform/pylon-build.sh'), str(out / 'pylon-build'), 'sanitized'], check=True, env=env,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if not ENGINE and not os.environ.get('SERVE'):
        # serve-build.sh also builds the supervisor; only psionic-serve is needed here.
        subprocess.run([str(ROOT / 'psionic/platform/serve-build.sh'), str(out / 'serve'), 'release', 'plain'], env=env,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def run(out, serve):
    procs = []
    try:
        rport = free_port()
        relay_log = open(out / 'relay.log', 'w')
        procs.append(subprocess.Popen([str(out / 'relay'), '--listen', '127.0.0.1', '--port', str(rport)], stdout=relay_log, stderr=subprocess.STDOUT))
        check(wait_port(rport, 30), 'the Coax relay listens on 127.0.0.1:%d' % rport)
        engine = ENGINE
        if not engine:
            eport = free_port()
            serve_log = open(out / 'serve.log', 'w')
            procs.append(subprocess.Popen([serve, '--toy', '--port', str(eport), '--fixture-delay-ms', '2500'], stdout=serve_log, stderr=subprocess.STDOUT))
            check(wait_port(eport, 60), 'psionic-serve --toy listens on 127.0.0.1:%d' % eport)
            engine = 'http://127.0.0.1:%d' % eport
        stop = out / 'stop'
        if stop.exists():
            stop.unlink()
        relay_url = 'ws://127.0.0.1:%d' % rport
        pylon_log = out / 'pylon.log'
        pylon = subprocess.Popen([str(out / 'pylon-build/pylon'), '--relay', relay_url, '--engine', engine, '--model', MODEL,
                                  '--slug', SLUG, '--label', 'Chapter 19 local', '--test-key-label', 'psionic ch19 pylon test key',
                                  '--signer-bun', BUN, '--signer-script', str(HERE / 'sign_double.ts'), '--node-path', NOBLE,
                                  '--stop-file', str(stop), '--decide-ms', '30000'],
                                 stdout=open(pylon_log, 'w'), stderr=open(out / 'pylon.err', 'w'))
        procs.append(pylon)
        check(wait_line(pylon_log, 'pylon: online, waiting for jobs', 120, pylon), 'the Pylon authenticated, published its list and beacon, and subscribed')
        benv = {**os.environ, 'NODE_PATH': NOBLE, 'RELAY': relay_url, 'SLUG': SLUG, 'MODEL': MODEL, 'STOP_FILE': str(stop),
                'CANCEL': '0' if ENGINE else '1'}
        buyer = subprocess.run([BUN, str(HERE / 'buyer.ts')], env=benv, capture_output=True, text=True, timeout=600)
        sys.stdout.write(buyer.stdout)
        if buyer.stderr.strip():
            print(buyer.stderr[-2000:])
        check(buyer.returncode == 0 and 'buyer: ok' in buyer.stdout, 'the buyer\'s checks held')
        try:
            pylon.wait(60)
        except subprocess.TimeoutExpired:
            pass
        log = pylon_log.read_text(errors='replace')
        err = (out / 'pylon.err').read_text(errors='replace')
        print('--- pylon log ---\n' + log + '--- end ---', flush=True)
        check(pylon.returncode == 0 and 'pylon: offline.' in log, 'the Pylon stopped cleanly after the offline beacon')
        check('refused 0,' in log and 'forged 0' in log, 'the relay accepted every event the Pylon published; no forged input')
        sanitizer = [s for s in ('AddressSanitizer', 'runtime error', 'LeakSanitizer') if s in err]
        check(not sanitizer, 'no sanitizer report from the Pylon')
        relay_log.flush()
        check(not any(s in (out / 'relay.log').read_text(errors='replace') for s in ('AddressSanitizer', 'runtime error')), 'no sanitizer report from the relay')
    finally:
        for p in procs:
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(10)
                except subprocess.TimeoutExpired:
                    p.kill()
    print('pylon e2e: ok' if not failed else 'pylon e2e: %d failed' % len(failed))
    return 0 if not failed else 1


if __name__ == '__main__':
    sys.exit(main())
