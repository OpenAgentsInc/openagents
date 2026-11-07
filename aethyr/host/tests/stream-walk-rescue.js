// Firm reason: 1 (IndexedDB transactions, WebCrypto and the stopped host's reload action; Coax admits compatibility and crash-loop policy).
export const rescueHash=async bytes=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(b=>b.toString(16).padStart(2,'0')).join('');
export async function openRescueStore(slot){
  const db=await new Promise(resolve=>{try{const request=indexedDB.open('coaxial-rescue',1);request.onupgradeneeded=()=>request.result.createObjectStore('saves');request.onsuccess=()=>resolve(request.result);request.onerror=request.onblocked=()=>resolve(null);}catch{resolve(null);}});
  // Read+delete commit as one transaction: concurrent consumers cannot both
  // restore the same image, and a failed delete never grants restoration.
  async function take(){return new Promise(resolve=>{
    if(!db){resolve(null);return;}
    try{let value=null;const tx=db.transaction('saves','readwrite'),store=tx.objectStore('saves'),request=store.get(slot);request.onsuccess=()=>{value=request.result||null;store.delete(slot);};tx.oncomplete=()=>resolve(value);tx.onerror=tx.onabort=()=>resolve(null);}catch{resolve(null);}
  });}
  async function write(value){return new Promise(resolve=>{
    if(!db){resolve(false);return;}
    try{const tx=db.transaction('saves','readwrite'),store=tx.objectStore('saves');if(value)store.put(value,slot);else store.delete(slot);tx.oncomplete=()=>resolve(true);tx.onerror=tx.onabort=()=>resolve(false);}catch{resolve(false);}
  });}
  return {take,write,close:()=>db?.close()};
}
export function showRescueStop(saving){
  const panel=document.createElement('div');panel.setAttribute('role','alert');panel.style.cssText='position:fixed;inset:0;display:grid;place-content:center;gap:16px;background:#101820dd;color:white;font:18px sans-serif;text-align:center';
  const message=document.createElement('span');message.textContent='The world stopped.';
  const action=document.createElement('button');action.textContent='Saving…';action.disabled=true;action.onclick=()=>location.reload();panel.append(message,action);document.body.append(panel);
  saving.then(saved=>{action.textContent=saved?'Reload where you were':'Start over';action.disabled=false;});return()=>panel.remove();
}
