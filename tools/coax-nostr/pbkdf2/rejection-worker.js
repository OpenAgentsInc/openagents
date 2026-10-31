// Firm reason: 1 (browser worker glue: scheduling and transport for Coax's wasm in a Web Worker)
// Test-only observation at the exact terminal-message boundary. Production
// code never inspects or returns transferred plaintext input contents.
const receipts=new Map(),send=postMessage.bind(self);
postMessage=(message,transfer)=>{
 if(message.type==='error'){
  const buffers=receipts.get(message.id)??[];
  message={...message,observedInputBuffers:buffers.length,observedInputZero:buffers.every(b=>new Uint8Array(b).every(x=>x===0))};
 }
 send(message,transfer);
 if(['error','done','cancelled'].includes(message.type))receipts.delete(message.id);
};
importScripts('/worker.js');
const deliverToCanonical=onmessage;
onmessage=event=>{
 const buffers=new Set();
 for(const value of [event.data?.password,event.data?.salt]){
  if(value instanceof ArrayBuffer)buffers.add(value);
  else if(ArrayBuffer.isView(value)&&value.buffer instanceof ArrayBuffer)buffers.add(value.buffer);
 }
 receipts.set(event.data?.id,[...buffers]);deliverToCanonical(event);
};
