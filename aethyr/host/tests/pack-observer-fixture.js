// Firm reason: 1 (browser SDK: observation callback failures across actual Wasm construction, raw allocation, zeroing and ticket release).
import {openPackHost} from '../../../coax/web/pack-host.js';
export async function observerFixture(){
  const host=await openPackHost(),scene=await(await fetch('/scene/scene.json')).json(),r=scene.resources[5],url=`/scene/${r.container_sha256}.raw`,facts={};
  const Instance=WebAssembly.Instance,original=window.fetch;let constructs=0,requests=0,failConstructor=false,failedMemory;
  WebAssembly.Instance=new Proxy(Instance,{construct(target,args){constructs++;if(failConstructor){failedMemory=args[1].env.memory;throw Error('explicit instance constructor failure');}return Reflect.construct(target,args);}});
  window.fetch=(...args)=>{requests++;return original.apply(window,args);};
  try{
    for(const [name,kind,external] of [['planFailure','plan',false],['reactorFailure','allocate',false],['externalFailure','allocate',true],['releaseFailure','release',false]]){
      const owner={name},before=host.info(),c=constructs,n=requests;let failed=false,hit=false,retained;
      try{const transfer=await host.loadBytes(url,r.bytes,{owner,observe:e=>{if(e.kind===kind&&(kind!=='allocate'||!!e.external===external)){hit=true;throw Error(`explicit ${name} observer failure`);}}});retained=transfer.bytes;await host.retire(owner);}catch{failed=true;}finally{await host.retire(owner);}
      const after=host.info();facts[name]=failed&&hit&&after.owners===0&&after.storage[0]===before.storage[0]&&after.storage[1]===before.storage[1]&&(!retained||retained.every(b=>b===0))&&((kind==='plan'||kind==='allocate'&&!external)?constructs===c&&requests===n:constructs===c+1&&requests===n+1);
    }
    const events=[],owner={name:'constructor'},n=requests;failConstructor=true;let refused=false;
    try{await host.loadBytes(url,r.bytes,{owner,observe:e=>events.push(e)});}catch{refused=true;}finally{failConstructor=false;await host.retire(owner);}
    facts.constructorFailure=refused&&requests===n&&events.map(e=>e.kind).join('/')==='plan/allocate/constructor-refused'&&events[1].reactor===8388608&&events[2].reactor===8388608&&JSON.stringify(events[0].tickets)===JSON.stringify(events[2].tickets)&&events[2].storage[0]===1&&events[2].storage[1]===8388608&&new Uint8Array(failedMemory.buffer).every(b=>b===0);
    const after=host.info();facts.clean=after.storage[0]===1&&after.storage[1]===8388608&&['owners','loading','published','readers','sdkPending'].every(k=>after[k]===0);return facts;
  }finally{WebAssembly.Instance=Instance;window.fetch=original;}
}
