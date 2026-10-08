"""Fixed-size water samples through an allocated Chrome profile and port.

Run under openagents browser run against the candidate WASM test build.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import statistics
import sys
import time
import urllib.request

from cdp import Cdp

origin, destination = sys.argv[1], Path(sys.argv[2])
destination.mkdir(parents=True, exist_ok=True)
port = os.environ['OPENAGENTS_CHROME_PORT']
PROBE = r'''(() => {
  const probe = window.__waterFence = {api:null, samples:[], submitted:0, errors:[]};
  const context = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function(...args) {
    const result = context.apply(this,args);
    if (result && args[0] === 'webgl2') probe.gl = result;
    return result;
  };
  if (typeof GPUQueue !== 'undefined') {
    const submit = GPUQueue.prototype.submit;
    GPUQueue.prototype.submit = function(...args) {
      const start = performance.now(), id = probe.submitted++;
      const value = submit.apply(this,args);
      probe.api = 'webgpu';
      this.onSubmittedWorkDone().then(() => {
        probe.samples.push({id, ms:performance.now()-start});
      }).catch(error => probe.errors.push(String(error)));
      return value;
    };
  }
  const raf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = callback => raf(time => {
    if (!probe.gl) { callback(time); return; }
    const start = performance.now();
    callback(time);
    probe.gl.finish();
    probe.api = 'webgl2';
    probe.samples.push({id:probe.submitted++,ms:performance.now()-start});
  });
})();'''


def summary(values):
    values = sorted(value for value in values if isinstance(value, (int, float)))
    if not values:
        return {'count': 0, 'mean': None, 'p95': None, 'max': None}
    return {'count': len(values), 'mean': statistics.mean(values),
            'p95': values[(len(values)-1)*95//100], 'max': values[-1]}


def console(page):
    rows = []
    for event in page.events:
        if event['method'] == 'Runtime.consoleAPICalled':
            args = event['params']['args']
            rows.append({'level':event['params']['type'],
                         'text':' '.join(str(a.get('value', a.get('description', ''))) for a in args)})
        elif event['method'] == 'Runtime.exceptionThrown':
            rows.append({'level':'exception', 'text':str(event['params']['exceptionDetails'])})
    return rows


def run(mode, zone, dry):
    request = urllib.request.Request(f'http://127.0.0.1:{port}/json/new?about:blank', method='PUT')
    target = json.load(urllib.request.urlopen(request))
    page = Cdp(target['webSocketDebuggerUrl'])
    for domain in ['Runtime', 'Page', 'Network', 'Log']:
        page.call(f'{domain}.enable')
    page.call('Emulation.setDeviceMetricsOverride', {'width':1920, 'height':1080,
              'deviceScaleFactor':1, 'mobile':False})
    page.call('Page.addScriptToEvaluateOnNewDocument', {'source':PROBE})
    query = '?frames&town-clock=off' + ('&gl' if mode == 'webgl2' else '')
    if zone == 'water':
        query += '&zone=water'
    else:
        # Lantern Pond's southern bank, facing the center. The default
        # town spawn does not show a representative water footprint.
        query += '&at=-2.43,21.13,0.1792'
    if dry:
        query += '&water-dry'
    page.call('Page.navigate', {'url':origin + '/index.html' + query})
    started = time.monotonic()
    ready = False
    while time.monotonic() - started < 180:
        time.sleep(1)
        value = page.eval("({ready:document.querySelector('#everglade-status') && getComputedStyle(document.querySelector('#everglade-status')).display==='none',status:document.querySelector('#everglade-status')?.textContent})")['result'].get('value', {})
        if value.get('ready'):
            ready = True
            break
        if 'could not start' in value.get('status', '').lower():
            break
    assert ready, value
    warmed = time.monotonic()
    while time.monotonic() - warmed < 12:
        time.sleep(1)
        page.eval('window.__waterFence.submitted')
    page.events.clear()
    page.eval('window.__waterFence.samples=[]')
    measured = time.monotonic()
    while time.monotonic() - measured < 12:
        time.sleep(1)
        page.eval('window.__waterFence.submitted')
    probe = page.eval("({api:window.__waterFence.api,samples:window.__waterFence.samples,errors:window.__waterFence.errors,size:[document.querySelector('canvas').width,document.querySelector('canvas').height],webgl2:!!document.querySelector('canvas').getContext('webgl2'),userAgent:navigator.userAgent})")['result']['value']
    assert probe['size'] == [1920,1080], probe
    assert probe['webgl2'] == (mode == 'webgl2'), probe
    assert not probe['errors'], probe
    logs = console(page)
    samples = []
    for row in logs:
        if row['text'].startswith('Everglade water '):
            samples += json.loads(row['text'][len('Everglade water '):])
    errors = [row for row in logs if row['level'] in ['error', 'exception']]
    assert not errors, errors
    if not dry:
        assert samples, logs
    tag = f'{zone}-{mode}' + ('-dry' if dry else '')
    screenshot = page.call('Page.captureScreenshot', {'format':'png'})
    image = destination / f'{tag}.png'
    image.write_bytes(base64.b64decode(screenshot['data']))
    result = {'case':tag, 'mode':mode, 'tier':'low' if mode == 'webgl2' else 'medium',
              'high_tier':'not admitted on the browser platform', 'size':probe['size'],
              'user_agent':probe['userAgent'], 'query':query, 'samples':samples,
              'gpu_timestamps_supported':any(s['gpu_timestamps'] for s in samples),
              'gpu_ms':summary([s['gpu']['water_ms'] for s in samples if s['gpu']]),
              'main_ms':summary([s['main_ms'] for s in samples]),
              'worker_ms':summary([s['worker_ms'] for s in samples]),
              'main_cpu_ms':None, 'worker_cpu_supported':False,
              'completed_jobs':sum(s['completed_jobs'] for s in samples),
              'completed_synthesis_ms':sum(s['completed_synthesis_ms'] for s in samples),
              'per_job_elapsed_ms':summary([s['synthesis_ms'] for s in samples if s['completed_jobs']]),
              'worker_bytes':max((s['worker_bytes'] for s in samples), default=0),
              'ripple_cpu_bytes':max((s['ripple_cpu_bytes'] for s in samples), default=0),
              'gpu_bytes':max((s['gpu_bytes'] for s in samples), default=0),
              'queue_fence_frame_ms_estimate':summary([s['ms'] for s in probe['samples']]),
              'queue_fence_method':'WebGPU submit-to-onSubmittedWorkDone wall time; WebGL2 RAF work plus gl.finish wall time; neither is a GPU timestamp',
              'inline_synthesis':True, 'capture':image.name,
              'capture_sha256':hashlib.sha256(image.read_bytes()).hexdigest()}
    page.call('Page.close')
    return result


results = []
for zone in ['water', 'everglade']:
    for mode in ['webgpu', 'webgl2']:
        dry = run(mode, zone, True)
        wet = run(mode, zone, False)
        wet['queue_fence_water_ms_estimate'] = max(0, wet['queue_fence_frame_ms_estimate']['mean']
                                                 - dry['queue_fence_frame_ms_estimate']['mean'])
        results += [dry, wet]
        (destination / 'browser.json').write_text(json.dumps(results, indent=2) + '\n')
        print(json.dumps({key:wet[key] for key in ['case','gpu_ms','main_ms','worker_ms','gpu_bytes',
              'queue_fence_water_ms_estimate']}), flush=True)
