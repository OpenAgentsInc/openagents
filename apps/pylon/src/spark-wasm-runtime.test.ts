import { describe, expect, it } from "bun:test"
import { isBunCompiledBinaryUrl } from "./spark-wasm-runtime.js"

// #5404: the compiled-binary detector decides whether `ensureSparkWasmAvailable`
// extracts the embedded Spark WASM and sets PYLON_SPARK_WASM_PATH. It used to
// match only the POSIX embedded-FS marker (`/$bunfs/`), which returned FALSE on
// Windows (`B:\~BUN\root\…`), so Windows binaries never extracted the WASM and
// fell back to the missing build-machine path → ENOENT → receive-only/non-earning.
// These tests pin the cross-platform URL shapes so this can't silently regress.
describe("isBunCompiledBinaryUrl (#5404 cross-platform compiled-binary detection)", () => {
  it("detects the macOS/Linux POSIX embedded-FS URL", () => {
    expect(isBunCompiledBinaryUrl("file:///$bunfs/root/index.ts")).toBe(true)
    expect(isBunCompiledBinaryUrl("/$bunfs/root/spark-wasm-runtime.ts")).toBe(true)
  })

  it("detects the Windows drive-letter embedded-FS URL (the bug)", () => {
    // VERIFIED on a real Windows Server 2022 VM: Bun reports `import.meta.url`
    // as `file:///B:/%7EBUN/root/<exe>` — the `~` is PERCENT-ENCODED (`%7E`).
    // This exact shape is what the original detector (and a literal-`~BUN` first
    // attempt) missed, leaving Windows receive-only/non-earning.
    expect(isBunCompiledBinaryUrl("file:///B:/%7EBUN/root/pylon-windows-x64.exe")).toBe(true)
    // Lowercased percent-escape must still match.
    expect(isBunCompiledBinaryUrl("file:///b:/%7ebun/root/pylon-windows-x64.exe")).toBe(true)
    // Raw Bun Windows embedded path: `B:\~BUN\root\…` (backslashes, drive letter).
    expect(isBunCompiledBinaryUrl("B:\\~BUN\\root\\index.ts")).toBe(true)
    // Decoded url form with a literal tilde.
    expect(isBunCompiledBinaryUrl("file:///B:/~BUN/root/index.ts")).toBe(true)
    // A different drive letter must still match (don't hardcode `B:`).
    expect(isBunCompiledBinaryUrl("C:\\~BUN\\root\\index.ts")).toBe(true)
  })

  it("returns false for a normal source/npm module URL on every platform", () => {
    expect(isBunCompiledBinaryUrl("file:///Users/x/work/openagents/apps/pylon/src/index.ts")).toBe(
      false,
    )
    expect(
      isBunCompiledBinaryUrl("file:///home/runner/openagents/apps/pylon/src/index.ts"),
    ).toBe(false)
    // A Windows source checkout must NOT be misdetected as a compiled binary.
    expect(isBunCompiledBinaryUrl("file:///C:/Users/x/openagents/apps/pylon/src/index.ts")).toBe(
      false,
    )
    expect(isBunCompiledBinaryUrl("C:\\Users\\x\\openagents\\apps\\pylon\\src\\index.ts")).toBe(
      false,
    )
  })

  it("returns false for non-string / empty inputs", () => {
    expect(isBunCompiledBinaryUrl(undefined)).toBe(false)
    expect(isBunCompiledBinaryUrl(null)).toBe(false)
    expect(isBunCompiledBinaryUrl("")).toBe(false)
    expect(isBunCompiledBinaryUrl(123 as unknown)).toBe(false)
  })
})
