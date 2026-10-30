// The real Coax/Wasm document. No content or UI state is injected.
import {test,expect} from 'bun:test'
import {readFileSync} from 'node:fs'
import {resolve} from 'node:path'
import {createHash} from 'node:crypto'
const root=resolve(import.meta.dir,'../../..'),markdown=readFileSync(resolve(root,'docs/connect-a-pylon.md'),'utf8')
const command='curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh && bash /tmp/join-pylon.sh'
async function guide(width:number,height:number,dpr:number){
 const memory=new WebAssembly.Memory({initial:128,maximum:1024})
 const {instance}=await WebAssembly.instantiate(readFileSync(resolve(root,'apps/openagents-web/build/connect-pylon.wasm')),{env:{memory},wasi_snapshot_preview1:{fd_fdstat_get:()=>52,fd_seek:()=>52,fd_write:()=>{throw Error("unexpected Wasm diagnostic")}}})
 const x=instance.exports as any;x._initialize();const state=Number(x.__heap_base.value)+1024
 const vertices=(state+x.coaxial_state_size()+31)&~15,uniforms=vertices+x.coaxial_max_vertices()*12*4+16
 const frame=()=>{const n=x.coaxial_frame(state,vertices,uniforms,width,height,dpr,0,1);expect(n).toBeGreaterThan(100);expect(n).toBeLessThanOrEqual(x.coaxial_max_vertices());const floats=new Float32Array(memory.buffer,vertices,n*12);expect(floats.every(Number.isFinite)).toBe(true);return {n,floats}}
 const control=(id:number)=>[0,1,2,3].map(axis=>x.coaxial_guide_control(state,id,axis))
 const click=(id:number)=>{const b=control(id),a=(b[0]+b[2])*dpr/2,c=(b[1]+b[3])*dpr/2;x.coaxial_pointer(state,a,c,0,0,1);x.coaxial_pointer(state,a,c,2,0,1)}
 const copied=()=>new TextDecoder().decode(new Uint8Array(memory.buffer,x.coaxial_outbox(state),x.coaxial_outbox_len(state)))
 return {x,state,frame,control,click,copied}
}
for(const v of [{w:390,h:844,d:1},{w:780,h:1688,d:2},{w:1440,h:900,d:1},{w:844,h:390,d:1}]){
 test(`guide controls, full copy and bounded readable geometry ${v.w}x${v.h}@${v.d}`,async()=>{
  const g=await guide(v.w,v.h,v.d),{x,state}=g,{floats}=g.frame();expect(x.coaxial_state_size()).toBeLessThan(65536);expect(x.coaxial_layout()>>>0).toBe((x.coaxial_state_size()*1000003+parseInt(createHash('sha256').update(markdown).digest('hex').slice(0,8),16))>>>0)
  // Body text actually uses the shared mixed-case system sans atlas, not
  // HUD capitals that would turn executable command casing into a puzzle.
  expect([...floats].filter((mode,i)=>i%12===11 && mode===4).length).toBeGreaterThan(400)
  for(const id of [0,1,2,3]){const b=g.control(id);expect(b[0]).toBeGreaterThanOrEqual(0);expect(b[1]).toBeGreaterThanOrEqual(0);expect(b[2]).toBeLessThanOrEqual(v.w/v.d);expect(b[3]).toBeLessThanOrEqual(v.h/v.d)}
  g.click(0);expect(x.coaxial_action(state)).toBe(2);expect(g.copied()).toBe(markdown);x.coaxial_action_done(state)
  g.click(1);expect(x.coaxial_action(state)).toBe(2);expect(g.copied()).toBe(command);x.coaxial_action_done(state)
  x.coaxial_wheel(state,100);expect(x.coaxial_guide_scroll(state)).toBe(100)
  x.coaxial_wheel(state,1e6);expect(x.coaxial_guide_scroll(state)).toBe(x.coaxial_guide_limit(state));g.frame()
  x.coaxial_wheel(state,-1e6);expect(x.coaxial_guide_scroll(state)).toBe(0)
  const b=g.control(0),a=(b[0]+10)*v.d,c=(b[1]+10)*v.d
  x.coaxial_pointer(state,a,c,0,1,7);x.coaxial_pointer(state,a,c+40*v.d,1,1,7);x.coaxial_pointer(state,a,c+40*v.d,2,1,7);expect(x.coaxial_action(state)).toBe(0)
  x.coaxial_pointer(state,a,c,0,1,7);x.coaxial_pointer(state,a,c,3,1,7);expect(x.coaxial_action(state)).toBe(0)
  g.click(2);expect(x.coaxial_action(state)).toBe(1);expect(g.copied()).toBe('/')
 })
}
