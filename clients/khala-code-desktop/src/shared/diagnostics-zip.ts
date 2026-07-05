/**
 * Minimal, dependency-free ZIP (store method, no compression) archive writer
 * and reader for the Khala Code desktop debug-log export (issue #8441).
 *
 * Debug-log bundles are small, plain-text diagnostic summaries, so store-only
 * entries keep this module tiny and fully self-contained (no external zip
 * library, no shelling out to a system `zip`/`tar` binary). The reader exists
 * so the export path can be round-trip tested without depending on any
 * external unzip tool either.
 *
 * This intentionally implements only the subset of the ZIP format needed for
 * flat, stored (uncompressed) entries: local file headers, an end-of-central-
 * directory record, and a matching central directory. It does not support
 * compression, encryption, ZIP64, or directory entries.
 */

export type KhalaCodeDesktopZipArchiveEntry = Readonly<{
  data: Uint8Array
  path: string
}>

const LOCAL_FILE_HEADER_SIGNATURE = 0x04034b50
const CENTRAL_DIRECTORY_HEADER_SIGNATURE = 0x02014b50
const END_OF_CENTRAL_DIRECTORY_SIGNATURE = 0x06054b50
const VERSION_NEEDED = 20
const STORE_METHOD = 0

// Standard reflected CRC-32 (polynomial 0xEDB88320) table, computed once.
const CRC32_TABLE: Uint32Array = (() => {
  const table = new Uint32Array(256)
  for (let n = 0; n < 256; n += 1) {
    let c = n
    for (let k = 0; k < 8; k += 1) {
      c = (c & 1) !== 0 ? (0xedb88320 ^ (c >>> 1)) : (c >>> 1)
    }
    table[n] = c >>> 0
  }
  return table
})()

export const crc32KhalaCodeDesktopDiagnostics = (data: Uint8Array): number => {
  let crc = 0xffffffff
  for (let i = 0; i < data.length; i += 1) {
    const byte = data[i] ?? 0
    crc = CRC32_TABLE[(crc ^ byte) & 0xff]! ^ (crc >>> 8)
  }
  return (crc ^ 0xffffffff) >>> 0
}

const textEncoder = new TextEncoder()
const textDecoder = new TextDecoder()

/** MS-DOS date/time encoding fixed at the Unix epoch — content is what matters, not the timestamp. */
const DOS_TIME = 0
const DOS_DATE = 0b0000000000100001 // 1980-01-01, the DOS epoch floor.

type WritableSegment = Uint8Array

const concatSegments = (segments: readonly WritableSegment[]): Uint8Array => {
  const total = segments.reduce((sum, segment) => sum + segment.length, 0)
  const out = new Uint8Array(total)
  let offset = 0
  for (const segment of segments) {
    out.set(segment, offset)
    offset += segment.length
  }
  return out
}

const uint16LE = (value: number): Uint8Array => {
  const out = new Uint8Array(2)
  new DataView(out.buffer).setUint16(0, value & 0xffff, true)
  return out
}

const uint32LE = (value: number): Uint8Array => {
  const out = new Uint8Array(4)
  new DataView(out.buffer).setUint32(0, value >>> 0, true)
  return out
}

/**
 * Builds a valid, store-only ZIP archive from a flat list of path/data
 * entries. Paths are normalized to forward slashes and must not escape the
 * archive root (no leading slash, no `..` segments) — callers control entry
 * names directly (manifest.json, main.log, etc), so this is a defensive
 * assertion rather than an untrusted-input sanitizer.
 */
export const buildKhalaCodeDesktopZipArchive = (
  entries: readonly KhalaCodeDesktopZipArchiveEntry[],
): Uint8Array => {
  const localSegments: WritableSegment[] = []
  const centralSegments: WritableSegment[] = []
  let offset = 0

  for (const entry of entries) {
    const path = entry.path.replace(/\\/g, "/").replace(/^\/+/, "")
    if (path.length === 0 || path.split("/").includes("..")) {
      throw new Error(`Refusing to add unsafe zip entry path: ${entry.path}`)
    }
    const nameBytes = textEncoder.encode(path)
    const crc = crc32KhalaCodeDesktopDiagnostics(entry.data)
    const size = entry.data.length

    const localHeader = concatSegments([
      uint32LE(LOCAL_FILE_HEADER_SIGNATURE),
      uint16LE(VERSION_NEEDED),
      uint16LE(0), // general purpose bit flag
      uint16LE(STORE_METHOD),
      uint16LE(DOS_TIME),
      uint16LE(DOS_DATE),
      uint32LE(crc),
      uint32LE(size), // compressed size == size (store method)
      uint32LE(size),
      uint16LE(nameBytes.length),
      uint16LE(0), // extra field length
      nameBytes,
    ])
    const localEntry = concatSegments([localHeader, entry.data])
    localSegments.push(localEntry)

    const centralHeader = concatSegments([
      uint32LE(CENTRAL_DIRECTORY_HEADER_SIGNATURE),
      uint16LE(VERSION_NEEDED), // version made by
      uint16LE(VERSION_NEEDED), // version needed to extract
      uint16LE(0), // general purpose bit flag
      uint16LE(STORE_METHOD),
      uint16LE(DOS_TIME),
      uint16LE(DOS_DATE),
      uint32LE(crc),
      uint32LE(size),
      uint32LE(size),
      uint16LE(nameBytes.length),
      uint16LE(0), // extra field length
      uint16LE(0), // file comment length
      uint16LE(0), // disk number start
      uint16LE(0), // internal file attributes
      uint32LE(0), // external file attributes
      uint32LE(offset), // relative offset of local header
      nameBytes,
    ])
    centralSegments.push(centralHeader)

    offset += localEntry.length
  }

  const centralDirectory = concatSegments(centralSegments)
  const centralDirectoryOffset = offset
  const endRecord = concatSegments([
    uint32LE(END_OF_CENTRAL_DIRECTORY_SIGNATURE),
    uint16LE(0), // number of this disk
    uint16LE(0), // disk where central directory starts
    uint16LE(entries.length), // number of central directory records on this disk
    uint16LE(entries.length), // total number of central directory records
    uint32LE(centralDirectory.length),
    uint32LE(centralDirectoryOffset),
    uint16LE(0), // comment length
  ])

  return concatSegments([...localSegments, centralDirectory, endRecord])
}

/**
 * Reads back a store-only ZIP archive produced by
 * `buildKhalaCodeDesktopZipArchive`. Used by tests to prove the export
 * round-trips without depending on any external unzip tool. Not a general
 * ZIP parser — it assumes store method and no ZIP64 extensions, matching
 * what the writer above produces.
 */
export const readKhalaCodeDesktopZipArchiveEntries = (
  archive: Uint8Array,
): readonly KhalaCodeDesktopZipArchiveEntry[] => {
  const view = new DataView(archive.buffer, archive.byteOffset, archive.byteLength)
  const entries: KhalaCodeDesktopZipArchiveEntry[] = []
  let offset = 0

  while (offset < archive.length) {
    const signature = view.getUint32(offset, true)
    if (signature !== LOCAL_FILE_HEADER_SIGNATURE) break

    const nameLength = view.getUint16(offset + 26, true)
    const extraLength = view.getUint16(offset + 28, true)
    const compressedSize = view.getUint32(offset + 18, true)
    const nameStart = offset + 30
    const dataStart = nameStart + nameLength + extraLength
    const path = textDecoder.decode(archive.subarray(nameStart, nameStart + nameLength))
    const data = archive.slice(dataStart, dataStart + compressedSize)
    entries.push({ data, path })
    offset = dataStart + compressedSize
  }

  return entries
}
