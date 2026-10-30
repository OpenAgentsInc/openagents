#!/usr/bin/env python3
"""The verified store's append-only file, end to end (nostr-event-store #8eb5).

Builds coax/tests/nostr/store_file_main.coax with store-file-native.c under
ASan+UBSan, runs its phases over one file, and checks each run's output and
the file's exact bytes against the state computed here:

  phase 1 on an empty file: admissions, of which only stored originals are saved
  replay: the lowest original comes back, verified again
  phase 2: a lower original is saved and wins after the next replay
  a torn tail and a bad length are cut off; a frame forged in the file and a
  frame that is not an event are refused, and the rest still replays
  a second writer cannot open a locked file

Test tooling only. Usage: python3 coax/tests/nostr/store_file.py
"""
import fcntl, os, re, struct, subprocess, sys, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
COAX = os.environ.get('COAX', str(ROOT / 'coax/bin/coax'))

def literal(path, name):
    src = (ROOT / path).read_text()
    m = re.search(name + r' :: "((?:[^"\\]|\\.)*)";', src)
    return re.sub(r'\\(.)', lambda g: g.group(1), m.group(1)).encode()

A = literal('coax/lib/nostr/store_admit.coax', 'NS_VALID_TEST')
B = literal('coax/tests/nostr/store_fixtures.coax', 'SVC_REORDERED')
C = literal('coax/tests/nostr/store_fixtures.coax', 'SVC_ESCAPED')
assert sorted([A, B, C]) == [B, C, A]
def frame(b): return struct.pack('>I', len(b)) + b
ACCEPT, DUP, BAD_SIG = 0, 1, 5
NONE, REORDERED, ESCAPED, SIGNED = 0, 1, 2, 3
fails = 0
def check(name, got, want):
    global fails
    ok = got == want
    fails += not ok
    print(('  ok   ' if ok else '  FAIL ') + name + ('' if ok else f': got {got!r}, want {want!r}'))

def main():
    with tempfile.TemporaryDirectory(prefix='store-file-') as tmp:
        tmp = Path(tmp)
        c = subprocess.run([COAX, 'c', str(HERE / 'store_file_main.coax')], check=True, capture_output=True).stdout
        (tmp / 'main.c').write_bytes(c)
        flags = ['-std=c11', '-O1', '-g', '-fsanitize=address,undefined', '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']
        exe = tmp / 'host'
        subprocess.run(['cc', *flags, '-Dmain=coax_store_file_main', '-c', str(tmp / 'main.c'), '-o', str(tmp / 'main.o')], check=True)
        subprocess.run(['cc', *flags, '-I', str(ROOT / 'coax/platform'), str(HERE / 'store_file_host.c'),
                        str(ROOT / 'coax/platform/store-file-native.c'), str(tmp / 'main.o'), '-lm', '-lpthread', '-o', str(exe)], check=True)
        store = tmp / 'events.log'
        def run(phase):
            p = subprocess.run([str(exe), str(store), str(phase)], capture_output=True, text=True, timeout=120,
                               env={**os.environ, 'ASAN_OPTIONS': 'halt_on_error=1:detect_leaks=0'})
            if p.returncode != 0 or p.stderr.strip():
                check(f'phase {phase} exits cleanly', (p.returncode, p.stderr[-400:]), (0, ''))
            return [int(x) for x in p.stdout.split()]
        # replay: ok frames stored kept refused cut which
        out = run(1)
        check('an empty file replays as an empty store', out[:7], [1, 0, 0, 0, 0, 0, NONE])
        check('phase 1 admissions', out[7:], [ACCEPT, DUP, BAD_SIG, BAD_SIG, DUP, DUP, ACCEPT, ESCAPED])
        want = frame(A) + frame(C)
        check('the file holds exactly the two stored originals', store.read_bytes() == want, True)
        check('the file is private', oct(store.stat().st_mode & 0o777), '0o600')
        check('restart: the lowest original comes back', run(0), [1, 2, 1, 1, 0, 0, ESCAPED])
        out = run(2)
        check('phase 2: a lower original is saved', out, [1, 2, 1, 1, 0, 0, ESCAPED, DUP, REORDERED])
        want += frame(B)
        check('the file gained its frame', store.read_bytes() == want, True)
        check('restart: it wins', run(0), [1, 3, 1, 2, 0, 0, REORDERED])
        with open(store, 'ab') as f: f.write(b'\x00\x00\x01\x00xyz')
        check('a torn tail is cut off', run(0), [1, 3, 1, 2, 0, 1, REORDERED])
        check('back to the whole frames', store.read_bytes() == want, True)
        with open(store, 'ab') as f: f.write(b'\x00\x00\x00\x00')
        check('a zero length is cut off', run(0), [1, 3, 1, 2, 0, 1, REORDERED])
        check('the cut file', store.read_bytes() == want, True)
        with open(store, 'ab') as f: f.write(frame(b'{}'))
        check('a frame that is not an event is refused', run(0), [1, 4, 1, 2, 1, 0, REORDERED])
        forged = bytearray(store.read_bytes())
        at = len(frame(A)) + len(frame(C)) + 4 + B.index(b'"sig":"') + 7
        forged[at] = ord('0') if forged[at] != ord('0') else ord('1')
        store.write_bytes(bytes(forged))
        check('a signature forged in the file is refused, the rest replays', run(0), [1, 4, 1, 1, 2, 0, ESCAPED])
        with open(store, 'rb+') as f:
            fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
            check('a second writer cannot open a locked file', run(0), [0, 0, 0, 0, 0, 0, NONE])
    print('store_file: ' + ('ok' if fails == 0 else f'{fails} failed'))
    return 1 if fails else 0

if __name__ == '__main__':
    sys.exit(main())
