#!/usr/bin/env python3
"""Real wasm env import and bundled browser host. No keys or logged bytes."""
import functools
import http.server
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import unittest
from entropy import ROOT, CHROME


class RandomImport(unittest.TestCase):
    def test_actual_import_and_bundled_host(self):
        if not CHROME.exists():self.skipTest('Chrome unavailable; no browser runtime pass')
        with tempfile.TemporaryDirectory(prefix='coax-entropy-import-') as tmp:
            d=Path(tmp)
            source=(ROOT/'coax/tests/entropy_import_face.c').read_text().replace('../platform/entropy-wasm.c',str(ROOT/'coax/platform/entropy-wasm.c'))
            for name,defines in [('consumer',''),('missing','#define OMIT_ENTROPY_FACE\n'),('unknown','#define OMIT_ENTROPY_FACE\n#define UNKNOWN_RANDOM_IMPORT\n'),('prototype','#define OMIT_ENTROPY_FACE\n#define UNKNOWN_PROTO_IMPORT\n'),('large','#define PROBE_STORAGE 8388616\n'),('stdio','#define STDIO_PROBE\n')]:
                face=d/(name+'.c');face.write_text(defines+source)
                subprocess.run([str(ROOT/'coax/web/wasm.sh'),str(ROOT/'coax/tests/entropy_import.coax'),str(face),str(d/(name+'.wasm'))],check=True)
            for src in ['coax/web/entropy.js','coax/web/entropy-import.js','coax/tests/entropy_import.html','coax/tests/entropy_import_host.html']:
                shutil.copyfile(ROOT/src,d/Path(src).name)
            subprocess.run(['bun','build',str(ROOT/'coax/web/host.js'),'--target','browser','--format','esm','--outfile',str(d/'host.bundle.js')],check=True,capture_output=True)
            subprocess.run(['bun',str(ROOT/'coax/web/strip.ts'),str(d)],check=True,capture_output=True)
            class Quiet(http.server.SimpleHTTPRequestHandler):
                def log_message(self,*args):pass
                def handle(self):
                    try:super().handle()
                    except (BrokenPipeError,ConnectionResetError):pass # fixture iframe closed its dev socket
            server=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(Quiet,directory=str(d)))
            thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
            try:
                url=f'http://127.0.0.1:{server.server_port}/entropy_import.html'
                browser=subprocess.Popen([str(CHROME),'--headless=new','--disable-gpu','--disable-background-networking','--disable-component-update','--disable-default-apps','--disable-sync','--no-first-run','--no-default-browser-check','--remote-debugging-port=0','--user-data-dir='+str(d/'chrome'),'about:blank'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
                try:
                    run=subprocess.run(['bun',str(ROOT/'coax/tests/browser_status.js'),str(d/'chrome'),url],capture_output=True,text=True,timeout=30)
                    self.assertEqual(run.returncode,0,run.stdout+'\n'+run.stderr)
                    self.assertEqual(run.stdout.strip(),'PASS 29 random-import browser/Wasm cases')
                finally:browser.terminate();browser.wait(timeout=10)
                print('Chrome: 29 actual random-import/Wasm and bundled-host cases passed')
            finally:server.shutdown();server.server_close();thread.join(timeout=5)


if __name__=='__main__':unittest.main()
