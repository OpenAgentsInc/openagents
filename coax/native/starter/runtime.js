// Firm reason: 6 (JavaScriptCore SDK transport for compiler-generated Coax modules).
// No DOM, WebKit, networking, tokens, timers or application rules live here.
const cnMemory = {buffer: new ArrayBuffer(32 * 1024 * 1024)};
const cnRequests = [];
const cnString = (ptr,len) => cnUTF8(Array.from(new Uint8Array(cnMemory.buffer,ptr,len)));
const cnEffect = (op,url,n,body,k,step=0) => cnRequests.push([op,JSON.stringify({url:cnString(url,n),body:cnString(body,k),step})]);
const cnExports = instantiate({env: {memory: cnMemory,
  dom_stream:(u,n,b,k)=>cnEffect(401,u,n,b,k),
  dom_file_send:(u,n,b,k)=>cnEffect(402,u,n,b,k),
  dom_file_step:(u,n,b,k,s)=>cnEffect(403,u,n,b,k,s),
}, wasi_snapshot_preview1: {
  fd_fdstat_get: () => 8, fd_seek: () => 8, fd_write: () => 8,
}});
const cnState = Math.ceil(cnExports.__heap_base.value / 65536) * 65536;
if (cnExports.coaxial_state_size() > cnMemory.buffer.byteLength - cnState) throw new Error("App exceeds installed memory");
cnExports.coaxial_state_init(cnState);
function cnFrame() {
  let n = cnExports.dom_frame(cnState, 0, 1);
  const reset=n<0;
  if (reset) n=cnExports.dom_frame(cnState,0,1);
  if (n<0) throw new Error("Component reset did not settle");
  const commands=coaxTreeCommands(cnMemory.buffer, cnExports.dom_out(cnState), n,
    bytes => cnUTF8(Array.from(bytes)));
  return reset ? [[12],...commands] : commands;
}
function cnEvent(kind, slot, bytes) {
  if (bytes.length > cnExports.dom_inbox_cap()) throw new Error("Input exceeds installed inbox");
  new Uint8Array(cnMemory.buffer, cnExports.dom_inbox(cnState), bytes.length).set(bytes);
  cnExports.dom_event(cnState, kind, slot, bytes.length);
}
function cnAction() {
  if (cnRequests.length) return cnRequests.shift();
  const copied=cnExports.dom_clip_len?.(cnState) || 0;
  if (copied) {const text=cnString(cnExports.dom_clip(cnState),copied);cnExports.dom_clip_done(cnState);return [404,JSON.stringify({text})];}
  const op = cnExports.dom_act_kind(cnState);
  if (!op) return [0, ""];
  const count=cnExports.dom_act_len?.(cnState) || 0;
  const body = cnExports.app_body ? cnUTF8(Array.from(new Uint8Array(cnMemory.buffer,
    cnExports.app_body(cnState), cnExports.app_body_len(cnState)))) : cnExports.dom_act && count ? cnString(cnExports.dom_act(cnState),count) : "{}";
  cnExports.dom_act_done(cnState);
  return [op, body];
}
