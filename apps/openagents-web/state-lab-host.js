// Firm reason: 1 (Canvas2D, WebAssembly memory, browser input and localStorage SDK effects)
// Layout, controls, collision, navigation, fluids, clocks and state are Coax.
const config=document.querySelector('script[data-module]')?.dataset;
const storage=config?.storage||'openagents.state-lab';
const bytes = await (await fetch(config?.module||'/build/state-lab.wasm')).arrayBuffer();
const decode = new TextDecoder(), encode = new TextEncoder();
async function createLab(credits=0) {
  const memory=new WebAssembly.Memory({initial:256,maximum:1024});
  const wasi={fd_write(fd,iovs,n,written){const v=new DataView(memory.buffer);let total=0;
    for(let i=0;i<n;i++){const p=v.getUint32(iovs+i*8,true),len=v.getUint32(iovs+i*8+4,true);console.error(decode.decode(new Uint8Array(memory.buffer,p,len)));total+=len;}
    v.setUint32(written,total,true);return 0;},proc_exit(code){throw Error(`State Lab exited (${code})`);},
    fd_fdstat_get:()=>52,fd_seek:()=>52,fd_close:()=>52,clock_time_get:()=>52};
  const {instance}=await WebAssembly.instantiate(bytes,{env:{memory},wasi_snapshot_preview1:wasi}),x={...instance.exports,memory};
  x._initialize?.();
  if (!x.lab_open(credits)) throw Error('Coax fixture refused initialization');
  const text=(p,n)=>decode.decode(new Uint8Array(x.memory.buffer,p,n));
  const names=Array.from({length:x.lab_metric_count()},(_,i)=>text(x.lab_metric_name_ptr(i),x.lab_metric_name_len(i)));
  const metrics=()=>Object.fromEntries(names.map((name,i)=>[name,x.lab_metric(i)]));
  const final=()=>({case:'final',hash:text(x.lab_hash_ptr(),64),metrics:metrics()});
  return {x,names,text,metrics,final,
    frame(ns){x.lab_frame(BigInt(ns));}, action(i){x.lab_action(i);},
    script(on=true,observe=true){x.lab_script(+on,+observe);},
    trial(fps,visual=false,observe=true){if(!x.lab_trial(fps,+visual,+observe))throw Error('Coax replay refused');const r=this.receipt();r.contract_hash=text(x.lab_contract_ptr(fps),64);return r;},
    steps(n){if(!x.lab_steps(n))throw Error('Coax step budget refused');},
    visual(){x.lab_action(13);},
    key(key,down){const b=encode.encode(key);if(b.length>8192)throw Error('SDK key span');new Uint8Array(x.memory.buffer,x.lab_input_ptr(),b.length).set(b);x.lab_key(b.length,+down);},
    snapshot(){return new Uint8Array(x.memory.buffer,x.lab_saved_ptr(),x.lab_saved_len()).slice();},
    import(b){if(b.length>8192)throw Error('SDK save span');new Uint8Array(x.memory.buffer,x.lab_input_ptr(),b.length).set(b);return !!x.lab_import(b.length);},
    receipt(){return {profile:config?.profile||'declared-state-1',memory_bytes:x.memory.buffer.byteLength,controls:x.lab_control_count?Array.from({length:x.lab_control_count()},(_,r)=>Array.from({length:4},(_,c)=>x.lab_control_word(r,c))):undefined,trace:Array.from({length:x.lab_trace_count()},(_,r)=>({hash:text(x.lab_trace_hash_ptr(r),64),metrics:Object.fromEntries(names.map((name,i)=>[name,x.lab_trace_metric(r,i)]))})),final:final()};}
  };
}
let credits=0;
try{credits=Number(localStorage.getItem(`${storage}.credit.v1`)||0);}catch{}
let current=await createLab(credits), savedVersion=0, running=true, last=null;
try{const saved=localStorage.getItem(`${storage}.save.v1`);if(saved)current.import(Uint8Array.from(atob(saved),x=>x.charCodeAt(0)));}catch{}
const canvas=document.querySelector('canvas'), ctx=canvas.getContext('2d');
function draw(){
  canvas.width=innerWidth;canvas.height=innerHeight;current.x.lab_view(canvas.width,canvas.height);
  ctx.fillStyle='#11191c';ctx.fillRect(0,0,canvas.width,canvas.height);
  const data=new Float32Array(current.x.memory.buffer,current.x.lab_draw_ptr(),current.x.lab_draw_count()*8);
  for(let i=0;i<data.length;i+=8){const [x,y,w,h,r,g,b,kind]=data.subarray(i,i+8);ctx.fillStyle=`rgb(${r*255} ${g*255} ${b*255})`;
    if(kind===0)ctx.fillRect(x,y,w,h);else{ctx.font=`${h}px monospace`;ctx.fillText(current.text(current.x.lab_text_ptr(kind-1),current.x.lab_text_len(kind-1)),x,y);}}
}
function persist(){try{
  localStorage.setItem(`${storage}.credit.v1`,String(current.metrics().credits));
  const version=current.x.lab_saved_version();if(version!==savedVersion){savedVersion=version;const b=current.snapshot();if(b.length)localStorage.setItem(`${storage}.save.v1`,btoa(String.fromCharCode(...b)));}
}catch{}}
function frame(now){if(running){if(last!==null)current.frame(Math.round((now-last)*1e6));last=now;draw();persist();}else last=null;requestAnimationFrame(frame);}
canvas.addEventListener('pointerdown',e=>current.x.lab_hit(e.clientX,e.clientY));
addEventListener('keydown',e=>{if(e.key.startsWith('Arrow'))e.preventDefault();current.key(e.key,true);});
addEventListener('keyup',e=>current.key(e.key,false));
window.stateLab={ready:true,create:createLab,get current(){return current;},pause(){running=false;},resume(){running=true;last=null;},show(lab){current=lab;savedVersion=0;draw();},draw};
draw();requestAnimationFrame(frame);
