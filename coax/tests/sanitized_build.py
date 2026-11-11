#!/usr/bin/env python3
# Firm reason: 2 (independent compiler, ABI, diagnostic and native-effect oracles; no product implementation).
"""Sanitizer objects the compiler's units share, compiled once per input (#13145).

parser.py and ct_strings.py each link the whole compiler under ASan and UBSan.
The compiler is one translation unit (src/main.cpp includes every other
source) beside megabytes of generated C, so that compile is minutes of one
core, and it used to run from nothing in every run of the unit.

objects() compiles each translation unit to an object and keeps it under a key
of what the object is made from: the C compiler's identity, the flags and the
preprocessed source (so every header the unit reads counts, and a comment or
a file it doesn't read does not). A lookup reads this machine's cache
($COAX_SANITIZED_CACHE, default ~/.cache/coax-sanitized), then the build
hosts' shared bucket (coax/remote/shared-cache.sh, sanitized/KEY.o.gz), and
only then compiles. The units of one call compile side by side, and one
compile per key runs on a machine at a time: a second caller waits for the
first one's object.

A compile is CPU-bound, and the build hosts are shared, so run() limits it by
the CPU seconds it uses, not by the clock: a host with every slot busy makes
it take several times as long and use about the same CPU. A limit on the
clock that fits an idle host fails on a busy one (the compiler's unit is 173 s
on an idle host, and its limit was 180 s). The clock is only the backstop for
a step that hangs ($COAX_SANITIZED_WALL seconds, default 1800). The link, a
few seconds, runs every time.

A run of the sanitized compiler itself gets no CPU limit: it hands its work to
a resident compiler it starts (coax serve), which would inherit the limit and
die of it some requests later. Such a run has the clock's backstop alone. A
new compiler's first check of lib/compiler/checker.coax runs every check of
the unit under the sanitizers, 660 CPU seconds: 77 s on an idle host's 20
build workers, and most of those 660 on the one worker a build keeps while the
host's load is over its budget (src/jobs.cpp). Its verdicts are then kept in
the machine's check cache under the compiler's hash, and the same objects
link to the same compiler bytes in every slot and on every host, so a later
run there reuses them (97 s, then 6 s).
"""
import fcntl
import gzip
import hashlib
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time

ROOT=Path(__file__).resolve().parents[1]
REPO=ROOT.parent
FLAGS=['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer']
KEEP_DAYS=3

def cache_dir():
    return Path(os.environ.get('COAX_SANITIZED_CACHE') or Path.home()/'.cache'/'coax-sanitized')
COMPILE_CPU=900   # CPU seconds one unit's compile may use (the compiler's unit uses about 175)
def wall():
    return int(os.environ.get('COAX_SANITIZED_WALL') or 1800)

def run(args,cpu=None,check=True,**kwargs):
    """Runs a step. With cpu, each process of it may use that many CPU seconds, however long
    the machine's load makes that take; the clock (wall()) only catches a hang. Raises
    AssertionError past a limit or (check) on a failing status; returns the completed
    process (bytes, or text with text=True)."""
    args=list(map(str,args));name=Path(args[0]).name+' '+' '.join(args[1:])[-120:]
    limited=['sh','-c',f'ulimit -t {int(cpu)}; exec "$@"','sh',*args] if cpu else args
    began=time.monotonic()
    try:r=subprocess.run(limited,capture_output=True,timeout=wall(),**kwargs)
    except subprocess.TimeoutExpired:
        raise AssertionError(f'{name}: no result after {wall()} s on the clock (COAX_SANITIZED_WALL), load {os.getloadavg()[0]:.0f}')
    err=r.stderr if isinstance(r.stderr,str) else r.stderr.decode(errors='replace')
    if cpu and (r.returncode in (-signal.SIGXCPU,-signal.SIGKILL,128+signal.SIGXCPU,128+signal.SIGKILL) or 'CPU time limit exceeded' in err):
        raise AssertionError(f'{name}: used more than {cpu} s of CPU ({time.monotonic()-began:.0f} s on the clock)\n{err}')
    if check and r.returncode:raise AssertionError(err)
    return r

def _near(path):
    """A path under the repository as relative to it, so __FILE__ and the key don't name the checkout."""
    path=Path(path).resolve()
    try:return str(path.relative_to(REPO))
    except ValueError:return str(path)

_identities={}
def _identity(tool):
    if tool not in _identities:
        out=[run([tool,arg],60,check=False,text=True).stdout for arg in ('--version','-dumpmachine')]
        _identities[tool]='\n'.join(out)
    return _identities[tool]

def _key(command):
    """What the object is made from: the tool, its flags and the preprocessed unit (no line markers)."""
    pre=run([*command,'-E','-P'],COMPILE_CPU,cwd=REPO)
    h=hashlib.sha256()
    for part in (b'coax-sanitized-v1',_identity(command[0]).encode(),'\0'.join(command[1:]).encode(),pre.stdout):
        h.update(hashlib.sha256(part).digest())
    return h.hexdigest()[:40]

def _shared():
    """The shared bucket's tool, when this job has a token for it (a build host's job)."""
    if os.environ.get('COAX_SHARED_CACHE')=='0' or not os.environ.get('COAX_SHARED_CACHE_TOKEN'):return None
    tool=Path(os.environ.get('COAX_SHARED_CACHE_TOOL') or REPO/'coax/remote/shared-cache.sh')
    return tool if os.access(tool,os.X_OK) else None

def _stat(result,began):
    """One line in the job's cache report (shared-cache.sh summary)."""
    stats=os.environ.get('COAX_SHARED_CACHE_STATS')
    if not stats:return
    try:
        with open(stats,'a') as f:f.write(f'sanitized {result} {int((time.monotonic()-began)*1e6)}\n')
    except OSError:pass

def _prune(cache):
    limit=time.time()-KEEP_DAYS*86400
    for entry in cache.glob('*.o'):
        try:
            if entry.stat().st_mtime<limit:entry.unlink();Path(str(entry)[:-2]+'.lock').unlink(missing_ok=True)
        except OSError:pass

def _object(command,label,log):
    """The object command makes (command has no -c or -o), from the caches or a compile. Returns its path."""
    began=time.monotonic();cache=cache_dir();cache.mkdir(parents=True,exist_ok=True)
    key=_key(command);kept=cache/(key+'.o');name=f'sanitized/{key}.o.gz'
    def done(how):
        _stat(how,began);log(f'sanitized {label}: {how} ({key[:12]}) in {time.monotonic()-began:.1f} s, load {os.getloadavg()[0]:.0f}')
        return kept
    with open(cache/(key+'.lock'),'w') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)   # one compile per key on this machine; a waiter finds the object
        if kept.exists():
            os.utime(kept);return done('hit')
        tool=_shared()
        with tempfile.TemporaryDirectory(prefix='.build.',dir=cache) as scratch:
            scratch=Path(scratch);fresh=scratch/'unit.o';packed=scratch/'unit.o.gz'
            if tool and subprocess.run([tool,'get',name,packed],capture_output=True).returncode==0:
                try:
                    with gzip.open(packed,'rb') as src,open(fresh,'wb') as dst:shutil.copyfileobj(src,dst)
                    os.replace(fresh,kept);return done('shared')
                except (OSError,EOFError):pass   # a damaged object: compile it here
            run([*command,'-c','-o',fresh],COMPILE_CPU,cwd=REPO)
            if tool:
                with open(fresh,'rb') as src,gzip.open(packed,'wb',compresslevel=1) as dst:shutil.copyfileobj(src,dst)
                subprocess.run([tool,'put',name,packed],capture_output=True)
            os.replace(fresh,kept);_prune(cache)
            return done('miss')

def objects(units,log=None):
    """Each unit's object, in order. A unit is (tool, [flags...], source); the flags carry
    the standard, the sanitizers (FLAGS), includes and defines. Units compile side by side."""
    log=log or (lambda line:print(line,file=sys.stderr,flush=True))
    results=[None]*len(units);errors=[None]*len(units)
    def work(i,tool,flags,source):
        flags=[_near(f) if os.path.isabs(str(f)) else str(f) for f in flags]
        try:results[i]=_object([str(tool),*flags,_near(source)],Path(source).name,log)
        except BaseException as error:errors[i]=error
    threads=[threading.Thread(target=work,args=(i,*unit)) for i,unit in enumerate(units)]
    for t in threads:t.start()
    for t in threads:t.join()
    for error in errors:
        if error:raise error
    return results

def link(tool,flags,objs,exe):
    """Links the objects into exe with the flags they were compiled with."""
    run([tool,*flags,*objs,'-o',exe],COMPILE_CPU)
    return exe
