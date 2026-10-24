export interface KernelRequestTimings {
  encodeMs: number;
  wasmRequestMs: number;
  decodeMs: number;
  overheadMs: number;
  totalMs: number;
}
export type KernelExports = WebAssembly.Exports & {
  memory: WebAssembly.Memory;
  af_alloc: (n: number) => number;
  af_free: (p: number, n: number) => void;
  af_request: (p: number, n: number) => number;
  af_response_len: () => number;
};

/** The same Rust authority and JSON/Wasm transport in editor and published players. */
export class RuntimeKernel {
  private calls = 0;
  private measured = false;
  private timings: KernelRequestTimings | null = null;
  setProfiling(enabled: boolean) {
    this.measured = enabled;
    this.timings = null;
  }
  get lastRequestTimings(): Readonly<KernelRequestTimings> | null {
    return this.timings;
  }
  private constructor(
    private readonly exports: KernelExports,
    private readonly startup: {
      compileMs: number;
      instantiateMs: number;
      moduleBytes: number;
      fetchMs: number;
    },
  ) {}
  static async create(options: { url?: string; fetcher?: typeof fetch } = {}) {
    const started = performance.now();
    const response = await (options.fetcher ?? fetch)(
      options.url ?? "/kernel.wasm",
    );
    if (!response.ok)
      throw new Error("Build the Rust kernel before opening the world.");
    const bytes = await response.arrayBuffer(),
      fetched = performance.now() - started;
    const kernel = await RuntimeKernel.fromBytes(bytes);
    kernel.startup.fetchMs = fetched;
    return kernel;
  }
  static async fromBytes(bytes: BufferSource): Promise<RuntimeKernel> {
    const started = performance.now();
    const module = await WebAssembly.compile(bytes);
    const compiled = performance.now();
    const instance = await WebAssembly.instantiate(module, {});
    const exports = instance.exports as KernelExports;
    if (
      !(exports.memory instanceof WebAssembly.Memory) ||
      [
        exports.af_alloc,
        exports.af_free,
        exports.af_request,
        exports.af_response_len,
      ].some((value) => typeof value !== "function")
    )
      throw new Error("Open a compatible Rust kernel module.");
    return new RuntimeKernel(exports, {
      compileMs: compiled - started,
      instantiateMs: performance.now() - compiled,
      moduleBytes: bytes.byteLength,
      fetchMs: 0,
    });
  }
  get memoryBytes() {
    return this.exports.memory.buffer.byteLength;
  }
  benchmarkInfo() {
    return {
      backend: "baseline",
      simulation: "current-rust-json",
      authorWasmBytes: this.memoryBytes,
      packedWasmBytes: 0,
      authorCalls: this.calls,
      packedSteps: 0,
      speedScale: 1,
      ...this.startup,
    };
  }
  request<T = any>(message: object): T {
    this.calls++;
    const measured = this.measured,
      started = measured ? performance.now() : 0;
    if (measured) this.timings = null;
    let encodeMs = 0,
      wasmRequestMs = 0,
      decodeMs = 0;
    const bytes = new TextEncoder().encode(JSON.stringify(message)),
      e = this.exports;
    if (measured) encodeMs = performance.now() - started;
    const pointer = e.af_alloc(bytes.length);
    if (!pointer)
      throw new Error("The simulation could not reserve command memory.");
    let result = 0,
      length = 0;
    try {
      new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes);
      const callStarted = measured ? performance.now() : 0;
      result = e.af_request(pointer, bytes.length);
      if (measured) wasmRequestMs = performance.now() - callStarted;
      length = e.af_response_len();
      if (!result || length > 16 * 1024 * 1024)
        throw new Error("The simulation returned an invalid response.");
      const decodeStarted = measured ? performance.now() : 0;
      const value = JSON.parse(
        new TextDecoder().decode(
          new Uint8Array(e.memory.buffer, result, length),
        ),
      ) as T;
      if (measured) decodeMs = performance.now() - decodeStarted;
      return value;
    } finally {
      e.af_free(pointer, bytes.length);
      if (result && length) e.af_free(result, length);
      if (measured) {
        const totalMs = performance.now() - started;
        this.timings = {
          encodeMs,
          wasmRequestMs,
          decodeMs,
          overheadMs: Math.max(
            0,
            totalMs - encodeMs - wasmRequestMs - decodeMs,
          ),
          totalMs,
        };
      }
    }
  }
}
