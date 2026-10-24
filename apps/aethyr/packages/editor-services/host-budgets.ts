/** Wire-v6 limits match the public Rust SDK. Older immutable releases retain theirs. */
export const HOST_FRAME_BYTES = 512 * 1024;
export const HOST_ACTION_BYTES = HOST_FRAME_BYTES - 8 * 1024;
export const AUTHORIZATION_BODY_BYTES = HOST_FRAME_BYTES + 16 * 1024;
export {
  MATERIAL_PNG_BYTES,
  MATERIAL_IMAGE_SIZE,
} from "../runtime/material-source.ts";
export function checkAuthorFrame(action: unknown, frame: unknown): void {
  const bytes = (value: unknown) =>
    new TextEncoder().encode(JSON.stringify(value)).byteLength;
  if (bytes(action) > HOST_ACTION_BYTES || bytes(frame) > HOST_FRAME_BYTES)
    throw new Error(
      "This change is too large. Export your sources and use fewer material layers.",
    );
}
