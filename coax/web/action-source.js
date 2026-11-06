// Firm reason: 1 (browser SDK: measure action exports and form only Coax-admitted byte views).
export function actionSource(x,state,memory,span=null){
 const bytes=x.coaxial_outbox_len(state);
 const address=bytes?x.coaxial_outbox(state):0;
 const capacity=bytes?(x.coaxial_outbox_capacity?.(state)??0):0;
 if(span&&!span(address,capacity,bytes,memory.buffer.byteLength,!!x.coaxial_outbox_capacity))return null;
 return {address,bytes,view:bytes?new Uint8Array(memory.buffer,address,bytes):new Uint8Array(0)};
}
