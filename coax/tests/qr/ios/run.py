#!/usr/bin/env python3
"""Actual iOS Vision frame decode, on an owned temporary headless Simulator."""
import ctypes as C
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import time
from PIL import Image

HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3]
sys.path.insert(0,str(HERE.parent))
from test_qr import Result,View,view

def run(*args,timeout=90):
    r=subprocess.run([str(x)for x in args],capture_output=True,text=True,timeout=timeout)
    if r.returncode:raise RuntimeError(r.stderr or r.stdout)
    return r.stdout.strip()
def main():
    directory=ROOT/'coax/bin/qr-2026-10-28';app=directory/'CoaxQRProbe.app';app.mkdir(parents=True,exist_ok=True)
    rows=json.loads((HERE.parent/'reference-2026-10-28.json').read_text())['rows']
    lib=C.CDLL(str(directory/'checked.dylib'));lib.qr_encode.argtypes=[View,View];lib.qr_encode.restype=Result
    fixtures=[]
    for index,row in enumerate(rows):
        if not row['bytes']:continue # Empty byte QR is a grid test, not a textual payload assertion.
        data=(C.c_uint8*len(row['bytes']))(*row['bytes']);out=(C.c_uint8*5929)();result=lib.qr_encode(view(data),view(out))
        if result.status:raise RuntimeError('Coax QR refused iOS fixture')
        n=result.size;image=Image.new('L',(n+8,n+8),255);pixels=image.load()
        for y in range(n):
            for x in range(n):pixels[x+4,y+4]=0 if out[y*n+x] else 255
        name=f'qr-{index}.png';image.resize(((n+8)*8,(n+8)*8),Image.Resampling.NEAREST).save(app/name)
        fixtures.append({'file':name,'text':row['text']})
    (app/'fixtures.json').write_text(json.dumps(fixtures)+'\n')
    identifier='com.openagents.coax.qr-probe'
    (app/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':identifier,'CFBundleName':'Coax QR Probe','CFBundleExecutable':'QRProbe','CFBundlePackageType':'APPL','CFBundleVersion':'1','CFBundleShortVersionString':'1.0','MinimumOSVersion':'26.0','UIDeviceFamily':[1],'UILaunchScreen':{}}))
    sdk=run('xcrun','--sdk','iphonesimulator','--show-sdk-path')
    run('xcrun','--sdk','iphonesimulator','swiftc','-target','arm64-apple-ios26.0-simulator','-sdk',sdk,'-O',ROOT/'coax/lib/qr/platform/QRFrameDecoder.swift',HERE/'Probe.swift','-framework','UIKit','-framework','Vision','-framework','CoreVideo','-o',app/'QRProbe')
    run('codesign','--force','--sign','-',app)
    runtime='com.apple.CoreSimulator.SimRuntime.iOS-26-5'
    device=run('xcrun','simctl','create','Coax QR temporary probe','com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro',runtime)
    try:
        run('xcrun','simctl','boot',device);run('xcrun','simctl','bootstatus',device,'-b',timeout=90)
        run('xcrun','simctl','install',device,app);run('xcrun','simctl','launch',device,identifier)
        container=Path(run('xcrun','simctl','get_app_container',device,identifier,'data'));result=container/'Documents/qr-scanner.json'
        for _ in range(300):
            if result.exists():break
            time.sleep(.1)
        report=json.loads(result.read_text());report['decoder_source_sha256']=hashlib.sha256((ROOT/'coax/lib/qr/platform/QRFrameDecoder.swift').read_bytes()).hexdigest();report['binary_sha256']=hashlib.sha256((app/'QRProbe').read_bytes()).hexdigest();report['runtime']=runtime
        output=directory/'ios-scanner.json';output.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
        assert not report.get('failure'),report
        assert len(report['rows'])==19 and all(row['matches']for row in report['rows']),report
        assert any(row['inputBytes']==70 for row in report['rows']) and any(row['inputBytes']==300 for row in report['rows'])
        print('Actual iOS Vision scanner:19 rendered Coax QR frames decoded, including70/300-byte URLs; physical optics not observed')
    finally:
        subprocess.run(['xcrun','simctl','shutdown',device],capture_output=True)
        run('xcrun','simctl','delete',device)
if __name__=='__main__':main()
