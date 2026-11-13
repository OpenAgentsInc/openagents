// Firm reason: 1 (browser glue: scalar calls into the shared Coax host transport authority)
// Each adapter captures one concrete engine. Platform tasks retain the owner
// returned by Coax, never a slot number alone. No slot/byte/deadline policy here.
export function makeHostTransport(x,memory) {
  const version=x?.coaxial_host_contract_version?.();
  if (version===undefined || version===0) return null; // pinned/pre-contract face
  if (version!==1 || x.coaxial_host_transport_version?.()!==1) throw Error('unsupported Metaverse Engine host transport contract');
  const owner = id => ({id, generation: x.coaxial_host_owner(1), engine: x.coaxial_host_owner(2)});
  const args = o => [o.id, o.generation, o.engine];
  const delivery = accepted => accepted ? {kind:Number(x.coaxial_host_delivery(0)), bytes:Number(x.coaxial_host_delivery(1)), failure:!!x.coaxial_host_delivery(2)} : null;
  const route = (fn,text) => {
    const bytes=new TextEncoder().encode(text),capacity=x.coaxial_host_route_capacity();
    if (bytes.length>capacity) return false;
    new Uint8Array(memory.buffer,x.coaxial_host_route_storage(),bytes.length).set(bytes);
    return !!fn(bytes.length);
  };
  return {
    httpPath: text => route(x.coaxial_host_http_path,text),
    httpTarget: x.coaxial_host_http_target ? text => {
      const bytes=new TextEncoder().encode(text);
      if(bytes.length>x.coaxial_host_route_capacity())return null;
      new Uint8Array(memory.buffer,x.coaxial_host_route_storage(),bytes.length).set(bytes);
      const n=x.coaxial_host_http_target(bytes.length);
      return n<0 ? null : new TextDecoder('utf-8',{fatal:true}).decode(new Uint8Array(memory.buffer,x.coaxial_host_target_storage(),n));
    } : undefined,
    localUri: text => route(x.coaxial_host_local_uri,text),
    localOrigin: text => route(x.coaxial_host_local_origin,text),
    reset: () => !!x.coaxial_host_transport_reset(),
    socketReserve() { const id=x.coaxial_host_socket_reserve();return id < 0 ? null : owner(id); },
    socketSend: o => !!x.coaxial_host_socket_send(...args(o)),
    socketClose: o => !!x.coaxial_host_socket_close(...args(o)),
    socketEvent: (o,event,bytes,capacity) => delivery(x.coaxial_host_socket_event(...args(o),event,BigInt(bytes),BigInt(capacity))),
    httpReserve(flags,bounded,timeout,now,capacity) {
      const id=x.coaxial_host_http_reserve(flags,bounded?1:0,BigInt(timeout),BigInt(now),BigInt(capacity));
      return id < 0 ? null : owner(id);
    },
    httpChunk: (o,bytes) => !!x.coaxial_host_http_chunk(...args(o),BigInt(bytes)),
    httpFinish: (o,now,ok,utf8) => delivery(x.coaxial_host_http_finish(...args(o),BigInt(now),ok?1:0,utf8?1:0)),
    httpTimeout: (o,now) => delivery(x.coaxial_host_http_timeout(...args(o),BigInt(now))),
    httpCancel: id => !!x.coaxial_host_http_cancel(id),
  };
}
