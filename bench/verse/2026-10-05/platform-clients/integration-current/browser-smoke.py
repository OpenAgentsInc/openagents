import pathlib,os,subprocess,time,json,urllib.request,http.server,threading,functools,sys,base64,hashlib
from cdp import Cdp
base=pathlib.Path(__file__).parent;site=base/"site";profile=base/("profile-"+str(time.time_ns()));home=base/"home"
for x in [profile,home]:x.mkdir(exist_ok=True)
html=pathlib.Path("/home/christopherdavid/work/openagents-verse-audit/crates/everglade-web/index.html").read_text();html=html.replace('<script type="module">','<script>localStorage.setItem("openagents.grid.secret", "'+"29"*32+'");</script><script type="module">');(site/"index.html").write_text(html)
(site/"start").touch()
http=http.server.ThreadingHTTPServer(("127.0.0.1",0),functools.partial(http.server.SimpleHTTPRequestHandler,directory=str(site)))
threading.Thread(target=http.serve_forever,daemon=True).start();url=f"http://127.0.0.1:{http.server_port}/?zone=chamber&gl"
browser='/nix/store/km74bimklrd4zmvxz4963srphshynzia-ungoogled-chromium-152.0.7977.82/bin/chromium'
env=os.environ.copy();env.update(HOME=str(home),XDG_CONFIG_HOME=str(home/"config"),XDG_CACHE_HOME=str(home/"cache"));env.pop("DISPLAY",None);env.pop("WAYLAND_DISPLAY",None)
args=[browser,"--headless=new","--user-data-dir="+str(profile),"--remote-debugging-port=0","--remote-allow-origins=http://127.0.0.1","--disable-background-networking","--disable-sync","--no-first-run","--use-gl=angle","--use-angle=swiftshader","--enable-unsafe-swiftshader","--window-size=1000,800","about:blank"]
log=(base/"browser.log").open('w');proc=subprocess.Popen(args,env=env,stdout=log,stderr=subprocess.STDOUT);results=[];cdp=None
(base/"receipt.json").unlink(missing_ok=True)
try:
 for i in range(200):
  if (profile/"DevToolsActivePort").exists():break
  if proc.poll() is not None:raise RuntimeError("Browser exited; inspect browser.log")
  time.sleep(.1)
 port=int((profile/"DevToolsActivePort").read_text().splitlines()[0]);tabs=json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json"));tab=next(x for x in tabs if x['type']=='page');cdp=Cdp(tab['webSocketDebuggerUrl']);cdp.call('Runtime.enable');cdp.call('Page.enable');cdp.call('Page.bringToFront');cdp.call('Emulation.setDeviceMetricsOverride',{'width':1000,'height':800,'deviceScaleFactor':1,'mobile':False});cdp.call('Page.navigate',{'url':url})
 def val(expr):
  r=cdp.eval(expr)
  if 'exceptionDetails' in r:raise RuntimeError(r)
  return r.get('result',{}).get('value')
 def wait(label,expression,seconds=45):
  end=time.time()+seconds
  while time.time()<end:
   value=val(expression)
   if value:results.append({'name':label,'value':value});return value
   time.sleep(.2)
  raise RuntimeError(label+' timed out: '+str(val('document.body.innerText')))
 wait('authenticated joined HUD',"document.querySelector('[role=status]')?.textContent?.startsWith('Health')")
 val("window.probeFrames=[];requestAnimationFrame(function probe(t){probeFrames.push(t);requestAnimationFrame(probe)})")
 time.sleep(3)
 results.append({'name':'adaptive graphics at original viewport','value':val("(()=>{const c=document.querySelector('canvas');return {viewport:[innerWidth,innerHeight],physical:[c.width,c.height],scale:Number(c.dataset.renderScale)}})()")})
 def authority():
  for attempt in range(20):
   try:return json.loads((site/'pose-state.json').read_text())
   except (FileNotFoundError,json.JSONDecodeError):time.sleep(.05)
  raise RuntimeError('Scratch authority observation unavailable')
 before=authority();cdp.call('Input.dispatchKeyEvent',{'type':'keyDown','key':'q','code':'KeyQ','windowsVirtualKeyCode':81});time.sleep(.8);cdp.call('Input.dispatchKeyEvent',{'type':'keyUp','key':'q','code':'KeyQ','windowsVirtualKeyCode':81});time.sleep(.3);after=authority()
 distance=sum((a-b)**2 for a,b in zip(before['position'],after['position']))**.5
 assert after['epoch']==before['epoch'] and after['sequence']>before['sequence'] and distance>.1,{'before':before,'after':after,'distance':distance,'raf':val('probeFrames')}
 time.sleep(.25);stopped=authority();drift=sum((a-b)**2 for a,b in zip(after['position'],stopped['position']))**.5
 assert drift<.1 and stopped['epoch']==after['epoch'],{'after':after,'stopped':stopped,'drift':drift}
 results.append({'name':'authoritative release stops movement','value':{'after':after,'stopped':stopped,'drift':drift}})
 results.append({'name':'software rendering cadence','value':val('probeFrames')})
 results.append({'name':'authoritative keyboard movement','value':{'before':before,'after':after,'distance':distance}})

 cdp.call('Emulation.setDeviceMetricsOverride',{'width':1000,'height':800,'deviceScaleFactor':1,'mobile':False});time.sleep(.5)
 layout=val("(()=>{const p=document.getElementById('chamber-controls'),r=p.getBoundingClientRect();return {viewport:[innerWidth,innerHeight],panel:[r.x,r.y,r.width,r.height],buttons:[...p.querySelectorAll('button')].map(b=>({label:b.getAttribute('aria-label'),width:b.getBoundingClientRect().width,height:b.getBoundingClientRect().height})),identity:p.querySelector('p').textContent}})()")
 assert layout['panel'][1]>=0 and sum([layout['panel'][1],layout['panel'][3]])<=layout['viewport'][1]+1
 assert len(layout['buttons'])>=10 and all(b['label'] and b['height']>=44 and b['width']>=44 for b in layout['buttons']);results.append({'name':'visible controls and minimum targets','value':layout})
 image=cdp.call('Page.captureScreenshot',{'format':'png','captureBeyondViewport':False});(base/'joined-frame.png').write_bytes(base64.b64decode(image['data']))
 cdp.call('Emulation.setDeviceMetricsOverride',{'width':360,'height':640,'deviceScaleFactor':2,'mobile':False});time.sleep(1)
 viewport=val("(()=>{const c=document.querySelector('canvas'),p=document.getElementById('chamber-controls'),r=p.getBoundingClientRect();return {css:[c.clientWidth,c.clientHeight],physical:[c.width,c.height],panel:[r.x,r.y,r.width,r.height],viewport:[innerWidth,innerHeight]}})()")
 assert viewport['css']==[360,640] and all(0<v<=limit for v,limit in zip(viewport['physical'],[720,1280]))
 assert abs(viewport['physical'][0]/360-viewport['physical'][1]/640)<.02
 assert viewport['panel'][0]>=0 and viewport['panel'][1]>=0 and viewport['panel'][0]+viewport['panel'][2]<=360+1 and viewport['panel'][1]+viewport['panel'][3]<=640+1
 results.append({'name':'small viewport and physical scaling','value':viewport})
 image=cdp.call('Page.captureScreenshot',{'format':'png','captureBeyondViewport':False});(base/'narrow-frame.png').write_bytes(base64.b64decode(image['data']))
 val("window.dispatchEvent(new Event('blur'))")
 wait('blur releases session',"document.querySelector('[role=status]')?.textContent?.includes('paused')",5)
 val("window.dispatchEvent(new Event('focus'))")
 wait('focus rejoins same world identity',"document.querySelector('[role=status]')?.textContent?.startsWith('Health')")
 shortcuts=val("(()=>{const fire=(code,ctrlKey=false)=>{const e=new KeyboardEvent('keydown',{code,ctrlKey,bubbles:true,cancelable:true});window.dispatchEvent(e);return e.defaultPrevented};return {shortcut:fire('KeyR',true),unbound:fire('KeyP'),tab:fire('Tab'),space:fire('Space')}})()")
 assert shortcuts=={'shortcut':False,'unbound':False,'tab':False,'space':True};results.append({'name':'browser shortcuts and focus keys preserved','value':shortcuts})
 val("window.dispatchEvent(new KeyboardEvent('keyup',{code:'Space',bubbles:true}))")
 cdp.call('Input.dispatchKeyEvent',{'type':'keyDown','key':'w','code':'KeyW','windowsVirtualKeyCode':87});time.sleep(.25);cdp.call('Input.dispatchKeyEvent',{'type':'keyUp','key':'w','code':'KeyW','windowsVirtualKeyCode':87})
 val("[...document.querySelectorAll('button')].find(b=>b.textContent==='Target nearest').click()")
 time.sleep(.5);assert val("document.querySelector('[role=status]').textContent.startsWith('Health')")
 results.append({'name':'keyboard and targeting do not panic','value':True})
 config=json.loads((site/'chamber.json').read_text());bad=dict(config);bad['grant']='02'*32;(site/'chamber.json').write_text(json.dumps(bad));val("[...document.querySelectorAll('button')].find(b=>b.textContent==='Reconnect').click()")
 wait('ungranted reconnect refused',"document.querySelector('[role=status]')?.textContent?.includes('refused')")
 (site/'chamber.json').write_text(json.dumps(config));val("[...document.querySelectorAll('button')].find(b=>b.textContent==='Reconnect').click()")
 wait('restored grant rejoins',"document.querySelector('[role=status]')?.textContent?.startsWith('Health')")
 errors=[e for e in cdp.events if e.get('method')=='Runtime.exceptionThrown'];assert not errors,errors
 (base/'receipt.json').write_text(json.dumps({'schema':'verse.platform-browser-smoke.v1','pass':True,'browser':subprocess.check_output([browser,'--version'],env=env,text=True).strip(),'software_gpu':True,'temporary_identity':True,'runtime_artifacts':{name:{'bytes':(site/name).stat().st_size,'sha256':hashlib.sha256((site/name).read_bytes()).hexdigest()} for name in ['everglade_web.js','everglade_web_bg.wasm']},'results':results,'exceptions':errors,'limitations':['Headless software WebGL2 on Linux is not supported-hardware, physical-phone, gamepad, screen-reader, thermal, crowded-world, or output-audio acceptance.']},indent=2)+'\n')
 print('Browser smoke passed',flush=True)
except Exception as e:
 (base/'failed.json').write_text(json.dumps({'error':str(e),'results':results,'events':cdp.events if cdp else []},indent=2)+'\n');raise
finally:
 if cdp:
  try:cdp.call('Page.navigate',{'url':'about:blank'})
  except Exception:pass
 proc.terminate()
 try:proc.wait(timeout=10)
 except subprocess.TimeoutExpired:proc.kill();proc.wait()
 http.shutdown();(site/'stop').touch();log.close()
