// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
import { createEntropy } from "./entropy.js";

const contractNames = ["entropy_admit", "entropy_chunk_bytes", "entropy_full_chunks", "entropy_tail", "entropy_finish", "entropy_request_limit", "entropy_output_admit"];

// One host-owned reservation can serve successive engine instances. Calls are
// synchronous; the Coax completion resets its prefix before returning. This is
// ordinary reset, not a guarantee of optimization-resistant secret erasure.
export function prepareRandomImport(memory, module, reservation = {}) {
  const needed = WebAssembly.Module.imports(module).some((i) => i.kind === "function" && i.module === "env" && i.name === "coaxial_random");
  if (!needed) return { imports: {}, bind() {} };
  const names = new Set(WebAssembly.Module.exports(module).filter((e) => e.kind === "function").map((e) => e.name));
  for (const name of contractNames) if (!names.has(name)) throw new Error(`coaxial_random requires Coax export ${name}`);
  let contract, fill;
  return {
    imports: {
      // 0 success; otherwise the shared Coax ENTROPY_* error. No output writes
      // occur until Coax commits a complete request. No provider/seed argument.
      coaxial_random(out, requested) {
        if (!contract) throw new Error("coaxial_random called before its Coax instance was bound");
        const n = Number.isSafeInteger(requested) ? requested : -1;
        const valid = Number.isSafeInteger(out) && out >= 0 && n >= 0 && out + n <= memory.buffer.byteLength;
        const admitted = contract.entropy_output_admit(BigInt(n), BigInt(n), Number(valid));
        if (admitted) return admitted;
        if (!n) return fill(out, 0, 0, 0, 0).error;
        const capacity = Number(contract.entropy_request_limit());
        if (!reservation.capacity || reservation.capacity < capacity) {
          const start = memory.buffer.byteLength;
          try { memory.grow(Math.ceil(capacity / 65536)); }
          catch {
            // Missing scratch is a Coax capacity refusal, never false success.
            return contract.entropy_admit(BigInt(n), BigInt(n), 0n, 0, 0);
          }
          reservation.ptr = start;
          reservation.capacity = capacity;
        }
        return fill(out, n, reservation.ptr, reservation.capacity, n).error;
      },
    },
    bind(exports) {
      // This closure belongs to this instance, including across hot reload.
      contract = exports;
      // A larger reloaded module may now own the old host interval as static
      // data/stack. Move the next reservation beyond current memory in that case.
      if (reservation.capacity && Number(exports.__heap_base?.value ?? 0) > reservation.ptr) reservation.capacity = 0;
      fill = createEntropy(memory, contract);
    },
  };
}

// Unknown function imports must not be treated as successful no-op effects.
export function unsupportedImport(module, name) {
  throw new Error(`unsupported Wasm import ${module}.${name}`);
}
