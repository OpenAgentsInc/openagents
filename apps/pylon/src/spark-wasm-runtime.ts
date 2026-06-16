// #5166: make the Breez Spark WASM loadable inside a Bun-compiled standalone
// binary on ANY machine.
//
// The SDK's `nodejs/breez_sdk_spark_wasm.js` loads its ~7 MB WASM eagerly at
// import time via `readFileSync(`${__dirname}/breez_sdk_spark_wasm_bg.wasm`)`.
// In a Bun-compiled binary, `__dirname` is baked to the BUILD machine's
// `node_modules/.bun/...` path, which does not exist on the user's machine — so
// the SDK throws on import and the offline-receive helper reports
// `helper-unavailable` (the binary half of #5166).
//
// Fix: the binary build (1) base64-embeds the WASM into
// `./generated/spark-wasm-b64` (bundled into the binary by Bun), and (2) patches
// the SDK's wasm loader to honor `PYLON_SPARK_WASM_PATH`. At runtime, before the
// SDK is imported, we extract the embedded WASM to a real temp file and point the
// SDK at it via that env var.
//
// Source / npm runs are untouched: the generated module is absent (the dynamic
// import is caught) and the SDK loads its own WASM from node_modules as before.

import { mkdirSync, writeFileSync, existsSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const WASM_BASENAME = "breez_sdk_spark_wasm_bg.wasm"
export const WASM_PATH_ENV = "PYLON_SPARK_WASM_PATH"

function isCompiledBinary(): boolean {
  return typeof import.meta.url === "string" && import.meta.url.includes("/$bunfs/")
}

let ensured = false

/**
 * Best-effort: ensure the Breez Spark WASM exists on disk and that
 * `PYLON_SPARK_WASM_PATH` points at it, so the SDK can load inside a compiled
 * standalone binary. No-op in source/npm runs. Never throws.
 */
export async function ensureSparkWasmAvailable(): Promise<void> {
  if (ensured) return
  ensured = true
  try {
    // An operator override (or a prior extraction) that still exists wins.
    const existing = process.env[WASM_PATH_ENV]
    if (existing && existsSync(existing)) return

    // Source/npm: the SDK resolves its own WASM from node_modules — nothing to do.
    if (!isCompiledBinary()) return

    // The generated module only exists in a binary build; absent elsewhere.
    const mod = (await import("./generated/spark-wasm-b64").catch(() => null)) as
      | { WASM_B64?: string }
      | null
    const b64 = mod?.WASM_B64
    if (typeof b64 !== "string" || b64.length === 0) return

    const bytes = Buffer.from(b64, "base64")
    const dir = join(tmpdir(), "oa-pylon-spark")
    mkdirSync(dir, { recursive: true })
    const out = join(dir, WASM_BASENAME)
    // Idempotent: only (re)write if missing or the wrong size.
    if (!existsSync(out) || statSync(out).size !== bytes.byteLength) {
      writeFileSync(out, bytes)
    }
    process.env[WASM_PATH_ENV] = out
  } catch {
    // Best-effort: on any failure leave the env unset and let the SDK fall back
    // to its own resolution (which works in source/npm).
  }
}
