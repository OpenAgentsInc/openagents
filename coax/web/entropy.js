// Generic browser CSPRNG effect. The passed exports are the actual Coax policy;
// this module owns address observation and WebCrypto calls, not admission policy.
// No seed/provider argument exists. Tests replace globals in an isolated realm.
export function createEntropy(memory, contract) {
  return function fill(out, capacity, scratch, scratchCapacity, requested) {
    const integer = (n) => Number.isSafeInteger(n);
    const n = integer(requested) ? requested : -1;
    const cap = integer(capacity) ? capacity : -1;
    const scratchCap = integer(scratchCapacity) ? scratchCapacity : -1;
    const bytes = memory.buffer.byteLength;
    const valid = integer(out) && integer(scratch) && out >= 0 && scratch >= 0 && n >= 0
      && out + n <= bytes && scratch + n <= bytes;
    const overlap = valid && n > 0 && out < scratch + n && scratch < out + n;
    const admitted = contract.entropy_admit(BigInt(n), BigInt(cap), BigInt(scratchCap), Number(valid), Number(overlap));
    if (admitted) return { written: 0, error: admitted };
    let crypto, random;
    try { crypto = globalThis.crypto; random = crypto?.getRandomValues; } catch {}
    const available = typeof random === "function";
    let ok = false, written = 0;
    if (available && n > 0) {
      try {
        const chunk = Number(contract.entropy_chunk_bytes());
        const full = Number(contract.entropy_full_chunks(BigInt(n)));
        const tail = Number(contract.entropy_tail(BigInt(n)));
        const view = new Uint8Array(memory.buffer, scratch, n);
        for (let i = 0; i < full; i++) {
          const part = view.subarray(written, written + chunk);
          if (random.call(crypto, part) !== part) throw new TypeError("CSPRNG returned a different view");
          written += chunk;
        }
        if (tail) {
          const part = view.subarray(written, written + tail);
          if (random.call(crypto, part) !== part) throw new TypeError("CSPRNG returned a different view");
          written += tail;
        }
        ok = true;
      } catch {}
    }
    const error = contract.entropy_finish(out, scratch, BigInt(cap), BigInt(scratchCap), BigInt(n), Number(valid), Number(overlap), Number(available), Number(ok), BigInt(written));
    return { written: error ? 0 : n, error };
  };
}
