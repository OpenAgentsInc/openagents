import {test,expect} from 'bun:test'
import {homeRuntime} from './home_runtime'
const generated=Bun.spawnSync(['python3','coax/web/tests/newland_fixtures.py']);if(generated.exitCode)throw Error(generated.stderr.toString())
const f=JSON.parse(generated.stdout.toString())
test('homepage admits signed Pylons, refuses forged observations and never replaces a project with incomplete history',async()=>{
 let asks=0,beaconAsks=0,watch: any
 const server=Bun.serve({port:0,fetch(req,s){if(s.upgrade(req))return;return new Response(null,{status:400})},websocket:{message(ws,data){
  const m=JSON.parse(String(data));if(m[0]!=='REQ')return;asks++
  if(m[1]==='home-beacons'){beaconAsks++;watch=ws;for(const event of [f.invalid_beacons.digest,{...f.beacon,sig:'0'.repeat(128)},f.beacon,f.beacon])ws.send(JSON.stringify(['EVENT',m[1],event]))}
  if(m[1]==='home-work')for(const event of [f.older_answer,f.answer,f.answer,f.status,{...f.answer,content:'forged'}])ws.send(JSON.stringify(['EVENT',m[1],event]))
  ws.send(JSON.stringify(['EOSE',m[1],['finish']]))
 }}})
 let now=f.now
 const h=await homeRuntime('apps/openagents-web/build/newland.wasm',{relay:`ws://127.0.0.1:${server.port}`,clock:()=>now})
 try{
  const original=h.x.coaxial_land_project_tasks(h.p,0)
  await h.until(()=>h.stats().beacons===1&&h.stats().refused>0)
  expect(h.stats().pylons).toBe(1);expect(h.stats().ready).toBe(0)
  expect(h.x.coaxial_land_project_tasks(h.p,0)).toBe(original)
  expect(h.x.coaxial_land_pylon_answers(h.p,0)).toBe(1);expect(h.x.coaxial_land_pylon_updates(h.p,0)).toBe(1)
  expect(h.x.coaxial_land_pylon_status(h.p,0)).toBe(0)
  now+=1;watch.send(JSON.stringify(['EVENT','home-beacons',f.updated_beacon]))
  await h.until(()=>h.stats().beacons===2)
  expect(h.x.coaxial_land_pylon_free(h.p,0)).toBe(0)
  watch.send(JSON.stringify(['EVENT','home-beacons',f.beacon]));watch.close()
  await h.until(()=>beaconAsks>=2,8000)
  expect(h.x.coaxial_land_pylon_free(h.p,0)).toBe(0)
  expect(h.x.coaxial_land_pylon_answers(h.p,0)).toBe(1)
  expect(h.x.coaxial_land_pylon_updates(h.p,0)).toBe(1)
  now+=301;h.frame();expect(h.x.coaxial_land_pylon_status(h.p,0)).toBe(3)
  expect(asks).toBeGreaterThan(3);expect(watch).toBeDefined()
 }finally{h.stop();server.stop(true)}
},15000)
