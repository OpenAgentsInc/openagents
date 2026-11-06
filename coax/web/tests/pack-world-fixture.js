// Firm reason: 1 (browser SDK fixture observing ordinary pack and concrete GPU/engine owners).
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function wait(test,label){const until=performance.now()+30000;while(!test()){if(performance.now()>until)throw Error(`pack world:${label}`);await sleep(20);}}
export async function packHostRefusal(cut){
 if(globalThis.packFault!==(cut?'cut':'hold'))throw Error('pack fault preload was not installed');
 const c=globalThis.coaxial,retention=()=>c.packRetention(),rows=()=>c.renderAssets().filter(row=>row.pack),sum=key=>rows().reduce((n,row)=>n+Number(!!row[key]),0),bytes=()=>rows().reduce((n,row)=>n+(row.bytes?.byteLength??0),0);
 if(cut){await wait(()=>sum('failed')===4&&sum('done')===4,'cut whole-pack refusal');const r=retention(),before=c.worldLoop();await wait(()=>c.worldLoop().frames>before.frames+6,'frames after cut');return [c.renderPresentation().version,r.groups,r.owners,r.published,r.loading,r.readers,sum('failed'),sum('done'),bytes(),Number(c.worldLoop().frames>before.frames),Number(c.worldLoop().submissions>before.submissions),r.storage[0],r.storage[1]];}
 await wait(()=>retention().readers===1&&retention().copied===3,'held physical bytes');const r=retention(),out=[r.groups,r.owners,r.loading,r.published,r.readers,sum('done'),bytes(),Number(c.ready)];c.retireRenderer();await wait(()=>retention().groups===0&&retention().owners===0,'retired pending owner');const closed=retention();out.push(closed.owners,closed.groups,closed.storage[0],closed.storage[1]);let remote=false;const until=performance.now()+5000;while(!remote&&performance.now()<until){remote=await(await fetch('http://127.0.0.1:9187/close-count')).text()==='1';if(!remote)await sleep(10);}out.push(Number(remote));return out;
}
export async function packWorldFixture(){
 const c=globalThis.coaxial,retention=()=>c.packRetention(),catalog=()=>c.renderAssets().find(a=>a.pack&&a.kind==='data')?.bytes;
 await wait(()=>c.sceneReady&&retention().published===1,'initial complete pack');await c.renderSettled();let r=retention();const initial=catalog(),out=[c.renderPresentation().version,r.groups,r.owners,r.published,r.loading,r.readers,r.sdkPending,Number(c.renderLedger()?.live>0),Number(c.worldLoop().submissions>0)];
 const reload=await c.reloadRenderer();out.push(Number(reload),Number(catalog()===initial),retention().groups);
 c.loseDevice();await wait(()=>c.gpuRecovery?.state==='recovered'&&c.sceneReady&&retention().published===1,'device recovery');await c.renderSettled();out.push(Number(c.gpuRecovery.state==='recovered'),initial.byteLength,retention().groups,retention().published);
 const recovered=catalog();out.push(Number(await c.reloadEngine()));await wait(()=>c.sceneReady,'engine replacement');await c.renderSettled();out.push(Number(catalog()===recovered),retention().groups);
 c.retireRenderer();await wait(()=>retention().owners===0&&retention().groups===0,'final pack release');r=retention();out.push(r.owners,r.groups,r.storage[0],r.storage[1]);return out;
}
