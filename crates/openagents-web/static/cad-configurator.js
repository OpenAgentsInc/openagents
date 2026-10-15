// Pointer, slider, worker, and download glue for Rust CAD snapshots.
const form=document.querySelector('[data-cad-config-form]');
if(form) form.addEventListener('submit',()=>{
  form.elements.controls.value=JSON.stringify([...form.querySelectorAll('[data-cad-config-option]')]
    .filter(row=>row.querySelector('[data-config-chosen]').checked)
    .map(row=>({path:row.dataset.cadConfigOption,label:row.querySelector('[data-config-label]').value,
      min:Number(row.querySelector('[data-config-min]').value),max:Number(row.querySelector('[data-config-max]').value),step:Number(row.querySelector('[data-config-step]').value)})));
});
if(form){
  const submit=form.querySelector('button[type=submit]');submit.disabled=false;
  for(const name of ['aria-disabled','tabindex','data-disabled'])submit.removeAttribute(name);
  form.dataset.ready='1';
}
const page=document.querySelector('[data-cad-config-public]');
if(page){
  const status=page.querySelector('[data-config-status]'),viewer=page.querySelector('.oa-cad-viewer');
  const sliders=[...page.querySelectorAll('.oa-cad-config-sliders input[type=range]')],buttons=[...page.querySelectorAll('[data-config-download]')];
  const worker=new Worker('/cad/worker.js',{type:'module'});let seq=0,waiting=new Map(),snapshot=null,current=null,generation=0,pending=null,busy=false,ready=false;
  const say=text=>status.textContent=text;
  const enable=(el,on)=>{el.disabled=!on;for(const name of ['aria-disabled','tabindex','data-disabled'])if(on)el.removeAttribute(name);};
  const files=on=>buttons.forEach(b=>enable(b,on));
  const call=(call,args,format)=>new Promise((resolve,reject)=>{const id=++seq;waiting.set(id,{resolve,reject});worker.postMessage({id,call,args,format});});
  async function pump(){
    if(busy||!pending||!snapshot||!ready||!viewer.__cadSetModel)return;
    const job=pending;pending=null;busy=true;
    try{
      const result=await call('run',{tool:'cad_configure',args:{document:snapshot.document,controls:snapshot.controls,values:job.values}});
      if(job.generation===generation){
        if(result.error)throw new Error(result.error);
        current=result.document;viewer.__cadSetModel(result.view);files(true);say('Design checked. Your files are ready.');
      }
    }catch(error){if(job.generation===generation){current=null;files(false);say(error.message);}}
    finally{busy=false;pump();}
  }
  function rebuild(){current=null;files(false);pending={values:sliders.map(s=>Number(s.value)),generation:++generation};say('Checking your dimensions…');pump();}
  worker.onmessage=event=>{
    const m=event.data;
    if('ready'in m){ready=m.ready;if(!ready)say("This browser couldn't start the CAD engine. Reload to try again.");else pump();return;}
    const w=waiting.get(m.id);waiting.delete(m.id);if(w){if(m.ok)w.resolve(m.value);else w.reject(new Error(m.error));}
    if(m.broken){ready=false;files(false);say('The CAD engine stopped. Reload to try again.');}
  };
  fetch(page.dataset.cadConfigPublic,{credentials:'omit'}).then(r=>{if(!r.ok)throw new Error('This shared link is no longer available.');return r.json();}).then(async data=>{
    for(const asset of data.assets || []) {
      const bytes=Uint8Array.from(atob(asset.bytes_base64),c=>c.charCodeAt(0));
      const reference=await call('asset',{bytes,format:asset.format});
      if(reference!==`sha256:${asset.sha256}`)throw new Error('An imported model failed its SHA-256 check.');
    }
    snapshot=data;sliders.forEach((s,i)=>{s.value=String(data.values[i]);s.dispatchEvent(new Event('input',{bubbles:true}));});rebuild();
  }).catch(e=>say(e.message));
  sliders.forEach(s=>s.addEventListener('input',()=>{const text=s.closest('.oa-slider')?.querySelector('.oa-slider__value-text');if(text)text.textContent=s.value;rebuild();}));
  // The viewer loads independently; wait for its GPU setup before replacing it.
  const mounted=setInterval(()=>{if(viewer.__cadSetModel){clearInterval(mounted);pump();}},50);
  for(const b of buttons)b.addEventListener('click',async()=>{
    if(!current)return;
    const doc=current,stamp=generation,format=b.dataset.configDownload;files(false);say('Making your file…');
    try{
      const bytes=await call(format==='pdf'||format==='dxf'?'drawing':'export',{document:doc,views:['front','top','iso']},format);
      if(stamp!==generation)return;
      const types={step:'model/step',stl:'model/stl',pdf:'application/pdf',dxf:'image/vnd.dxf'};
      const url=URL.createObjectURL(new Blob([bytes],{type:types[format]}));const a=document.createElement('a');a.href=url;a.download=`design.${format}`;a.click();setTimeout(()=>URL.revokeObjectURL(url),10000);say('Your file is ready.');
    }catch(e){say(e.message);}finally{if(stamp===generation)files(!!current);}
  });
  window.addEventListener('pagehide',()=>{clearInterval(mounted);worker.terminate();waiting.forEach(w=>w.reject(new Error('The page closed.')));waiting.clear();});
}
