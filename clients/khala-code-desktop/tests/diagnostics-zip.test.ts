import { describe, expect, test } from "bun:test"

import {
  buildKhalaCodeDesktopZipArchive,
  crc32KhalaCodeDesktopDiagnostics,
  readKhalaCodeDesktopZipArchiveEntries,
} from "../src/shared/diagnostics-zip"

const encoder = new TextEncoder()
const decoder = new TextDecoder()

describe("buildKhalaCodeDesktopZipArchive / readKhalaCodeDesktopZipArchiveEntries", () => {
  test("round-trips a single entry", () => {
    const archive = buildKhalaCodeDesktopZipArchive([
      { data: encoder.encode("hello world\n"), path: "manifest.json" },
    ])
    // A well-formed ZIP starts with the local file header signature "PK\x03\x04".
    expect(archive[0]).toBe(0x50)
    expect(archive[1]).toBe(0x4b)
    expect(archive[2]).toBe(0x03)
    expect(archive[3]).toBe(0x04)

    const entries = readKhalaCodeDesktopZipArchiveEntries(archive)
    expect(entries).toHaveLength(1)
    expect(entries[0]?.path).toBe("manifest.json")
    expect(decoder.decode(entries[0]?.data)).toBe("hello world\n")
  })

  test("round-trips multiple entries with distinct names and empty files", () => {
    const archive = buildKhalaCodeDesktopZipArchive([
      { data: encoder.encode("{}"), path: "manifest.json" },
      { data: new Uint8Array(0), path: "main.log" },
      { data: encoder.encode("line one\nline two\n"), path: "renderer.log" },
    ])
    const entries = readKhalaCodeDesktopZipArchiveEntries(archive)
    expect(entries.map(entry => entry.path)).toEqual(["manifest.json", "main.log", "renderer.log"])
    expect(entries[1]?.data.length).toBe(0)
    expect(decoder.decode(entries[2]?.data)).toBe("line one\nline two\n")
  })

  test("preserves binary-safe content byte-for-byte", () => {
    const data = new Uint8Array([0, 1, 2, 255, 254, 253, 10, 13, 0])
    const archive = buildKhalaCodeDesktopZipArchive([{ data, path: "binary.bin" }])
    const entries = readKhalaCodeDesktopZipArchiveEntries(archive)
    expect([...(entries[0]?.data ?? [])]).toEqual([...data])
  })

  test("rejects unsafe entry paths that escape the archive root", () => {
    expect(() =>
      buildKhalaCodeDesktopZipArchive([
        { data: new Uint8Array(0), path: "../../etc/passwd" },
      ]),
    ).toThrow()
  })

  test("handles an empty entry list", () => {
    const archive = buildKhalaCodeDesktopZipArchive([])
    expect(readKhalaCodeDesktopZipArchiveEntries(archive)).toEqual([])
  })
})

describe("crc32KhalaCodeDesktopDiagnostics", () => {
  test("matches the well-known CRC-32 of the ASCII string 'hello'", () => {
    // Reference value from the standard CRC-32 (zlib/PNG) checksum table.
    expect(crc32KhalaCodeDesktopDiagnostics(encoder.encode("hello"))).toBe(0x3610a686)
  })

  test("is deterministic for the same input", () => {
    const data = encoder.encode("khala code desktop diagnostics")
    expect(crc32KhalaCodeDesktopDiagnostics(data)).toBe(crc32KhalaCodeDesktopDiagnostics(data))
  })
})
