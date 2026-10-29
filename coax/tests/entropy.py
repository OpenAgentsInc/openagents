#!/usr/bin/env python3
"""Scoped native/Apple simulator/browser CSPRNG contract tests. No keys/logged bytes."""
import functools
import http.server
import json
import os
import platform
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import threading
import unittest

ROOT=Path(__file__).resolve().parents[2]
COAX=ROOT/'coax/bin/coax'
CC=os.environ.get('COAX_CC','clang')
CHROME=Path('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome')
if not CHROME.exists():CHROME=Path(shutil.which('google-chrome') or shutil.which('chromium') or '/nonexistent/chrome')


def generated(directory):
    result=subprocess.check_output([str(COAX),'c',str(ROOT/'coax/lib/crypto/entropy.coax'),'--library','--header',str(directory/'entropy-policy.h')])
    (directory/'entropy-policy.c').write_bytes(result)


def compile_native(directory,name,extra=(),target=()):
    output=directory/name
    framework=['-framework','Security'] if platform.system()=='Darwin' else []
    cmd=[CC,*target,'-std=c11','-O2','-g','-I'+str(directory),'-I'+str(ROOT/'coax/platform'),*extra,str(directory/'entropy-policy.c'),str(ROOT/'coax/platform/entropy-native.c'),str(ROOT/'coax/tests/entropy_native.c'),*framework,'-lm','-o',str(output)]
    subprocess.run(cmd,check=True)
    return output


class Entropy(unittest.TestCase):
    def test_native_real_os_and_test_only_failure_adapter(self):
        with tempfile.TemporaryDirectory(prefix='coax-entropy-native-') as tmp:
            d=Path(tmp);generated(d)
            for mode,flags in [('real',[]),('fixture',['-DCOAX_ENTROPY_TEST_PROVIDER'])]:
                exe=compile_native(d,mode,(*flags,'-fsanitize=address,undefined','-fno-sanitize-recover=all'))
                run=subprocess.run([str(exe)],capture_output=True,text=True,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'},timeout=30)
                self.assertEqual(run.returncode,0,run.stderr);print(run.stdout.strip())
            obj=d/'production.o'
            subprocess.run([CC,'-std=c11','-O2','-I'+str(d),'-I'+str(ROOT/'coax/platform'),'-c',str(ROOT/'coax/platform/entropy-native.c'),'-o',str(obj)],check=True)
            symbols=subprocess.check_output(['nm','-u',str(obj)],text=True)
            self.assertIn('SecRandomCopyBytes' if platform.system()=='Darwin' else 'getrandom',symbols);self.assertNotIn('coax_entropy_test_provider',symbols)

    def test_actual_ios_simulator_os_and_failure_boundaries(self):
        if platform.system()!='Darwin' or not shutil.which('xcrun'):self.skipTest('Apple simulator tools unavailable; no iOS runtime pass')
        devices=json.loads(subprocess.check_output(['xcrun','simctl','list','-j','devices','booted']))['devices']
        candidates=[d for group in devices.values() for d in group if d.get('state')=='Booted' and d.get('isAvailable')]
        if not candidates:self.skipTest('iOS simulator unavailable; this is not an iOS runtime pass')
        device=candidates[0]['udid']
        sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
        target=('-target','arm64-apple-ios26.0-simulator','-isysroot',sdk)
        with tempfile.TemporaryDirectory(prefix='coax-entropy-ios-') as tmp:
            d=Path(tmp);generated(d)
            for mode,flags in [('real',[]),('fixture',['-DCOAX_ENTROPY_TEST_PROVIDER'])]:
                exe=compile_native(d,mode,flags,target)
                run=subprocess.run(['xcrun','simctl','spawn',device,str(exe)],capture_output=True,text=True,timeout=30)
                self.assertEqual(run.returncode,0,run.stderr);print('iOS simulator: '+run.stdout.strip())
            ios_sdk=subprocess.check_output(['xcrun','--sdk','iphoneos','--show-sdk-path'],text=True).strip()
            # Physical arm64 API/ABI compilation, not a claim of a device runtime run.
            for source in [d/'entropy-policy.c',ROOT/'coax/platform/entropy-native.c']:
                subprocess.run([CC,'-target','arm64-apple-ios26.0','-isysroot',ios_sdk,'-std=c11','-O2','-I'+str(d),'-I'+str(ROOT/'coax/platform'),'-c',str(source),'-o',str(d/(source.stem+'.ios.o'))],check=True)

    def test_actual_chrome_webcrypto_and_wasm_boundary(self):
        if not CHROME.exists():self.skipTest('Chrome unavailable; no browser runtime pass')
        with tempfile.TemporaryDirectory(prefix='coax-entropy-browser-') as tmp:
            d=Path(tmp)
            subprocess.run([str(ROOT/'coax/web/wasm.sh'),str(ROOT/'coax/lib/crypto/entropy.coax'),str(ROOT/'coax/platform/entropy-wasm.c'),str(d/'entropy.wasm')],check=True)
            shutil.copyfile(ROOT/'coax/web/entropy.js',d/'entropy.js');shutil.copyfile(ROOT/'coax/tests/entropy_browser.html',d/'index.html')
            class Quiet(http.server.SimpleHTTPRequestHandler):
                def log_message(self,*args):pass
            server=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(Quiet,directory=str(d)))
            thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
            try:
                url=f'http://127.0.0.1:{server.server_port}/index.html'
                run=subprocess.run([str(CHROME),'--headless=new','--mute-audio','--disable-gpu','--disable-background-networking','--disable-component-update','--disable-default-apps','--disable-sync','--no-first-run','--no-default-browser-check','--user-data-dir='+str(d/'chrome'),'--virtual-time-budget=10000','--dump-dom',url],capture_output=True,text=True,timeout=40)
                self.assertEqual(run.returncode,0,run.stderr)
                self.assertIn('PASS 21 browser/Wasm CSPRNG boundary cases',run.stdout,run.stdout)
                print('Chrome: 21 actual browser/Wasm CSPRNG boundary cases passed')
            finally:server.shutdown();server.server_close();thread.join(timeout=5)


if __name__=='__main__':unittest.main()
