/** Owned straight-alpha map transport: AFR8 + version/width/height + RGBA8. */
export function decodeRgba8(buffer: ArrayBuffer): {
  width: number;
  height: number;
  pixels: Uint8Array;
} {
  if (buffer.byteLength < 16)
    throw new Error("The prepared material map header is incomplete.");
  const header = new DataView(buffer);
  if (
    header.getUint32(0, false) !== 0x41465238 ||
    header.getUint32(4, true) !== 1
  )
    throw new Error("The prepared material map header is invalid.");
  const width = header.getUint32(8, true),
    height = header.getUint32(12, true);
  if (
    width < 1 ||
    height < 1 ||
    width > 4096 ||
    height > 4096 ||
    buffer.byteLength !== 16 + width * height * 4
  )
    throw new Error("The prepared material map dimensions are invalid.");
  return { width, height, pixels: new Uint8Array(buffer, 16) };
}
