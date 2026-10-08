"""Fixed-size water samples through an allocated Chrome profile and port.

Run under openagents browser run against the candidate WASM test build.
"""
import base64
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import statistics
import sys
import time
import urllib.request

from PIL import Image

from cdp import Cdp

origin, destination = sys.argv[1], Path(sys.argv[2])
destination.mkdir(parents=True, exist_ok=True)
port = os.environ['OPENAGENTS_CHROME_PORT']
inputs = json.load(urllib.request.urlopen(origin + '/water-w11-inputs.json'))
assert inputs['app_start_verified'] and inputs['wasm_bindgen'] == '0.2.128', inputs
for key in ['wasm', 'glue', 'pack', 'kit']:
    expected = inputs[key]
    with urllib.request.urlopen(origin + expected['url']) as response:
        assert response.status == 200, (key, response.status)
        payload = response.read()
    assert len(payload) == expected['bytes'], key
    assert hashlib.sha256(payload).hexdigest() == expected['sha256'], key
del payload
PROBE = r'''(() => {
  const probe = window.__waterFence = {api:null, samples:[], submitted:0, errors:[], deviceErrors:[], deviceLost:[]};
  if (typeof GPUAdapter !== 'undefined') {
    const request = GPUAdapter.prototype.requestDevice;
    GPUAdapter.prototype.requestDevice = function(...args) {
      return request.apply(this,args).then(device => {
        probe.deviceFeatures = [...device.features];
        device.addEventListener('uncapturederror', event => probe.deviceErrors.push({type:event.error.constructor.name,message:event.error.message}));
        device.lost.then(value => probe.deviceLost.push({reason:value.reason,message:value.message}));
        return device;
      });
    };
  }
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
        elif event['method'] == 'Log.entryAdded':
            entry = event['params']['entry']
            rows.append({'level':entry['level'], 'text':entry['text'], 'url':entry.get('url')})
        elif event['method'] == 'Runtime.exceptionThrown':
            rows.append({'level':'exception', 'text':str(event['params']['exceptionDetails'])})
    return rows


def unexpected_errors(rows):
    # Chrome requests the minimal local test page's absent favicon.
    return [row for row in rows if row['level'] in ['error', 'exception']
            and not (row.get('url') or '').endswith('/favicon.ico')]


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
    startup_logs = console(page)
    admissions = [json.loads(row['text'][len('Everglade water renderer '):])
                  for row in startup_logs if row['text'].startswith('Everglade water renderer ')]
    assert len(admissions) == 1 and admissions[0]['physical'], startup_logs
    admission = admissions[0]
    assert admission['tier'] == ('low' if mode == 'webgl2' else 'medium'), admission
    network = [{'url':event['params']['response']['url'],
                'status':event['params']['response']['status']}
               for event in page.events if event['method'] == 'Network.responseReceived']
    for key in ['wasm', 'pack'] + (['kit'] if zone == 'everglade' else []):
        assert any(row['url'] == origin + inputs[key]['url'] and row['status'] == 200
                   for row in network), (key, network)
    assert not any('proxies' in row['text'] for row in startup_logs), startup_logs
    assert not unexpected_errors(startup_logs), startup_logs
    warmed = time.monotonic()
    while time.monotonic() - warmed < 12:
        time.sleep(1)
        page.eval('window.__waterFence.submitted')
    page.events.clear()
    page.eval('window.__waterFence.samples=[]')
    measured_at = datetime.now(timezone.utc).isoformat()
    measured = time.monotonic()
    while time.monotonic() - measured < 12:
        time.sleep(1)
        page.eval('window.__waterFence.submitted')
    probe = page.eval("({api:window.__waterFence.api,samples:window.__waterFence.samples,errors:window.__waterFence.errors,deviceErrors:window.__waterFence.deviceErrors,deviceLost:window.__waterFence.deviceLost,deviceFeatures:window.__waterFence.deviceFeatures,size:[document.querySelector('canvas').width,document.querySelector('canvas').height],webgl2:!!document.querySelector('canvas').getContext('webgl2'),userAgent:navigator.userAgent})")['result']['value']
    elapsed = time.monotonic() - measured
    assert probe['size'] == [1920,1080], probe
    assert probe['webgl2'] == (mode == 'webgl2'), probe
    assert not probe['errors'] and not probe['deviceErrors'] and not probe['deviceLost'], probe
    logs = console(page)
    samples = []
    for row in logs:
        if row['text'].startswith('Everglade water ['):
            samples += json.loads(row['text'][len('Everglade water '):])
    errors = unexpected_errors(logs)
    assert not errors, errors
    if not dry:
        assert samples, logs
    tag = f'{zone}-{mode}' + ('-dry' if dry else '')
    screenshot = page.call('Page.captureScreenshot', {'format':'png'})
    image = destination / f'{tag}.png'
    image.write_bytes(base64.b64decode(screenshot['data']))
    assert any(low != high for low, high in Image.open(image).convert('RGB').getextrema()), 'Blank frame: ' + tag
    jobs = sum(s['completed_jobs'] for s in samples)
    synthesis_ms = sum(s['completed_synthesis_ms'] for s in samples)
    result = {'case':tag, 'mode':mode, 'tier':admission['tier'],
              'measured_at_utc':measured_at, 'measurement_elapsed_seconds':elapsed,
              'water_log_frames':len(samples), 'queue_fence_completions':len(probe['samples']),
              'measurement_window':'one-second renderer log batches observed during the dated interval',
              'high_tier':'not admitted on the browser platform', 'size':probe['size'],
              'user_agent':probe['userAgent'], 'device_features':probe.get('deviceFeatures'),
              'device_errors':probe['deviceErrors'], 'device_lost':probe['deviceLost'],
              'console':logs, 'visual_capture_checked':True, 'query':query, 'samples':samples,
              'inputs':inputs, 'renderer_admission':admission, 'startup_responses':network,
              'licensed_kit_loaded':zone == 'everglade',
              'gpu_timestamps_supported':any(s['gpu_timestamps'] for s in samples),
              'gpu_ms':summary([s['gpu']['water_ms'] for s in samples if s['gpu']]),
              'main_ms':summary([s['main_ms'] for s in samples]),
              'worker_ms':summary([s['worker_ms'] for s in samples]),
              'main_cpu_ms':None, 'worker_cpu_supported':False,
              'completed_jobs':jobs, 'completed_synthesis_ms':synthesis_ms,
              'mean_completed_job_elapsed_ms':synthesis_ms/jobs if jobs else None,
              'mean_completed_job_cpu_ms':None,
              'observed_last_job_elapsed_ms':summary([s['synthesis_ms'] for s in samples if s['completed_jobs']]),
              'worker_bytes':max((s['worker_bytes'] for s in samples), default=0),
              'ripple_cpu_bytes':max((s['ripple_cpu_bytes'] for s in samples), default=0),
              'gpu_bytes':max((s['gpu_bytes'] for s in samples), default=0),
              'queue_fence_frame_ms_estimate':summary([s['ms'] for s in probe['samples']]),
              'queue_fence_method':'WebGPU submit-to-onSubmittedWorkDone wall time; WebGL2 RAF work plus gl.finish wall time; neither is a GPU timestamp',
              'inline_synthesis':True, 'capture':image.name,
              'capture_sha256':hashlib.sha256(image.read_bytes()).hexdigest()}
    page.call('Page.close')
    page.s.close()
    return result


# Preserve a failed case and close its GPU tab before the next case.
def failed(mode, zone, dry, error):
    tag = f'{zone}-{mode}' + ('-dry' if dry else '')
    row = {'case':tag, 'mode':mode, 'success':False,
           'failed_at_utc':datetime.now(timezone.utc).isoformat(),
           'failure_type':type(error).__name__, 'failure':str(error)[:32000]}
    trace = error.__traceback__
    page = None
    while trace:
        if trace.tb_frame.f_code.co_name == 'run':
            values = trace.tb_frame.f_locals
            page = values.get('page')
            for key in ['admission','probe','network','samples','elapsed','measured_at','query']:
                if key in values: row[key] = values[key]
        trace = trace.tb_next
    if page:
        try:
            row['probe_at_failure'] = page.eval('({probe:window.__waterFence,status:document.querySelector("#everglade-status")?.textContent,size:[document.querySelector("canvas")?.width,document.querySelector("canvas")?.height]})')['result'].get('value')
            screenshot = page.call('Page.captureScreenshot', {'format':'png'})
            image = destination / (tag+'-failed.png')
            image.write_bytes(base64.b64decode(screenshot['data']))
            row['capture'] = image.name
            row['capture_sha256'] = hashlib.sha256(image.read_bytes()).hexdigest()
        except Exception as capture_error:
            row['evidence_error'] = str(capture_error)[:2000]
        row['console'] = console(page)
        row['responses'] = [{'url':event['params']['response']['url'], 'status':event['params']['response']['status']} for event in page.events if event['method']=='Network.responseReceived']
        try: page.call('Page.close')
        except Exception: pass
        finally: page.s.close()
    return row

results = []
selected = os.environ.get('WATER_W11_BROWSER_CASES')
selected = set(selected.split(',')) if selected else None
known = {f'{zone}-{mode}' for zone in ['water', 'everglade'] for mode in ['webgpu', 'webgl2']}
assert selected is None or selected and selected <= known, selected
for zone in ['water', 'everglade']:
    for mode in ['webgpu', 'webgl2']:
        if selected is not None and f'{zone}-{mode}' not in selected:
            continue
        pair = []
        for dry in [True, False]:
            try:
                row = run(mode, zone, dry)
                row['success'] = True
            except Exception as error:
                row = failed(mode, zone, dry, error)
            results.append(row); pair.append(row)
            (destination / 'browser.json').write_text(json.dumps(results,indent=2)+'\n')
            print(json.dumps({k:(str(row[k])[:800] if k == 'failure' else row[k]) for k in ['case','success','failure','gpu_ms','main_ms','gpu_bytes'] if k in row}),flush=True)
        dry, wet = pair
        if dry['success'] and wet['success']:
            wet['queue_fence_water_ms_estimate'] = max(0, wet['queue_fence_frame_ms_estimate']['mean']-dry['queue_fence_frame_ms_estimate']['mean'])
        (destination / 'browser.json').write_text(json.dumps(results,indent=2)+'\n')
Path(destination/'harness.json').write_text(json.dumps({'executed_harness_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'failure_retention':True,'log_and_device_errors_checked':True,'blank_frames_rejected':True},indent=2)+'\n')
sys.exit(any(not row['success'] for row in results))
