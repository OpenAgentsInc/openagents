// Firm reason: 1 (browser glue: measure media export scalars and form only Coax-admitted borrowed views).
// A null result refuses this packet. Empty packets do not require storage.
// The versioned caller supplies the common Coax span authority; a pre-v1
// caller omits it and retains its historical view behavior.
export function mediaSource(x, state, memory, span = null) {
  const bytes = x.coaxial_media_len?.(state) ?? 0;
  const address = bytes ? x.coaxial_media?.(state) ?? 0 : 0;
  if (span && !span(address, x.coaxial_media_capacity?.(state) ?? 0, bytes, memory.buffer.byteLength, !!x.coaxial_media_capacity)) return null;
  if (!bytes) return { address: 0, bytes: 0, view: new Uint8Array(0) };
  return { address, bytes, view: new Uint8Array(memory.buffer, address, bytes) };
}
