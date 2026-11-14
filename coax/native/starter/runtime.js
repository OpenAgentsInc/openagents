// Firm reason: 6 (JavaScriptCore SDK transport for compiler-generated Coax modules).
// No DOM, WebKit, networking, tokens, timers or application rules live here.
const cnMemory = {buffer: new ArrayBuffer(32 * 1024 * 1024)};
const cnExports = instantiate({env: {memory: cnMemory}, wasi_snapshot_preview1: {
  fd_fdstat_get: () => 8, fd_seek: () => 8, fd_write: () => 8,
}});
const cnState = Math.ceil(cnExports.__heap_base.value / 65536) * 65536;
if (cnExports.coaxial_state_size() > cnMemory.buffer.byteLength - cnState) throw new Error("App exceeds installed memory");
cnExports.coaxial_state_init(cnState);
function cnFrame() {
  const n = cnExports.dom_frame(cnState, 0, 1);
  return coaxTreeCommands(cnMemory.buffer, cnExports.dom_out(cnState), n,
    bytes => cnUTF8(Array.from(bytes)));
}
function cnEvent(kind, slot, bytes) {
  if (bytes.length > cnExports.dom_inbox_cap()) throw new Error("Input exceeds installed inbox");
  new Uint8Array(cnMemory.buffer, cnExports.dom_inbox(cnState), bytes.length).set(bytes);
  cnExports.dom_event(cnState, kind, slot, bytes.length);
}
function cnAction() {
  const op = cnExports.dom_act_kind(cnState);
  if (!op) return [0, ""];
  const body = cnExports.app_body ? cnUTF8(Array.from(new Uint8Array(cnMemory.buffer,
    cnExports.app_body(cnState), cnExports.app_body_len(cnState)))) : "{}";
  cnExports.dom_act_done(cnState);
  return [op, body];
}
